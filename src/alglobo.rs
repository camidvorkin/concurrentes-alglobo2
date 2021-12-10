#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::thread;
use std::time::Duration;

mod communication;
pub mod logger;
mod leader_election;
mod utils;

use leader_election::LeaderElection;

const TIMEOUT: Duration = Duration::from_secs(3);


fn main() {

    // TODO: Listener
    // thread::Builder::new()
    //         .name("Listen terminal for nodes".to_string())
    //         .spawn(move || {
                
    //         });

    let n_nodes = 5;
    let mut handles = vec!();

    for id in 0..n_nodes {
        handles.push(thread::Builder::new()
            .name("Leader election".to_string())
            .spawn(move || {
                let mut node = LeaderElection::new(id);
                node.loop_node() 
            }));
    }
    handles.into_iter().for_each(|h| { let _ignore = h.unwrap().join(); });
}
