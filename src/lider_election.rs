use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write, Cursor, Read};
use std::mem::size_of;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use std::io::Read;
use std::io::Write;

use rand::{Rng, thread_rng};
use std::convert::TryInto;

fn id_to_ctrladdr(id: usize) -> String { "127.0.0.1:1234".to_owned() + &*id.to_string() }
fn id_to_dataaddr(id: usize) -> String { "127.0.0.1:1235".to_owned() + &*id.to_string() }

const TIMEOUT: Duration = Duration::from_secs(5);

const MSG_ELECTION: u8 = b'E';
const MSG_COORDINATOR: u8 = b'C';

pub struct LeaderElection {
    id: usize,
    socket: TcpStream,
    leader_id: Arc<(Mutex<Option<usize>>, Condvar)>,
    stop: Arc<(Mutex<bool>, Condvar)>,
}

impl LeaderElection {
    pub fn new(id: usize) -> LeaderElection {
        let mut ret = LeaderElection {
            id,
            socket: TcpStream::connect(id_to_ctrladdr(id)).unwrap(),
            leader_id: Arc::new((Mutex::new(Some(id)), Condvar::new())),
            stop: Arc::new((Mutex::new(false), Condvar::new()))
        };

        let mut clone = ret.clone();
        thread::spawn(move || clone.responder());

        ret.find_new();
        ret
    }

    fn responder(&mut self) {
        while !*self.stop.0.lock().unwrap() {
            let mut buf = [0; 1 + size_of::<usize>() + (6) * size_of::<usize>()]; // TODO: no hardcoded values
            // let (size, from) = self.socket.read_exact(&mut buf).unwrap();
            self.socket.read_exact(&mut buf).unwrap();
            let (msg_type, mut ids) = self.parse_message(&buf);

            match msg_type {
                MSG_ELECTION => {
                    println!("[{}] recibí Election de {}, ids {:?}", self.id, from, ids);
                    if ids.contains(&self.id) {
                        let new_coordinator = *ids.iter().max().unwrap();
                        self.socket.write_all(&self.ids_to_msg(MSG_COORDINATOR, &[new_coordinator, self.id]), from).unwrap();
                    } else {
                        ids.push(self.id);
                        let msg = self.ids_to_msg(MSG_ELECTION, &ids);
                        let clone = self.clone();
                        thread::spawn(move || clone.safe_send_next(&msg, clone.id));
                    }
                }
                MSG_COORDINATOR => {
                    println!("[{}] recibí nuevo coordinador de {}, ids {:?}", self.id, from, ids);
                    *self.leader_id.0.lock().unwrap() = Some(ids[0]);
                    self.leader_id.1.notify_all();
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
        let mut ids = vec!();

        let mut count = usize::from_le_bytes(buf[1..1+size_of::<usize>()].try_into().unwrap());

        let mut pos = 1+size_of::<usize>();
        for id in 0..count {
            ids.push(usize::from_le_bytes(buf[pos..pos+size_of::<usize>()].try_into().unwrap()));
            pos += size_of::<usize>();
        }

        (buf[0], ids)
    }

    fn ids_to_msg(&self, header:u8, ids:&[usize]) -> Vec<u8> {
        let mut msg = vec!(header);
        msg.extend_from_slice(&ids.len().to_le_bytes());
        for id in ids {
            msg.extend_from_slice(&id.to_le_bytes());
        }
        msg
    }

    fn safe_send_next(&self, msg: &[u8], id:usize) {
        let next_id = self.next(id); 
        if next_id == self.id { 
            println!("[{}] enviando {} a {}", self.id, msg[0] as char, next_id);
            panic!("Di toda la vuelta sin respuestas")
        }
        self.socket.send_to(msg, id_to_ctrladdr(next_id));
    }

    fn next(&self, id:usize) -> usize {
        (id + 1) % 5 // TODO: team members dynamic?
    }

    fn find_new(&mut self) {
        if *self.stop.0.lock().unwrap() {
            return
        }
        println!("[{}] buscando lider", self.id);
        *self.leader_id.0.lock().unwrap() = None;

        self.safe_send_next(&self.ids_to_msg(MSG_ELECTION , &[self.id]), self.id);

        self.leader_id.1.wait_while(self.leader_id.0.lock().unwrap(), |leader_id| leader_id.is_none());
    }

    fn clone(&self) -> LeaderElection {
        LeaderElection {
            id: self.id,
            socket: self.socket.try_clone().unwrap(),
            leader_id: self.leader_id.clone(),
            stop: self.stop.clone(),
        }
    }
}