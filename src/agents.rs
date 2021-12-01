mod agent;
pub mod logger;
mod utils;
use agent::Agent;
use rand::Rng;
use serde_yaml::{self, Sequence};
use std::io::Write;
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread::{self},
};
use utils::{agent_get_name, agent_get_port, agent_get_success_rate, data_msg_from_bytes, DataMsg};

fn create_listener(agent: Agent) {
    let addr = SocketAddr::from(([127, 0, 0, 1], agent.get_port()));
    let listener = TcpListener::bind(addr)
        .unwrap_or_else(|_| panic!("listener on port {} failed", agent.get_port()));

    agent.log(
        format!(
            "Started {} on port {} with sucess rate {}",
            agent.get_name(),
            agent.get_port(),
            agent.get_success_rate()
        ),
        logger::LogLevel::INFO,
    );

    for stream in listener.incoming() {
        let mut stream = stream.expect("failed to read stream");
        let peer = stream
            .peer_addr()
            .expect("Couldn't read connection peer addr");
        let mut reader = BufReader::new(stream.try_clone().expect("Couldn't clone stream"));

        let name = agent.get_name();
        let success_rate = agent.get_success_rate();
        let agent_clone = agent.clone();

        thread::Builder::new()
            .name(format!("{} - {}", name, peer.port()))
            .spawn(move || loop {
                let mut buffer: [u8; 9] = [0; 9];
                reader
                    .read_exact(&mut buffer)
                    .expect("Couldn't read from stream");

                let data_msg: DataMsg = data_msg_from_bytes(buffer);

                let result: bool = rand::thread_rng().gen_bool(success_rate);
                agent_clone.log(
                    format!(
                        "{} | {} | {} | {}",
                        data_msg.transaction_id,
                        data_msg.opcode,
                        data_msg.data,
                        if result { "OK" } else { "ERR" }
                    ),
                    logger::LogLevel::INFO,
                );

                stream
                    .write_all(&[if result { 1 } else { 0 }])
                    .expect("Couldn't write to stream");
            })
            .expect("agent connection thread creation failed");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agents_config =
        std::fs::File::open("src/agents.yaml").expect("Couldn't open agents config file");
    let agents: Sequence =
        serde_yaml::from_reader(agents_config).expect("Couldn't parse agents config yaml");

    let mut agents_threads = vec![];
    for agent in agents {
        let a = Agent::new(
            agent_get_name(&agent),
            agent_get_port(&agent),
            agent_get_success_rate(&agent),
        );

        agents_threads.push(
            thread::Builder::new()
                .name(a.get_name())
                .spawn(move || {
                    create_listener(a);
                })
                .expect("agent thread creation failed"),
        )
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }

    Ok(())
}
