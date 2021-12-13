#![forbid(unsafe_code)]
#![allow(dead_code)]
mod agent;
mod communication;
pub mod logger;
mod utils;
use agent::Agent;
use communication::{DataMsg, DataMsgBytes, ABORT, COMMIT, FINISH, PREPARE};
use rand::Rng;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread,
};
use utils::{agent_get_name, agent_get_port, agent_get_success_rate, get_agents};

pub const TIMEOUT: Duration = Duration::from_secs(30);

fn create_listener(mut agent: Agent, is_alive: Arc<AtomicBool>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], agent.port));
    let listener = TcpListener::bind(addr)
        .unwrap_or_else(|_| panic!("listener on port {} failed", agent.port));

    agent.logger.info(format!(
        "Started on port {} with sucess rate {}",
        agent.port, agent.success_rate
    ));
    'listener: for stream in listener.incoming() {
        let mut stream = stream.as_ref().expect("failed to read stream");
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
            break 'listener;
        };

        stream
            .shutdown(std::net::Shutdown::Both)
            .expect("Couldn't shutdown stream");

        // TODO: this kills the agent only after handling the next request.
        // and it should be before the next loop (google how to handle incoming
        // requests).
        if !is_alive.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn main() {
    let agents = get_agents();

    let is_agent_alive = vec![
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(true)),
    ];
    let is_agent_alive_clone = is_agent_alive.clone();
    let _ = thread::Builder::new()
        .name("Agents".to_string())
        .spawn(move || {
            loop {
                // TODO: Listen to A, H or B for shutting down the according agent
                thread::sleep(TIMEOUT);
                let rand = rand::thread_rng().gen_range(0, 3);
                is_agent_alive[rand].store(false, Ordering::SeqCst);
                println!("Agent {} is dead", rand);
            }
        });

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
