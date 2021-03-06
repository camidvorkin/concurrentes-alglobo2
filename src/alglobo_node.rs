//! AlgloboNode struct
//!
//! Handles the logic of each Algobo.com node, where the leader is chosen with the ring
//! election algorithm. The leader will send each transaction result to the replicas.
//!
//! The communication between nodes is done via UDP, where each node will have two
//! sockets. One for receiving leader election messages and the other for receiving
//! transaction results.
//!
//! If the replicas reach a timeout without messages from the leader, a new leader is
//! coordinated.

use std::mem::size_of;
use std::net::UdpSocket;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use std::convert::TryInto;

use std::env;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;

use crate::communication::{DataMsg, ABORT, COMMIT, FINISH, PAYMENT_ERR, PAYMENT_OK, PREPARE};
use crate::logger::Logger;

use std::net::SocketAddr;

use crate::utils::{create_empty_csv, csv_to_prices, get_agents_ports, write_to_csv};

/// Socket used for receiving leader election/coordination messages
pub fn id_to_ctrladdr(id: usize) -> SocketAddr {
    let port = (1100 + id) as u16;
    SocketAddr::from(([127, 0, 0, 1], port))
}
/// Socket used for receiving transaction information from the leader
pub fn id_to_dataaddr(id: usize) -> SocketAddr {
    let port = (1200 + id) as u16;
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// The amount of nodes to launch the program with
pub const N_NODES: usize = 5;
/// Control message for ACKs
pub const MSG_ACK: u8 = b'A';
/// Control message for election messages
pub const MSG_ELECTION: u8 = b'E';
/// Control message for coordinator messages
pub const MSG_COORDINATOR: u8 = b'C';
/// Control message for kill messages
pub const MSG_KILL: u8 = b'K';
/// Timeout for receiving transaction information in the replicas
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// AlgloboNode struct
pub struct AlgloboNode {
    /// The id of the node
    id: usize,
    /// The UDP socket of the node
    socket: UdpSocket,
    /// The id of the leader, with a lock and a condvar
    leader_id: Arc<(Mutex<Option<usize>>, Condvar)>,
    /// Flag for reliable data transfer via UDP, with a lock and a condvar
    got_ack: Arc<(Mutex<Option<usize>>, Condvar)>,
    /// Stop flag to end the node's threads
    stop: Arc<AtomicBool>,
    /// Last processed id of the transaction, with a lock
    last_id: Arc<Mutex<usize>>,
    /// Last processed id of the transaction, with a lock
    last_status: Arc<Mutex<u8>>,
    /// Logger of the node
    logger: Logger,
}

// Mutexes are more explicit than atomic stuff when controlling threads!
#[allow(clippy::mutex_atomic)]
impl AlgloboNode {
    /// Creates the AlgoboNode and starts the control responder thread
    pub fn new(id: usize) -> AlgloboNode {
        let mut ret = AlgloboNode {
            id,
            socket: UdpSocket::bind(id_to_ctrladdr(id)).expect("Unable to bind socket"),
            leader_id: Arc::new((Mutex::new(Some(id)), Condvar::new())),
            got_ack: Arc::new((Mutex::new(None), Condvar::new())),
            stop: Arc::new(AtomicBool::new(false)),
            last_id: Arc::new(Mutex::new(0)),
            last_status: Arc::new(Mutex::new(0)),
            logger: Logger::new(format!("node-{}", id)),
        };

        let mut clone = ret.clone();
        thread::Builder::new()
            .name(format!("Node{}-Responder", id))
            .spawn(move || clone.responder())
            .expect("node responder thread creation failed");

        ret.find_new();
        ret
    }

    /// Control responder function. Handles receiving MSG_ACK, MSG_ELECTION,
    /// MSG_COORDINATOR and MSG_KILL messages from the control socket.
    /// It uses the ring election algorithm.
    fn responder(&mut self) {
        while !self.stop.load(Ordering::SeqCst) {
            let mut buf = [0; 1 + size_of::<usize>() + (N_NODES + 1) * size_of::<usize>()];
            self.socket
                .set_read_timeout(Some(TIMEOUT / 4))
                .expect("Unable to set read timeout");
            let res = self.socket.recv_from(&mut buf);

            if res.is_err() {
                continue;
            }
            let (_size, from) = res.expect("Unable to get size and from");
            let (msg_type, mut ids) = self.parse_message(&buf);

            match msg_type {
                MSG_ACK => {
                    self.logger.trace(format!("Got ACK from {}", from));
                    *self.got_ack.0.lock().expect("Unable to get stop lock") = Some(ids[0]);
                    self.got_ack.1.notify_all();
                }
                MSG_ELECTION => {
                    self.logger
                        .trace(format!("Got ELECTION from {} with ids {:?}", from, ids));
                    self.socket
                        .send_to(&self.ids_to_msg(MSG_ACK, &[self.id]), from)
                        .expect("Unable to send data");
                    if ids.contains(&self.id) {
                        let winner = *ids.iter().max().expect("Unable to get winner");
                        self.socket
                            .send_to(&self.ids_to_msg(MSG_COORDINATOR, &[winner]), from)
                            .expect("Unable to send data");
                    } else {
                        ids.push(self.id);
                        let msg = self.ids_to_msg(MSG_ELECTION, &ids);
                        let clone = self.clone();

                        thread::Builder::new()
                            .name(format!("Node{}-Sender", self.id))
                            .spawn(move || clone.safe_send_next(&msg, clone.id))
                            .expect("node sender thread creation failed");
                    }
                }
                MSG_COORDINATOR => {
                    self.logger
                        .trace(format!("Got COORDINATOR from {} with ids {:?}", from, ids));
                    *self.leader_id.0.lock().expect("Unable to get lock") = Some(ids[0]);
                    self.leader_id.1.notify_all();
                    self.socket
                        .send_to(&self.ids_to_msg(MSG_ACK, &[self.id]), from)
                        .expect("Unable to send message");
                    self.logger
                        .trace(format!("Sent ACK to {} with ids {:?}", from, ids));
                    if !ids[1..].contains(&self.id) {
                        ids.push(self.id);
                        let msg = self.ids_to_msg(MSG_COORDINATOR, &ids);
                        let clone = self.clone();

                        thread::Builder::new()
                            .name(format!("Node{}-Sender", self.id))
                            .spawn(move || clone.safe_send_next(&msg, clone.id))
                            .expect("node sender thread creation failed");
                    }
                }
                MSG_KILL => {
                    self.logger.info("Got killed".to_string());
                    self.stop();
                    break;
                }
                _ => {
                    self.logger
                        .info(format!("Got unknown message from {}", from));
                }
            }
        }
    }

    /// Parses the array of bytes into the message, returning its type and a
    /// vector of the ids associated with it.
    fn parse_message(&self, buf: &[u8]) -> (u8, Vec<usize>) {
        let mut ids = vec![];

        let count = usize::from_le_bytes(
            buf[1..1 + size_of::<usize>()]
                .try_into()
                .expect("Unable to convert to bytes"),
        );

        let mut pos = 1 + size_of::<usize>();
        for _id in 0..count {
            ids.push(usize::from_le_bytes(
                buf[pos..pos + size_of::<usize>()]
                    .try_into()
                    .expect("Unable to convert to bytes"),
            ));
            pos += size_of::<usize>();
        }

        (buf[0], ids)
    }

    /// Returns an array of bytes representing the message from
    /// the message type and the ids.
    fn ids_to_msg(&self, header: u8, ids: &[usize]) -> Vec<u8> {
        let mut msg = vec![header];
        msg.extend_from_slice(&ids.len().to_le_bytes());
        for id in ids {
            msg.extend_from_slice(&id.to_le_bytes());
        }
        msg
    }

    /// Sends the message to the next node in the ring. It waits for an ACK,
    /// setting the got_ack flag, and if it doesn't receive one it will
    /// try to send the message to the next node in the ring.
    fn safe_send_next(&self, msg: &[u8], id: usize) {
        if self.stop.load(Ordering::SeqCst) {
            return;
        }
        self.logger
            .trace(format!("Running safe_send_next for id {}", id));
        let next_id = self.next(id);
        if next_id == self.id {
            self.logger
                .trace(format!("Sent message {} to {}", msg[0], id));
            panic!("Complete ring, sent message to itself with no response");
        }
        *self.got_ack.0.lock().expect("Unable to get stop lock") = None;
        let _ignore = self.socket.send_to(msg, id_to_ctrladdr(next_id));
        let got_ack = self.got_ack.1.wait_timeout_while(
            self.got_ack.0.lock().expect("Unable to get stop lock"),
            TIMEOUT,
            |got_it| got_it.is_none() || got_it.expect("Unable to get lock value") != next_id,
        );
        if got_ack.expect("Unable to get condvar value").1.timed_out() {
            self.safe_send_next(msg, next_id)
        }
    }

    /// Get the id of next node in the ring.
    fn next(&self, id: usize) -> usize {
        (id + 1) % N_NODES
    }

    /// Start a new leader election sending a new MSG_ELECTION to the next node.
    /// Blocks until a new leader is found.
    fn find_new(&mut self) {
        if self.stop.load(Ordering::SeqCst) {
            return;
        }
        self.logger.info("Looking for new leader".to_string());
        *self.leader_id.0.lock().expect("Unable to get lock") = None;

        self.safe_send_next(&self.ids_to_msg(MSG_ELECTION, &[self.id]), self.id);

        let _ignore = self.leader_id.1.wait_while(
            self.leader_id.0.lock().expect("Unable to get lock"),
            |leader_id| leader_id.is_none(),
        );
    }

    /// Clone the AlgloboNode struct.
    fn clone(&self) -> AlgloboNode {
        AlgloboNode {
            id: self.id,
            socket: self.socket.try_clone().expect("Unable to clone socket"),
            leader_id: self.leader_id.clone(),
            got_ack: self.got_ack.clone(),
            stop: self.stop.clone(),
            last_id: self.last_id.clone(),
            last_status: self.last_status.clone(),
            logger: self.logger.clone(),
        }
    }

    /// Broadcast a message to all agents at the same time, returning
    /// a condvar and a vector of the results.
    fn broadcast(
        &self,
        transaction_id: usize,
        transaction_prices: &[u32],
        operation: u8,
        agents_ports: &[u16],
        im_alive: &Arc<AtomicBool>,
    ) -> (Vec<[u8; 1]>, bool) {
        let responses = Arc::new((Mutex::new(vec![]), Condvar::new()));

        for (i, agent_port) in agents_ports.iter().enumerate() {
            let addr = SocketAddr::from(([127, 0, 0, 1], *agent_port));

            let im_alive_clone = im_alive.clone();
            let logger_clone = self.logger.clone();

            let responses_clone = responses.clone();

            let msg = DataMsg {
                transaction_id: transaction_id as u32,
                opcode: operation,
                data: transaction_prices[i],
            };

            thread::Builder::new()
                .name(format!("Transaction {}", transaction_id))
                .spawn(move || {
                    let client_conn_result = TcpStream::connect(addr);
                    let (lock, cvar) = &*responses_clone;
                    let mut response: [u8; 1] = Default::default();

                    if client_conn_result.is_err() {
                        logger_clone.info("Could not connect to agent".to_string());
                        response[0] = PAYMENT_ERR;
                        lock.lock()
                            .expect("Unable to lock responses")
                            .push(response);
                        cvar.notify_all();
                        return;
                    }

                    let mut client = client_conn_result.expect("Could not connect to agent");
                    client
                        .write_all(&DataMsg::to_bytes(&msg))
                        .unwrap_or_else(|_| {
                            im_alive_clone.store(false, Ordering::SeqCst);
                        });

                    client.read_exact(&mut response).unwrap_or_else(|_| {
                        im_alive_clone.store(false, Ordering::SeqCst);
                    });

                    if !im_alive_clone.load(Ordering::SeqCst) {
                        logger_clone.info("Connection with agent suddenly closed".to_string())
                    }

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
                |responses| responses.len() != agents_ports.len(),
            )
            .expect("Error on wait condvar");

        (all_responses.to_vec(), timeout.timed_out())
    }

    /// Finishes the transaction acording to the results of the broadcast
    /// and its last status obtained.
    fn finish_transaction(
        &self,
        operation: u8,
        transaction_id: usize,
        transaction_prices: &[u32],
        agents_ports: &[u16],
        im_alive: &Arc<AtomicBool>,
        retry_file: &std::fs::File,
    ) {
        self.logger.info(format!(
            "Payment of {:?} | {}",
            transaction_prices,
            if operation == COMMIT { "OK" } else { "ERR" },
        ));
        self.logger.trace(format!(
            "Transaction {} | {}",
            transaction_id,
            if operation == COMMIT {
                "ABORT"
            } else {
                "COMMIT"
            },
        ));
        if operation == ABORT {
            write_to_csv(retry_file, transaction_prices);
        }

        let (_all_responses, _is_timeout) = self.broadcast(
            transaction_id,
            transaction_prices,
            operation,
            agents_ports as &[u16],
            im_alive,
        );

        self.broadcast_last_log(operation, transaction_id);
        *self.last_id.lock().expect("Unable to get lock") += 1;
        *self.last_status.lock().expect("Unable to get lock") = operation;
    }

    /// Function used by the leader for handling the payments. It sends
    /// the payment information in the prices.csv to all the agents and logs
    /// their results.
    /// It will stop processing payments if the stop flag is set to true.
    /// It will send a KILL message to all nodes if all payments finished processing.
    fn process_payments(&self) {
        let im_alive = Arc::new(AtomicBool::new(true));
        let agents_ports = get_agents_ports();

        let prices_file = match env::args().nth(1) {
            Some(val) => val,
            None => "src/prices.csv".to_string(),
        };
        let prices = csv_to_prices(&prices_file);

        let retry_file = create_empty_csv("src/prices-retry.csv");

        let last_status = *self.last_status.lock().expect("Unable to get lock");
        if last_status == PREPARE {
            // If the last transaction was a PREPARE, we need to ABORT it
            let transaction_id = *self.last_id.lock().expect("Unable to get lock");
            self.finish_transaction(
                ABORT,
                transaction_id,
                &prices[transaction_id].clone(),
                &agents_ports as &[u16],
                &im_alive,
                &retry_file,
            );
        } else if last_status == COMMIT || last_status == ABORT {
            // If the last transaction was a COMMIT or ABORT, we need to start a new one
            *self.last_id.lock().expect("Unable to get lock") += 1;
        }

        while *self.last_id.lock().expect("Unable to get lock") < prices.len() {
            if self.stop.load(Ordering::SeqCst) {
                self.logger
                    .trace("Leader stopped before PREPARE msg".to_string());
                return;
            }

            let transaction_id = *self.last_id.lock().expect("Unable to get lock");
            let transaction_prices = prices[transaction_id].clone();

            if !im_alive.load(Ordering::SeqCst) {
                break;
            };

            let im_alive_clone_agents = im_alive.clone();
            self.logger
                .trace(format!("Transaction {} | PREPARE", transaction_id));

            let (all_responses, is_timeout) = self.broadcast(
                transaction_id,
                &transaction_prices,
                PREPARE,
                &agents_ports as &[u16],
                &im_alive_clone_agents,
            );
            self.broadcast_last_log(PREPARE, transaction_id);

            if self.stop.load(Ordering::SeqCst) {
                self.logger
                    .trace("Leader stopped after PREPARE msg".to_string());
                return;
            }

            // Wait for all agents to respond or timeout
            // let all_oks = all_respo
            let all_oks = all_responses.iter().all(|&opt| opt[0] == PAYMENT_OK);

            let operation = if all_oks && !is_timeout && im_alive.load(Ordering::SeqCst) {
                COMMIT
            } else {
                ABORT
            };
            self.finish_transaction(
                operation,
                transaction_id,
                &transaction_prices,
                &agents_ports as &[u16],
                &im_alive,
                &retry_file,
            );
            // This sleep is only for debugging purposes
            sleep(Duration::from_millis(1000));
        }

        self.logger
            .trace("Sending finish command to agents".to_string());
        let dummy_data = vec![0; agents_ports.len()];
        let (_all_responses, _is_timeout) =
            self.broadcast(0, &dummy_data, FINISH, &agents_ports as &[u16], &im_alive);

        self.logger.info("Killing all replicas".to_string());
        for i in 0..N_NODES {
            if i == self.id {
                continue;
            }
            self.socket
                .send_to(&[MSG_KILL], id_to_ctrladdr(i))
                .expect("Unable to send kill");
        }
    }

    /// Sends the status of the last broadcast to all the replicas.
    pub fn broadcast_last_log(&self, status: u8, id: usize) {
        let mut bytes = vec![status];
        bytes.extend(id.to_be_bytes());

        for i in 0..N_NODES {
            if i == self.get_leader_id() {
                continue;
            }
            self.socket
                .send_to(&bytes, id_to_dataaddr(i))
                .expect("Unable to send message");
        }
    }

    /// Main loop function of the node. If it's the leader it will process the
    /// payments file and send each result to the replicas. If it's a replica it
    /// will receive the results and if it doesn't receive one it will start a new
    /// leader election.
    ///
    /// Will loop until the payment file is fully processed or until the stop flag
    /// is set.
    pub fn loop_node(&mut self) {
        self.logger.info("Start".to_string());
        let socket = UdpSocket::bind(id_to_dataaddr(self.id)).expect("Unable to bind socket");

        while !self.stop.load(Ordering::SeqCst) {
            if self.am_i_leader() {
                self.logger.info("I am the leader".to_string());
                let _ignore = socket.set_read_timeout(None);
                self.process_payments();
                break;
            } else {
                let leader_id = self.get_leader_id();
                self.logger
                    .trace(format!("Waiting for message from leader {}", leader_id));

                let mut response: [u8; std::mem::size_of::<usize>() + 1] = Default::default();
                socket
                    .set_read_timeout(Some(TIMEOUT))
                    .expect("Unable to set timeout");

                if let Ok((_size, _from)) = socket.recv_from(&mut response) {
                    *self.last_status.lock().expect("Unable to get lock") = response[0];
                    let id_bytes: [u8; std::mem::size_of::<usize>()] =
                        response[1..].try_into().expect("Incorrect message length");
                    *self.last_id.lock().expect("Unable to get lock") =
                        usize::from_be_bytes(id_bytes) as usize;
                    self.logger.trace(format!(
                        "Received last log: Last status is {} for node {}",
                        response[0], response[1]
                    ));
                } else {
                    self.logger
                        .info("The leader is dead. Long live the leader.".to_string());
                    self.find_new();
                }
            }
        }
    }

    /// Asks itself... am I the leader?
    /// Do I exist?
    /// Is any of this even real?
    /// I'm not quite dead, not quite alive
    /// It's similar to a constant state of sleep paralysis
    fn am_i_leader(&self) -> bool {
        self.get_leader_id() == self.id
    }

    /// Returns the leader id
    fn get_leader_id(&self) -> usize {
        self.leader_id
            .1
            .wait_while(
                self.leader_id.0.lock().expect("Unable to get lock"),
                |leader_id| leader_id.is_none(),
            )
            .expect("Unable to wait for condvar")
            .expect("Unable to get condvar result")
    }

    /// Kills a node, due either to a manual trigger or to a communication timeout
    fn stop(&mut self) {
        self.logger.info("Stop node".to_string());
        self.stop.store(true, Ordering::SeqCst);
    }
}
