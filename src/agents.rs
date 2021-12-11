#![forbid(unsafe_code)]
#![allow(dead_code)]
mod agent;
mod communication;
pub mod logger;
mod utils;
use agent::Agent;
use communication::{DataMsg, DataMsgBytes, ABORT, COMMIT, FINISH, PREPARE};
use std::io::Write;
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread,
};
use utils::{agent_get_name, agent_get_port, agent_get_success_rate, get_agents};

fn create_listener(mut agent: Agent) {
    let addr = SocketAddr::from(([127, 0, 0, 1], agent.port));
    let listener = TcpListener::bind(addr)
        .unwrap_or_else(|_| panic!("listener on port {} failed", agent.port));

    agent.log(
        format!(
            "Started on port {} with sucess rate {}",
            agent.port, agent.success_rate
        ),
        logger::LogLevel::INFO,
    );

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
    }
}

fn main() {
    let agents = get_agents();

    let mut agents_threads = vec![];
    for agent in agents {
        let agent = Agent::new(
            agent_get_name(&agent),
            agent_get_port(&agent),
            agent_get_success_rate(&agent),
        );

        agents_threads.push(
            thread::Builder::new()
                .name(agent.name.clone())
                .spawn(move || {
                    create_listener(agent);
                })
                .expect("agent thread creation failed"),
        )
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }
}
