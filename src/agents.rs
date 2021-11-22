pub mod logger;
use rand::Rng;
use serde_yaml::{self, Sequence};
use std::{
    convert::TryInto,
    io::{BufReader, Read},
    net::{SocketAddr, TcpListener},
    thread::{self},
};

fn create_listener(name: String, port: u16, successrate: f64) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener =
        TcpListener::bind(addr).unwrap_or_else(|_| panic!("listener on port {} failed", port));

    logger::log(
        format!(
            "Started {} on port {} with sucess rate {}",
            name, port, successrate
        ),
        logger::LogLevel::INFO,
    );

    for stream in listener.incoming() {
        let stream = stream.expect("failed to read stream");

        let peer = stream
            .peer_addr()
            .expect("Couldn't read connection peer addr");
        let mut reader = BufReader::new(stream);

        thread::Builder::new()
            .name(format!("{} - {}", name, peer.port()))
            .spawn(move || loop {
                let mut buffer: [u8; 4] = [0; 4];
                reader
                    .read_exact(&mut buffer)
                    .expect("Couldn't read from stream");

                println!(
                    "{}:{}",
                    u32::from_be_bytes(buffer),
                    rand::thread_rng().gen_bool(successrate)
                );
            })
            .expect("agent connection thread creation failed");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logger::init();
    let agents_config =
        std::fs::File::open("src/agents.yaml").expect("Couldn't open agents config file");
    let agents: Sequence =
        serde_yaml::from_reader(agents_config).expect("Couldn't parse agents config yaml");

    let mut agents_threads = vec![];
    for agent in agents {
        let name = agent["name"]
            .as_str()
            .expect("Agent name must be a string")
            .to_string();

        let port = agent["port"]
            .as_u64()
            .expect("Agent port must be an unsigned integer")
            .try_into()
            .expect("Agent port must be a valid port number");

        let successrate = agent["successrate"]
            .as_f64()
            .expect("Agent successrate must be a float");

        agents_threads.push(
            thread::Builder::new()
                .name(name.clone())
                .spawn(move || {
                    create_listener(name, port, successrate);
                })
                .expect("agent thread creation failed"),
        )
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }

    Ok(())
}
