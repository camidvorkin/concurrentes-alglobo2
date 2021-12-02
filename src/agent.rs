use rand::Rng;

use crate::logger::{LogLevel, Logger};

pub struct Agent {
    pub name: String,
    pub port: u16,
    pub success_rate: f64,
    pub logger: Logger,
}

impl Clone for Agent {
    fn clone(&self) -> Self {
        Agent {
            name: self.name.clone(),
            port: self.port,
            success_rate: self.success_rate,
            logger: self.logger.clone(),
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
        }
    }

    pub fn log(&self, msg: String, loglevel: LogLevel) {
        self.logger.log(msg, loglevel);
    }

    pub fn prepare(&self) -> u8 {
        let result = rand::thread_rng().gen_bool(self.success_rate);
        result as u8
    }
    pub fn commit(&self) -> u8 {
        1
    }
    pub fn abort(&self) -> u8 {
        1
    }
}
