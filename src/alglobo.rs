#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::io::BufRead;
use std::thread;
use std::time::Duration;
use std::{io, net::UdpSocket};

mod communication;
mod constants;
mod leader_election;
pub mod logger;
mod utils;

use constants::{MSG_KILL, N_NODES};
use leader_election::{id_to_ctrladdr, LeaderElection};

pub const TIMEOUT: Duration = Duration::from_secs(5);

fn psycho_node_killer() {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => match line.trim().parse::<usize>() {
                Ok(number) => {
                    if (0..N_NODES).contains(&number) {
                        let addr = id_to_ctrladdr(number);
                        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
                        socket
                            .send_to(&[MSG_KILL], addr)
                            .expect("Couldn't send KILL message");
                    }
                }
                Err(_) => continue,
            },
            Err(_) => panic!("Failed to read stdin"),
        }
    }
}

fn main() {
    thread::Builder::new()
        .name("psycho killer".to_string())
        .spawn(psycho_node_killer)
        .expect("Couldn't create psycho killer loop");

    let mut node_threads = vec![];

    for id in 0..N_NODES {
        node_threads.push(
            thread::Builder::new()
                .name(format!("Alglobo Node {}", id))
                .spawn(move || {
                    let mut node = LeaderElection::new(id);
                    node.loop_node()
                })
                .expect("alglobo node thread creation failed"),
        );
    }

    for thread in node_threads {
        thread.join().expect("alglobo node thread join failed");
    }
}
