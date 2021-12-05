use rand::Rng;
use std::collections::HashMap;
use crate::communication::{ABORT, COMMIT, PREPARE};
use crate::logger::{LogLevel, Logger};

pub struct Agent {
    pub name: String,
    pub port: u16,
    pub success_rate: f64,
    pub logger: Logger,
    transactions_state: HashMap<u32, u8>,
}

impl Clone for Agent {
    fn clone(&self) -> Self {
        Agent {
            name: self.name.clone(),
            port: self.port,
            success_rate: self.success_rate,
            logger: self.logger.clone(),
            transactions_state: self.transactions_state.clone(),
        }
    }
}

impl Agent {
    pub fn new(name: String, port: u16, success_rate: f64) -> Self {
        Agent {
            name: name.clone(),
            port: port,
            success_rate: success_rate,
            logger: Logger::new(name),
            transactions_state: HashMap::new(),
        }
    }

    pub fn log(&self, msg: String, loglevel: LogLevel) {
        self.logger.log(msg, loglevel);
    }

    pub fn prepare(&mut self, transaction_id: u32) -> u8 {
        let success = rand::thread_rng().gen_bool(self.success_rate);
        let msg = if success { "Success" } else { "Failed" };
        self.log(format!("Preparing transaction {} at {}: {}", transaction_id, self.name, msg), LogLevel::INFO);
        self.transactions_state.insert(transaction_id, PREPARE);
        if success {COMMIT} else {ABORT}
    }

    pub fn commit(&mut self, transaction_id: u32) -> u8 {
        self.log(format!("Committing transaction {}", transaction_id), LogLevel::INFO);
        self.transactions_state.insert(transaction_id, COMMIT);
        COMMIT
    }
    pub fn abort(&mut self, transaction_id: u32) -> u8 {
        self.log(format!("Aborting transaction {}", transaction_id), LogLevel::INFO);
        self.transactions_state.insert(transaction_id, ABORT);
        ABORT
    }
}
