mod agent;
mod communication;
pub mod logger;
mod utils;
use agent::Agent;
use communication::{data_msg_from_bytes, DataMsgBytes, ABORT, COMMIT, PREPARE};
use std::io::Write;
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread::{self},
};
use utils::{agent_get_name, agent_get_port, agent_get_success_rate, get_agents};

fn create_listener(agent: Agent) {
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

    for stream in listener.incoming() {
        let mut stream = stream.expect("failed to read stream");
        let peer = stream
            .peer_addr()
            .expect("Couldn't read connection peer addr");
        let mut reader = BufReader::new(stream.try_clone().expect("Couldn't clone stream"));

        let agent_clone = agent.clone();

        thread::Builder::new()
            .name(format!("{} - {}", agent_clone.name, peer.port()))
            .spawn(move || loop {
                let mut buffer: DataMsgBytes = [0; 9];
                reader
                    .read_exact(&mut buffer)
                    .expect("Couldn't read from stream");

                let data_msg = data_msg_from_bytes(buffer);

                agent_clone.log(
                    format!("Received {:?} from {}", data_msg, peer),
                    logger::LogLevel::INFO,
                );

                let result = match data_msg.opcode {
                    PREPARE => agent_clone.prepare(),
                    COMMIT => agent_clone.commit(),
                    ABORT => agent_clone.abort(),
                    _ => panic!("Unknown opcode"),
                };

                stream
                    .write_all(&[result])
                    .expect("Couldn't write to stream");
            })
            .expect("agent connection thread creation failed");
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
