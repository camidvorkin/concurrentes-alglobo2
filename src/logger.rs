//! Logging functions
use std::fs::OpenOptions;
use std::io::Write;

/// Logging file to be created (or entirely rewritten) on each run

/// Simple logging levels for our logger
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    /// TRACE logs refer to system initialization and nothing domain specific
    TRACE,
    /// INFO logs are printed to the console and are useful for any domain specific information
    INFO,
    /// FINISH logs are used to signalize that we want to stop our processing and the program is shutting down
    FINISH,
}

/// A log consists of a message and their log level
pub type LoggerMsg = (String, LogLevel);

pub struct Logger {
    filename: String,
}

impl Clone for Logger {
    fn clone(&self) -> Self {
        Logger {
            filename: self.filename.clone(),
        }
    }
}

impl Logger {
    /// Creates a new logger
    pub fn new(filename: String) -> Self {
        let mut filename_path = "logs/".to_string();
        filename_path.push_str(filename.as_str());
        filename_path.push_str(".log");

        let logger = Logger {
            filename: filename_path.clone(),
        };
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(filename_path.as_str())
            .expect("Failed to create log file");
        logger.log("START".to_string(), LogLevel::TRACE);
        logger
    }

    /// Logs the message to the file and, if the level is INFO, prints to console
    pub fn log(&self, msg: String, loglevel: LogLevel) {
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.filename.clone().as_str())
            .expect("Unable to open log file");
        if let LogLevel::INFO = loglevel {
            println!("{}", msg)
        };
        // We want to have the loglevel on exactly N characters, so that `| TRACE  |` and `|  INFO  |` and `| FINISH |` have the same width.
        // This formatting only works with strings, not debug strings
        // i.e. {:^7} works, but {:^7?} does not
        // So we first do some format! shenanigans to convert the debug string to a string
        let loglevelstr = format!("{:?}", loglevel);

        let msg = format!("{} | {:<6} | {} \n", chrono::Local::now(), loglevelstr, msg);
        file.write_all(msg.as_bytes())
            .expect("Unable to write data");
    }
}
