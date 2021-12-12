#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::thread;
use std::time::Duration;
use std::net::UdpSocket;
use rand::{thread_rng, Rng};

mod communication;
mod leader_election;
pub mod logger;
mod constants;
mod utils;

use leader_election::{LeaderElection, id_to_ctrladdr};
use constants::{N_NODES, MSG_KILL};

pub const TIMEOUT: Duration = Duration::from_secs(5);

fn main() {
    let _listener = thread::Builder::new()
        .name("Listen terminal for nodes".to_string())
        .spawn(move || {
            // TODO: Replace this with a keyboard listener.
            // Let the user choose the ID of the node to kill.
            loop {
                thread::sleep(TIMEOUT);
                let addr = id_to_ctrladdr(rand::thread_rng().gen_range(2, N_NODES + 1));
                let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
                let _ignore = socket.send_to(&[MSG_KILL], addr);
            }
    });

    let mut handles = vec![];

    for id in 0..N_NODES {
        handles.push(
            thread::Builder::new()
                .name("Leader election".to_string())
                .spawn(move || {
                    let mut node = LeaderElection::new(id);
                    node.loop_node()
                }),
        );
    }
    handles.into_iter().for_each(|h| {
        let _ignore = h.unwrap().join();
    });
}
