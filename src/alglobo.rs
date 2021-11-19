use std::io::Write;
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;

pub mod logger;
use serde_yaml::{self, Sequence};
use std::{
    convert::TryInto,
    net::SocketAddr,
    thread::{self},
};

fn create_connection(port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut stream =
        TcpStream::connect(addr).unwrap_or_else(|_| panic!("connection with port {} failed", port));

    loop {
        stream.write_all("100\n".as_bytes()).expect("write failed");
        sleep(Duration::from_millis(1000));
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
        let port: u16 = agent["port"]
            .as_u64()
            .expect("Agent port must be an unsigned integer")
            .try_into()
            .expect("Agent port must be a valid port number");

        agents_threads.push(
            thread::Builder::new()
                .name(port.to_string().clone())
                .spawn(move || {
                    create_connection(port);
                })
                .expect("agent connection thread creation failed"),
        )
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }

    Ok(())
}
