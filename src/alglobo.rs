#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

mod communication;
pub mod logger;
mod utils;

use communication::FINISH;
use communication::{DataMsg, ABORT, COMMIT, PAYMENT_OK, PREPARE};

use std::net::SocketAddr;

use utils::{csv_to_prices, get_agents_ports};

const _TIMEOUT: Duration = Duration::from_secs(10);

fn broadcast(
    transaction_id: usize,
    transaction_prices: &[u32],
    operation: u8,
    agent_clients: &[TcpStream],
) -> Arc<(Mutex<Vec<[u8; 1]>>, Condvar)> {
    let responses = Arc::new((Mutex::new(vec![]), Condvar::new()));

    for (i, agent_client) in agent_clients.iter().enumerate() {
        let mut agent_client_clone = agent_client
            .try_clone()
            .expect("Could not clone agent client");
        let responses_clone = responses.clone();

        let msg = DataMsg {
            transaction_id: transaction_id as u32,
            opcode: operation,
            data: transaction_prices[i],
        };

        thread::Builder::new()
            .name(format!("Transaction {}", transaction_id))
            .spawn(move || {
                agent_client_clone
                    .write_all(&DataMsg::to_bytes(&msg))
                    .expect("write failed");

                let mut response: [u8; 1] = Default::default();
                agent_client_clone
                    .read_exact(&mut response)
                    .expect("read failed");

                let (lock, cvar) = &*responses_clone;
                lock.lock()
                    .expect("Unable to lock responses")
                    .push(response);

                cvar.notify_all();
            })
            .expect("thread creation failed");
    }

    let (lock, cvar) = &*responses;

    let _ = cvar
        .wait_while(
            lock.lock().expect("Unable to lock responses"),
            |responses| responses.len() != agent_clients.len(),
        )
        .expect("Error on wait condvar");

    responses
}

fn main() {
    let agents_ports = get_agents_ports();
    let prices = csv_to_prices("src/prices.csv");

    let mut agent_clients = Vec::new();
    for port in &agents_ports {
        let addr = SocketAddr::from(([127, 0, 0, 1], *port));
        let client = TcpStream::connect(addr)
            .unwrap_or_else(|_| panic!("connection with port {} failed", port));
        agent_clients.push(client);
    }

    for (transaction_id, transaction_prices) in prices.iter().enumerate() {
        let (lock, _cvar) =
            &*broadcast(transaction_id, transaction_prices, PREPARE, &agent_clients);

        let all_oks = lock
            .lock()
            .expect("Unable to lock responses")
            .iter()
            .all(|&opt| opt[0] == PAYMENT_OK);

        let operation = if all_oks { COMMIT } else { ABORT };

        let (_lock, _cvar) = &*broadcast(
            transaction_id,
            transaction_prices,
            operation,
            &agent_clients,
        );

        // This sleep is only for debugging purposes
        sleep(Duration::from_millis(1000));
    }

    let dummy_data = vec![0; agent_clients.len()];
    let (_lock, _cvar) = &*broadcast(0, &dummy_data, FINISH, &agent_clients);
}
