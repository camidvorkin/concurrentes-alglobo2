#![forbid(unsafe_code)]
#![allow(dead_code)]
use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

mod communication;
pub mod logger;
mod lider_election;
mod utils;

use communication::FINISH;
use communication::{DataMsg, ABORT, COMMIT, PAYMENT_OK, PREPARE};
use lider_election::LeaderElection;
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
    im_alive: &Arc<AtomicBool>,
    logger: &Logger,
) -> (Vec<[u8; 1]>, bool) {
    let responses = Arc::new((Mutex::new(vec![]), Condvar::new()));

    for (i, agent_client) in agent_clients.iter().enumerate() {
        let im_alive_clone = im_alive.clone();
        let logger_clone = logger.clone();

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
                    .unwrap_or_else(|_| {
                        im_alive_clone.store(false, Ordering::SeqCst);
                    });

                let mut response: [u8; 1] = Default::default();
                agent_client_clone
                    .read_exact(&mut response)
                    .unwrap_or_else(|_| {
                        im_alive_clone.store(false, Ordering::SeqCst);
                    });

                if !im_alive_clone.load(Ordering::SeqCst) {
                    logger_clone.log(
                        "Connection with agent suddenly closed".to_string(),
                        LogLevel::INFO,
                    )
                }

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


fn process_payments() {

    let im_alive = Arc::new(AtomicBool::new(true));
    let im_alive_clone_ctrlc = im_alive.clone();

    ctrlc::set_handler(move || {
        im_alive_clone_ctrlc.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

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
        if !im_alive.load(Ordering::SeqCst) {
            break;
        };

        let im_alive_clone_agents = im_alive.clone();
        let logger_clone = logger.clone();

        logger.log(
            format!("Transaction {} | PREPARE", transaction_id),
            LogLevel::TRACE,
        );
        transactions_state.insert(transaction_id as u32, PREPARE);

        let (all_responses, is_timeout) = broadcast(
            transaction_id,
            transaction_prices,
            PREPARE,
            &agent_clients,
            &im_alive_clone_agents,
            &logger_clone,
        );

        let all_oks = all_responses.iter().all(|&opt| opt[0] == PAYMENT_OK);

        let operation = if all_oks && !is_timeout && im_alive.load(Ordering::SeqCst) {
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

        if operation == ABORT {
            write_to_csv(&retry_file, transaction_prices);
        }

        let (_all_responses, _is_timeout) = broadcast(
            transaction_id,
            transaction_prices,
            operation,
            &agent_clients,
            &im_alive_clone_agents,
            &logger_clone,
        );

        // This sleep is only for debugging purposes
        sleep(Duration::from_millis(1000));
    }

    let dummy_data = vec![0; agent_clients.len()];
    let (_all_responses, _is_timeout) =
        broadcast(0, &dummy_data, FINISH, &agent_clients, &im_alive, &logger);
}


fn main() {

    // TODO: Listener
    // thread::Builder::new()
    //         .name("Listen terminal for nodes".to_string())
    //         .spawn(move || {
                
    //         });

    let n_nodes = 5;
    for id in 0..n_nodes {
        thread::Builder::new()
            .name("Leader election".to_string())
            .spawn(move || {
                let node = LeaderElection::new(id);
            });
    }



}
