use std::mem::size_of;
use std::net::UdpSocket;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use rand::{thread_rng, Rng};
use std::convert::TryInto;

use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;

use crate::communication::{DataMsg, ABORT, COMMIT, PAYMENT_OK, PREPARE};
use crate::logger::LogLevel;
use crate::logger::Logger;

use std::net::SocketAddr;

use crate::utils::{create_empty_csv, csv_to_prices, get_agents_ports, write_to_csv};

fn id_to_ctrladdr(id: usize) -> String {
    "127.0.0.1:1234".to_owned() + &*id.to_string()
}
fn id_to_dataaddr(id: usize) -> String {
    "127.0.0.1:1235".to_owned() + &*id.to_string()
}

const TIMEOUT: Duration = Duration::from_secs(5);

const MSG_ACK: u8 = b'A';
const MSG_ELECTION: u8 = b'E';
const MSG_COORDINATOR: u8 = b'C';

pub struct LeaderElection {
    id: usize,
    socket: UdpSocket,
    leader_id: Arc<(Mutex<Option<usize>>, Condvar)>,
    got_ack: Arc<(Mutex<Option<usize>>, Condvar)>,
    stop: Arc<(Mutex<bool>, Condvar)>,
    last_id: Arc<(Mutex<usize>, Condvar)>,
    last_status: Arc<(Mutex<u8>, Condvar)>,
}

impl LeaderElection {
    pub fn new(id: usize) -> LeaderElection {
        let mut ret = LeaderElection {
            id,
            socket: UdpSocket::bind(id_to_ctrladdr(id)).unwrap(),
            leader_id: Arc::new((Mutex::new(Some(id)), Condvar::new())),
            got_ack: Arc::new((Mutex::new(None), Condvar::new())),
            stop: Arc::new((Mutex::new(false), Condvar::new())),
            last_id: Arc::new((Mutex::new(0), Condvar::new())),
            last_status: Arc::new((Mutex::new(b'C'), Condvar::new())),
        };

        let mut clone = ret.clone();
        thread::spawn(move || clone.responder());

        ret.find_new();
        ret
    }

    fn responder(&mut self) {
        while !*self.stop.0.lock().unwrap() {
            let mut buf = [0; 1 + size_of::<usize>() + (5 + 1) * size_of::<usize>()]; // todo: dynamic o harcore
            let (_size, from) = self.socket.recv_from(&mut buf).unwrap();
            let (msg_type, mut ids) = self.parse_message(&buf);

            match msg_type {
                MSG_ACK => {
                    println!("[{}] recibí ACK de {}", self.id, from); //
                    *self.got_ack.0.lock().unwrap() = Some(ids[0]);
                    self.got_ack.1.notify_all();
                }
                MSG_ELECTION => {
                    println!("[{}] recibí Election de {}, ids {:?}", self.id, from, ids);
                    self.socket
                        .send_to(&self.ids_to_msg(MSG_ACK, &[self.id]), from)
                        .unwrap();
                    if ids.contains(&self.id) {
                        let winner = *ids.iter().max().unwrap();
                        self.socket
                            .send_to(&self.ids_to_msg(MSG_COORDINATOR, &[winner, self.id]), from)
                            .unwrap();
                    } else {
                        ids.push(self.id);
                        let msg = self.ids_to_msg(MSG_ELECTION, &ids);
                        let clone = self.clone();
                        thread::spawn(move || clone.safe_send_next(&msg, clone.id));
                    }
                }
                MSG_COORDINATOR => {
                    println!(
                        "[{}] recibí nuevo coordinador de {}, ids {:?}",
                        self.id, from, ids
                    );
                    *self.leader_id.0.lock().unwrap() = Some(ids[0]);
                    self.leader_id.1.notify_all();
                    self.socket
                        .send_to(&self.ids_to_msg(MSG_ACK, &[self.id]), from)
                        .unwrap();
                    if !ids[1..].contains(&self.id) {
                        ids.push(self.id);
                        let msg = self.ids_to_msg(MSG_COORDINATOR, &ids);
                        let clone = self.clone();
                        thread::spawn(move || clone.safe_send_next(&msg, clone.id));
                    }
                }
                _ => {
                    println!("[{}] ??? {:?}", self.id, ids);
                }
            }
        }
        *self.stop.0.lock().unwrap() = false;
        self.stop.1.notify_all();
    }

    fn parse_message(&self, buf: &[u8]) -> (u8, Vec<usize>) {
        let mut ids = vec![];

        let count = usize::from_le_bytes(buf[1..1 + size_of::<usize>()].try_into().unwrap());

        let mut pos = 1 + size_of::<usize>();
        for _id in 0..count {
            ids.push(usize::from_le_bytes(
                buf[pos..pos + size_of::<usize>()].try_into().unwrap(),
            ));
            pos += size_of::<usize>();
        }

        (buf[0], ids)
    }

    fn ids_to_msg(&self, header: u8, ids: &[usize]) -> Vec<u8> {
        let mut msg = vec![header];
        msg.extend_from_slice(&ids.len().to_le_bytes());
        for id in ids {
            msg.extend_from_slice(&id.to_le_bytes());
        }
        msg
    }

    fn safe_send_next(&self, msg: &[u8], id: usize) {
        let next_id = self.next(id);
        if next_id == self.id {
            println!("[{}] enviando {} a {}", self.id, msg[0] as char, next_id);
            panic!("Di toda la vuelta sin respuestas")
        }
        *self.got_ack.0.lock().unwrap() = None;
        let _ignore = self.socket.send_to(msg, id_to_ctrladdr(next_id));
        let got_ack =
            self.got_ack
                .1
                .wait_timeout_while(self.got_ack.0.lock().unwrap(), TIMEOUT, |got_it| {
                    got_it.is_none() || got_it.unwrap() != next_id
                });
        if got_ack.unwrap().1.timed_out() {
            self.safe_send_next(msg, next_id)
        }
    }

    fn next(&self, id: usize) -> usize {
        (id + 1) % 5 // TODO: team members dynamic?
    }

    fn find_new(&mut self) {
        if *self.stop.0.lock().unwrap() {
            return;
        }
        println!("[{}] buscando lider", self.id);
        *self.leader_id.0.lock().unwrap() = None;

        self.safe_send_next(&self.ids_to_msg(MSG_ELECTION, &[self.id]), self.id);

        let _ignore = self.leader_id
            .1
            .wait_while(self.leader_id.0.lock().unwrap(), |leader_id| {
                leader_id.is_none()
            });

        // Validar que lo anterior se resolvio
        // Fijarse en su log, la ultima linea aka el ultimo msj que se mando
        // Si es un prepare ->  id ABORT
        // Si es un commit ->  sigo con el siguiente msj
        // si es un abort -> sigo con el siguiente msj
    }

    fn clone(&self) -> LeaderElection {
        LeaderElection {
            id: self.id,
            socket: self.socket.try_clone().unwrap(),
            leader_id: self.leader_id.clone(),
            got_ack: self.got_ack.clone(),
            stop: self.stop.clone(),
            last_id: self.last_id.clone(),
            last_status: self.last_status.clone(),
        }
    }

    fn broadcast(
        &self,
        transaction_id: usize,
        transaction_prices: &[u32],
        operation: u8,
        agent_clients: &[TcpStream],
        im_alive: &Arc<AtomicBool>,
        logger: &Logger,
    ) -> (Vec<[u8; 1]>, bool) {
        let responses = Arc::new((Mutex::new(vec![]), Condvar::new()));

        for (i, agent_client) in agent_clients.iter().enumerate() {
            let im_alive_clone = im_alive.clone();
            let logger_clone = logger.clone();

            let mut agent_client_clone = agent_client
                .try_clone()
                .expect("Could not clone agent client");
            let responses_clone = responses.clone();

            let msg = DataMsg {
                transaction_id: transaction_id as u32,
                opcode: operation,
                data: transaction_prices[i],
            };

            thread::Builder::new()
                .name(format!("Transaction {}", transaction_id))
                .spawn(move || {
                    agent_client_clone
                        .write_all(&DataMsg::to_bytes(&msg))
                        .unwrap_or_else(|_| {
                            im_alive_clone.store(false, Ordering::SeqCst);
                        });

                    let mut response: [u8; 1] = Default::default();
                    agent_client_clone
                        .read_exact(&mut response)
                        .unwrap_or_else(|_| {
                            im_alive_clone.store(false, Ordering::SeqCst);
                        });

                    if !im_alive_clone.load(Ordering::SeqCst) {
                        logger_clone.log(
                            "Connection with agent suddenly closed".to_string(),
                            LogLevel::INFO,
                        )
                    }

                    let (lock, cvar) = &*responses_clone;
                    lock.lock()
                        .expect("Unable to lock responses")
                        .push(response);

                    cvar.notify_all();
                })
                .expect("thread creation failed");
        }

        let (lock, cvar) = &*responses;
        let (all_responses, timeout) = cvar
            .wait_timeout_while(
                lock.lock().expect("Unable to lock responses"),
                TIMEOUT,
                |responses| responses.len() != agent_clients.len(),
            )
            .expect("Error on wait condvar");

        (all_responses.to_vec(), timeout.timed_out())
    }

    fn process_payments(&self) {
        let im_alive = Arc::new(AtomicBool::new(true));
        let im_alive_clone_ctrlc = im_alive.clone();

        ctrlc::set_handler(move || {
            im_alive_clone_ctrlc.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");

        let agents_ports = get_agents_ports();

        let prices_file = match env::args().nth(1) {
            Some(val) => val,
            None => "src/prices.csv".to_string(),
        };
        let prices = csv_to_prices(&prices_file);

        let retry_file = create_empty_csv("src/prices-retry.csv");
        let logger = Logger::new("alglobo".to_string());
        let mut transactions_state: HashMap<u32, u8> = HashMap::new();

        let mut agent_clients = Vec::new();
        for port in &agents_ports {
            let addr = SocketAddr::from(([127, 0, 0, 1], *port));
            let client = TcpStream::connect(addr)
                .unwrap_or_else(|_| panic!("connection with port {} failed", port));
            agent_clients.push(client);
        }

        while *self.last_id.0.lock().unwrap() < prices.len() - 1 {
            *self.last_id.0.lock().unwrap() += 1;

            let transaction_id = *self.last_id.0.lock().unwrap();
            let transaction_prices = prices[transaction_id].clone();

            if !im_alive.load(Ordering::SeqCst) {
                break;
            };

            let im_alive_clone_agents = im_alive.clone();
            let logger_clone = logger.clone();

            logger.log(
                format!("Transaction {} | PREPARE", transaction_id),
                LogLevel::TRACE,
            );
            transactions_state.insert(transaction_id as u32, PREPARE);

            let (all_responses, is_timeout) = self.broadcast(
                transaction_id,
                &transaction_prices,
                PREPARE,
                &agent_clients,
                &im_alive_clone_agents,
                &logger_clone,
            );
            self.broadcast_last_log(PREPARE, transaction_id);

            let all_oks = all_responses.iter().all(|&opt| opt[0] == PAYMENT_OK);

            let operation = if all_oks && !is_timeout && im_alive.load(Ordering::SeqCst) {
                COMMIT
            } else {
                ABORT
            };
            logger.log(
                format!(
                    "Payment of {:?} | {}",
                    transaction_prices,
                    if operation == COMMIT { "OK" } else { "ERR" },
                ),
                LogLevel::INFO,
            );
            logger.log(
                format!(
                    "Transaction {} | {}",
                    transaction_id,
                    if operation == COMMIT {
                        "COMMIT"
                    } else {
                        "ABORT"
                    },
                ),
                LogLevel::TRACE,
            );
            transactions_state.insert(transaction_id as u32, operation);

            if operation == ABORT {
                write_to_csv(&retry_file, &transaction_prices);
            }

            let (_all_responses, _is_timeout) = self.broadcast(
                transaction_id,
                &transaction_prices,
                operation,
                &agent_clients,
                &im_alive_clone_agents,
                &logger_clone,
            );
            self.broadcast_last_log(operation, transaction_id);

            // This sleep is only for debugging purposes
            sleep(Duration::from_millis(1000));
        }

        // TODO: Pensar que hacer con esto
        // let dummy_data = vec![0; agent_clients.len()];
        // let (_all_responses, _is_timeout) =
        //     self.broadcast(0, &dummy_data, FINISH, &agent_clients, &im_alive, &logger);
    }

    pub fn broadcast_last_log(&self, status: u8, id: usize) {
        let msg = [
            status as u8,
            id as u8, // TODO: resize
        ];

        for i in 0..5 {
            // TODO: unhardcode maybe
            if i == self.get_leader_id() {
                continue;
            }
            self.socket.send_to(&msg, id_to_dataaddr(i)).unwrap();
        }
    }

    pub fn loop_node(&mut self) {
        loop {
            println!("[{}] inicio", self.id);
            let socket = UdpSocket::bind(id_to_dataaddr(self.id)).unwrap();
            // let im_alive = Arc::new(AtomicBool::new(true));

            loop {
                if self.am_i_leader() {
                    println!("[{}] soy SM", self.id);
                    if thread_rng().gen_range(0, 100) >= 110 {
                        // TODO: no es random, tras recibir un ctrl + c se cae el lider u otra combinacion de teclas para elegir cual se cae
                        println!("[{}] se cae el lider", self.id);
                        break;
                    }
                    let _ignore =socket.set_read_timeout(None);

                    self.process_payments();
                    // setten los last_id y last_status

                    // for de todos los mensajes
                    // self.resolve_lider_tasks()
                    // enviar el prepare
                    // recibierlo
                    // broadcasteo el id el mensaje y el resulado
                    // commitear o abortar
                    // recien cuanto tengo todo, si es commit -> broadcastear a replicas

                    // two phase commit:

                    // alglobo  --> prepare --> agentes
                    // -----
                    // agentes  --> ready   --> alglobo
                    // alglobo  (procesa)
                    // alglobo  --> commit  --> agentes
                    // -----
                    // agentes  --> finish  --> alglobo

                    // let (size, from) = socket.recv_from(&mut buf).unwrap();
                    // socket.send_to("PONG".as_bytes(), from).unwrap();
                } else {
                    let leader_id = self.get_leader_id();
                    println!("[{}] pido trabajo al SM {}", self.id, leader_id);

                    let mut response: [u8; 2] = Default::default();
                    socket.set_read_timeout(Some(TIMEOUT)).unwrap();
                    if let Ok((_size, _from)) = socket.recv_from(&mut response) {
                        *self.last_status.0.lock().unwrap() = response[0];
                        *self.last_id.0.lock().unwrap() = response[1] as usize;
                        println!("[{}] trabajando", self.id);
                        thread::sleep(Duration::from_millis(1000));
                    } else {
                        println!("[{}] comenzando la busqeuda de un nuevo lider :3", self.id);
                        self.find_new();
                    }
                }
            }

            self.stop();

            thread::sleep(Duration::from_secs(90));
        }
    }

    fn am_i_leader(&self) -> bool {
        self.get_leader_id() == self.id
    }

    fn get_leader_id(&self) -> usize {
        self.leader_id
            .1
            .wait_while(self.leader_id.0.lock().unwrap(), |leader_id| {
                leader_id.is_none()
            })
            .unwrap()
            .unwrap()
    }

    fn stop(&mut self) {
        *self.stop.0.lock().unwrap() = true;
        let _ignore = self.stop
            .1
            .wait_while(self.stop.0.lock().unwrap(), |should_stop| *should_stop);
    }
}
