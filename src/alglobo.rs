use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;
use std::sync::{Arc, Condvar, Mutex};

mod communication;
pub mod logger;
mod utils;

use communication::{data_msg_to_bytes, DataMsg, PREPARE, COMMIT, ABORT};

use std::{
    net::SocketAddr,
    thread::{self},
};
use utils::get_agents;

use utils::{agent_get_name, agent_get_port, csv_to_prices};

const TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    // TODO: Make get_agents() return a vector of agents instead of a serde Sequence
    // We have to do everything in this function just to not import Sequence from sede
    let agents = get_agents();
    let agents_clone = agents.clone();
    let n_agents = agents.len();
    let responses = Arc::new(( Mutex::new(vec![None; n_agents]), Condvar::new()));

    let prices = csv_to_prices("src/prices.csv", &agents);

    let mut agent_clients: Vec<TcpStream> = Vec::new();
    for agent in agents_clone.iter() {
        let port = agent_get_port(&agent);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let client = TcpStream::connect(addr).unwrap_or_else(|_| panic!("connection with port {} failed", port));
        agent_clients.push(client);
    }

    for (transaction_id, price) in prices.iter().enumerate() {
        for (j, agent) in agents.iter().enumerate() {
            let p = price[j];

            let mut agent_client = agent_clients[j].try_clone().expect("Could not clone agent client");
            let responses_clone = responses.clone();

            // TODO: Handle thread response. Maybe do a join instead of using a condvar?
            let _ = thread::Builder::new()
                .name(format!("{} - Transaction {}", agent_get_name(agent), transaction_id))
                .spawn(move || {
                    let msg = DataMsg {
                        transaction_id: transaction_id as u32,
                        opcode: PREPARE,
                        data: p
                    };

                    agent_client.write_all(&data_msg_to_bytes(&msg)).expect("write failed");
                    
                    // TODO: sacar sleep
                    sleep(Duration::from_millis(1000));

                    let mut response = [0u8; 1];
                    agent_client.read_exact(&mut response).expect("read failed");
                    println!("Received {:?}", response);
                    {
                        responses_clone.0.lock().unwrap().push(Some(response));
                    }
                });
        }
        // responses.1.wait_timeout_while(self.responses.0.lock().unwrap(), TIMEOUT, |responses| responses.iter().any(Option::is_none));
        let response = if responses.0.lock().unwrap().iter().all(|opt| opt.is_some() && opt.unwrap()[0] == COMMIT) {COMMIT} else {ABORT};
        for (j, _agent) in agents.iter().enumerate() {
            let msg = DataMsg {
                transaction_id: transaction_id as u32,
                opcode: response,
                data: price[j]
            };
            agent_clients[j].write_all(&data_msg_to_bytes(&msg)).expect("write failed");
            let mut response = [0u8; 1];
            agent_clients[j].read_exact(&mut response).expect("read failed");
        }
    }
}
