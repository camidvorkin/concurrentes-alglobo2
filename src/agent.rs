use crate::communication::{ABORT, ACK, COMMIT, PAYMENT_ERR, PAYMENT_OK, PREPARE};
use crate::logger::Logger;
use rand::Rng;
use std::collections::HashMap;

pub struct Agent {
    pub name: String,
    pub port: u16,
    pub success_rate: f64,
    pub logger: Logger,
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

    pub fn commit(&mut self, transaction_id: u32) -> u8 {
        self.logger
            .trace(format!("Transaction {} | COMMIT", transaction_id));
        self.transactions_state.insert(transaction_id, COMMIT);
        ACK
    }

    pub fn abort(&mut self, transaction_id: u32) -> u8 {
        self.logger
            .trace(format!("Transaction {} | ABORT", transaction_id));
        self.transactions_state.insert(transaction_id, ABORT);
        ACK
    }

    pub fn finish(&mut self) -> u8 {
        ACK
    }
}
