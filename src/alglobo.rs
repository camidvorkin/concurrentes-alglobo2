use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;

pub mod logger;
mod utils;
use serde_yaml::{self, Sequence};
use std::{
    net::SocketAddr,
    thread::{self},
};
use utils::data_msg_to_bytes;
use utils::{agent_get_name, agent_get_port, get_prices, DataMsg, TransactionPrepare};

fn create_connection(port: u16, agent_name: String, prices: Vec<HashMap<String, u32>>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut stream =
        TcpStream::connect(addr).unwrap_or_else(|_| panic!("connection with port {} failed", port));

    for (i, price) in prices.iter().enumerate() {
        let x = DataMsg {
            transaction_id: i as u32,
            opcode: TransactionPrepare,
            data: price[&agent_name] as u32,
        };

        stream
            .write_all(&data_msg_to_bytes(&x))
            .expect("write failed");
        sleep(Duration::from_millis(1000));

        // Read from the connection
        let mut buf = [0; 1];
        stream.read(&mut buf).expect("read failed");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agents_config = File::open("src/agents.yaml").expect("Couldn't open agents config file");
    let agents: Sequence =
        serde_yaml::from_reader(agents_config).expect("Couldn't parse agents config yaml");
    let prices = get_prices("src/prices.csv", agents.clone())?;

    let mut agents_threads = vec![];
    for agent in agents {
        let port: u16 = agent_get_port(&agent);
        let agent_name: String = agent_get_name(&agent);
        let prices_clone = prices.clone();

        agents_threads.push(
            thread::Builder::new()
                .name(port.to_string().clone())
                .spawn(move || {
                    create_connection(port, agent_name, prices_clone);
                })
                .expect("agent connection thread creation failed"),
        );
    }

    for thread in agents_threads {
        thread.join().expect("agent thread join failed");
    }

    Ok(())
}
