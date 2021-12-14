//! Agent Struct
//!
//! Used for handling the main logic of each agent
use crate::communication::{ABORT, ACK, COMMIT, PAYMENT_ERR, PAYMENT_OK, PREPARE};
use crate::logger::Logger;
use rand::Rng;
use std::collections::HashMap;

/// Agent Struct
pub struct Agent {
    /// Name of the agent used for logging purposes
    pub name: String,
    /// TCP port used by the agent
    pub port: u16,
    /// Success rate of each request sent to the agent
    pub success_rate: f64,
    /// Logger used by the agent
    pub logger: Logger,
    /// All transaction states handled by the agent
    transactions_state: HashMap<u32, u8>,
}

impl Agent {
    pub fn new(name: String, port: u16, success_rate: f64) -> Self {
        Agent {
            name: name.clone(),
            port,
            success_rate,
            logger: Logger::new(name),
            transactions_state: HashMap::new(),
        }
    }

    /// Handles the PREPARE phase, simulating the transaction result
    /// and printing the result to the logger.
    /// Returns PAYMENT_OK if the transaction was successful and PAYMENT_ERR otherwise
    pub fn prepare(&mut self, transaction_id: u32, data: u32) -> u8 {
        self.logger
            .trace(format!("Transaction {} | PREPARE", transaction_id));
        self.transactions_state.insert(transaction_id, PREPARE);

        let success = rand::thread_rng().gen_bool(self.success_rate);
        if success {
            self.logger.info(format!("Payment of ${} | OK", data));
            PAYMENT_OK
        } else {
            self.logger.info(format!("Payment of ${} | ERR", data));
            PAYMENT_ERR
        }
    }

    /// Handles the COMMIT phase, logging the transaction and
    /// adding the state to the states HashMap. Returns ACK
    pub fn commit(&mut self, transaction_id: u32) -> u8 {
        self.logger
            .trace(format!("Transaction {} | COMMIT", transaction_id));
        self.transactions_state.insert(transaction_id, COMMIT);
        ACK
    }

    /// Handles the ABORT phase, logging the transaction and
    /// adding the state to the states HashMap. Returns ACK
    pub fn abort(&mut self, transaction_id: u32) -> u8 {
        self.logger
            .trace(format!("Transaction {} | ABORT", transaction_id));
        self.transactions_state.insert(transaction_id, ABORT);
        ACK
    }

    /// Returns ACK
    pub fn finish(&mut self) -> u8 {
        ACK
    }
}
