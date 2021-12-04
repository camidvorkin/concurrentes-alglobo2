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

fn create_connection(port: u16, agent_name: String, prices: Vec<HashMap<String, u32>>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut stream =
        TcpStream::connect(addr).unwrap_or_else(|_| panic!("connection with port {} failed", port));

    for (i, price) in prices.iter().enumerate() {
        let msg = DataMsg {
            transaction_id: i as u32,
            opcode: PREPARE,
            data: price[&agent_name],
        };

        stream
            .write_all(&data_msg_to_bytes(&msg))
            .expect("write failed");
        
        // TODO: sacar sleep
        sleep(Duration::from_millis(1000));

        let mut response = [0u8; 1];
        stream.read_exact(&mut response).expect("read failed");
        println!("Received {:?}", response);
    }
}




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
        // TODO: que se envien los tres mensajes en paralelo
        responses.0.lock().unwrap().clear();
        for (j, agent) in agents.iter().enumerate() {
            let msg = DataMsg {
                transaction_id: transaction_id as u32,
                opcode: PREPARE,
                data: price[j]
            };

            agent_clients[j].write_all(&data_msg_to_bytes(&msg)).expect("write failed");
            
            // TODO: sacar sleep
            sleep(Duration::from_millis(1000));

            let mut response = [0u8; 1];
            agent_clients[j].read_exact(&mut response).expect("read failed");
            println!("Received {:?}", response);
            responses.0.lock().unwrap().push(Some(response));
        }

        let response = if responses.0.lock().unwrap().iter().all(|opt| opt.is_some() && opt.unwrap()[0] == COMMIT) {COMMIT} else {ABORT};
        for (j, agent) in agents.iter().enumerate() {
            let msg = DataMsg {
                transaction_id: transaction_id as u32,
                opcode: response,
                data: price[j]
            };
            agent_clients[j].write_all(&data_msg_to_bytes(&msg)).expect("write failed");
        }
    }
}
