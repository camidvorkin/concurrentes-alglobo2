use crate::logger::{Logger, LogLevel};

pub struct Agent {
    name: String,
    port: u16,
    success_rate: f64,
    logger: Logger,
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

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_success_rate(&self) -> f64 {
        self.success_rate
    }

    pub fn log(&self, msg: String, loglevel: LogLevel) {
        self.logger.log(msg, loglevel);
    }
}