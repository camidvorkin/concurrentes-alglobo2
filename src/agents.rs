//! AlGlobo Agents - Process payments
//! ---
//! This program sets up the various agents in the agents.yaml config file so that they can be used to process flight payments.
//!
//! Start the program with `cargo run --bin agents`
//!
//! Each agent will be listening on the configured TCP port, and will log and return the transaction states.

#![forbid(unsafe_code)]
#![allow(dead_code)]
mod agent;
mod communication;
pub mod logger;
mod utils;
use agent::Agent;
use communication::{DataMsg, DataMsgBytes, ABORT, COMMIT, FINISH, PREPARE};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread,
};
use utils::{agent_get_name, agent_get_port, agent_get_success_rate, get_agents};

/// Starts the agent killer in a new thread, killing agents via keyboard input
fn psycho_agent_killer(is_agent_alive: Vec<Arc<AtomicBool>>) {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => match line.trim().parse::<usize>() {
                Ok(number) => {
                    if (0..is_agent_alive.len()).contains(&number) {
                        is_agent_alive[number].store(false, Ordering::SeqCst);
                    }
                }
                Err(_) => continue,
            },
            Err(_) => panic!("Failed to read stdin"),
        }
    }
}

/// Constantly listens to TCP connections
/// Handles different 2-phase transaction messages like PREPARE and COMMIT
/// Stops listening on a F
fn create_listener(mut agent: Agent, is_alive: Arc<AtomicBool>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], agent.port));
    let listener = TcpListener::bind(addr)
        .unwrap_or_else(|_| panic!("listener on port {} failed", agent.port));
    listener
        .set_nonblocking(true)
        .expect("Cannot set non-blocking");

    agent.logger.info(format!(
        "Started on port {} with sucess rate {}",
        agent.port, agent.success_rate
    ));

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if !is_alive.load(Ordering::SeqCst) {
                    agent.logger.info("Got killed".to_string());
                    break;
                } else {
                    thread::sleep(Duration::from_millis(300));
                    continue;
                }
            }
            Err(e) => panic!("accept failed: {}", e),
        };

        let mut reader = BufReader::new(stream.try_clone().expect("Couldn't clone stream"));

        let mut buffer: DataMsgBytes = Default::default();

        reader
            .read_exact(&mut buffer)
            .expect("Couldn't read from stream");

        let data_msg = DataMsg::from_bytes(buffer);

        let result = match data_msg.opcode {
            PREPARE => agent.prepare(data_msg.transaction_id, data_msg.data),
            COMMIT => agent.commit(data_msg.transaction_id),
            ABORT => agent.abort(data_msg.transaction_id),
            FINISH => agent.finish(),
            _ => panic!("Unknown opcode"),
        };

        stream
            .write_all(&[result])
            .expect("Couldn't write to stream");

        if data_msg.opcode == FINISH {
            agent.logger.info("Stop".to_string());
            break;
        };

        stream
            .shutdown(std::net::Shutdown::Both)
            .expect("Couldn't shutdown stream");
    }
}

/// Main function. Starts the agents from the .yaml configuration file
/// and the agent killer. Finishes when all the agents are killed.
fn main() {
    let agents = get_agents();

    let mut is_agent_alive = vec![];
    for _ in 0..agents.len() {
        is_agent_alive.push(Arc::new(AtomicBool::new(true)));
    }

    let is_agent_alive_clone = is_agent_alive.clone();

    thread::Builder::new()
        .name("psycho killer".to_string())
        .spawn(move || psycho_agent_killer(is_agent_alive))
        .expect("Couldn't create psycho killer loop");

    let mut agents_threads = vec![];
    for (i, agent) in agents.iter().enumerate() {
        let agent = Agent::new(
            agent_get_name(agent),
            agent_get_port(agent),
            agent_get_success_rate(agent),
        );

        let is_alive = is_agent_alive_clone[i].clone();
        agents_threads.push(
            thread::Builder::new()
                .name(agent.name.clone())
                .spawn(move || {
                    create_listener(agent, is_alive);
                })
                .expect("agent thread creation failed"),
        )
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }
}
