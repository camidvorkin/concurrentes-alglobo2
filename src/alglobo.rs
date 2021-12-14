//! AlGlobo.com - Process payments
//! ---
//! This program sets up `N_NODES` which will process all of the payments from
//! the payments csv file. It will send the request to all the designated agents,
//! using the configuration in the agents.yaml file.
//!
//! If the leader node is killed, another one is elected using the ring election
//! agorithm.
//!
//! Start the program with `cargo run --bin alglobo <payments_file>.csv` (or
//! default to a csv if not provided)

#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::io::BufRead;
use std::thread;
use std::{io, net::UdpSocket};

mod alglobo_node;
mod communication;
pub mod logger;
mod utils;

use alglobo_node::{id_to_ctrladdr, AlgloboNode, MSG_KILL, N_NODES};

/// Starts the thread designated to kill each node via keyboard input.
fn psycho_node_killer() {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => match line.trim().parse::<usize>() {
                Ok(number) => {
                    if (0..N_NODES).contains(&number) {
                        let addr = id_to_ctrladdr(number);
                        let socket =
                            UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to address");
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

/// Starts the main process, starting the node killer and each node process
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
                    let mut node = AlgloboNode::new(id);
                    node.loop_node()
                })
                .expect("alglobo node thread creation failed"),
        );
    }

    for thread in node_threads {
        thread.join().expect("alglobo node thread join failed");
    }
}
