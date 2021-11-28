pub mod logger;
mod agent;
mod utils;
use rand::Rng;
use serde_yaml::{self, Sequence};
use std::{
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread::{self},
};
use agent::Agent;
use utils::{agent_get_name, agent_get_port, agent_get_success_rate};


fn create_listener(agent: Agent) {
    let addr = SocketAddr::from(([127, 0, 0, 1], agent.get_port()));
    let listener =
        TcpListener::bind(addr).unwrap_or_else(|_| panic!("listener on port {} failed", agent.get_port()));

    agent.log(
        format!(
            "Started {} on port {} with sucess rate {}",
            agent.get_name(), agent.get_port(), agent.get_success_rate()
        ),
        logger::LogLevel::INFO,
    );

    for stream in listener.incoming() {
        let stream = stream.expect("failed to read stream");

        let peer = stream
            .peer_addr()
            .expect("Couldn't read connection peer addr");
        let mut reader = BufReader::new(stream);
        
        let name = agent.get_name();
        let success_rate = agent.get_success_rate();
        let agent_clone = agent.clone();
        thread::Builder::new()
            .name(format!("{} - {}", name, peer.port()))
            .spawn(move || loop {
                let mut buffer: [u8; 4] = [0; 4];
                reader
                    .read_exact(&mut buffer)
                    .expect("Couldn't read from stream");

                agent_clone.log(
                    format!(
                        "Received {} and the operation {}",
                        u32::from_be_bytes(buffer),
                        if rand::thread_rng().gen_bool(success_rate) { "succeeded" } else { "failed" }
                    ),
                    logger::LogLevel::INFO,
                );
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
        let a = Agent::new(agent_get_name(&agent), agent_get_port(&agent), agent_get_success_rate(&agent));
        
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
