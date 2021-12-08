#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::collections::HashMap;
use std::env;
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
use logger::LogLevel;
use logger::Logger;

use std::net::SocketAddr;

use utils::{create_empty_csv, csv_to_prices, get_agents_ports, write_to_csv};

const TIMEOUT: Duration = Duration::from_secs(3);

fn broadcast(
    transaction_id: usize,
    transaction_prices: &[u32],
    operation: u8,
    agent_clients: &[TcpStream],
) -> (Vec<[u8; 1]>, bool) {
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
    let (all_responses, timeout) = cvar
        .wait_timeout_while(
            lock.lock().expect("Unable to lock responses"),
            TIMEOUT,
            |responses| responses.len() != agent_clients.len(),
        )
        .expect("Error on wait condvar");

    (all_responses.to_vec(), timeout.timed_out())
}

fn main() {
    let agents_ports = get_agents_ports();

    let prices_file = match env::args().nth(1) {
        Some(val) => val,
        None => "src/prices.csv".to_string(),
    };
    let prices = csv_to_prices(&prices_file);

    let retry_file = create_empty_csv("src/prices-retry.csv");
    let logger = Logger::new("alglobo".to_string());
    let mut transactions_state: HashMap<u32, u8> = HashMap::new();

    let mut agent_clients = Vec::new();
    for port in &agents_ports {
        let addr = SocketAddr::from(([127, 0, 0, 1], *port));
        let client = TcpStream::connect(addr)
            .unwrap_or_else(|_| panic!("connection with port {} failed", port));
        agent_clients.push(client);
    }

    for (transaction_id, transaction_prices) in prices.iter().enumerate() {
        logger.log(
            format!("Transaction {} | PREPARE", transaction_id),
            LogLevel::TRACE,
        );
        transactions_state.insert(transaction_id as u32, PREPARE);

        let (all_responses, is_timeout) =
            broadcast(transaction_id, transaction_prices, PREPARE, &agent_clients);

        let all_oks = all_responses.iter().all(|&opt| opt[0] == PAYMENT_OK);

        let operation = if all_oks && !is_timeout {
            COMMIT
        } else {
            ABORT
        };
        logger.log(
            format!(
                "Payment of {:?} | {}",
                transaction_prices,
                if operation == COMMIT { "OK" } else { "ERR" },
            ),
            LogLevel::INFO,
        );
        logger.log(
            format!(
                "Transaction {} | {}",
                transaction_id,
                if operation == COMMIT {
                    "COMMIT"
                } else {
                    "ABORT"
                },
            ),
            LogLevel::TRACE,
        );
        transactions_state.insert(transaction_id as u32, operation);

        let (_all_responses, _is_timeout) = broadcast(
            transaction_id,
            transaction_prices,
            operation,
            &agent_clients,
        );

        if operation == ABORT {
            write_to_csv(&retry_file, transaction_prices);
        }

        // This sleep is only for debugging purposes
        sleep(Duration::from_millis(1000));
    }

    let dummy_data = vec![0; agent_clients.len()];
    let (_all_responses, _is_timeout) = broadcast(0, &dummy_data, FINISH, &agent_clients);
}
