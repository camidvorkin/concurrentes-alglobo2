use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use serde_yaml::{self, Sequence};
use std::convert::TryInto;

const AGENTS_FILE: &str = "src/agents.yaml";

pub fn get_agents() -> Sequence {
    let agents_config = std::fs::File::open(AGENTS_FILE).expect("Couldn't open agents config file");
    serde_yaml::from_reader(agents_config).expect("Couldn't parse agents config yaml")
}

pub fn get_agents_ports() -> Vec<u16> {
    let agents = get_agents();
    let mut ports = Vec::new();
    for agent in agents {
        let port = agent_get_port(&agent);
        ports.push(port);
    }
    ports
}

pub fn agent_get_port(agent: &serde_yaml::Value) -> u16 {
    agent["port"]
        .as_u64()
        .expect("Agent port must be an unsigned integer")
        .try_into()
        .expect("Agent port must be a valid port number")
}

pub fn agent_get_name(agent: &serde_yaml::Value) -> String {
    agent["name"]
        .as_str()
        .expect("Name must be a string")
        .to_string()
}

pub fn agent_get_success_rate(agent: &serde_yaml::Value) -> f64 {
    agent["successrate"]
        .as_f64()
        .expect("Agent successrate must be a float")
}

pub fn csv_to_prices(filename: &str) -> Vec<Vec<u32>> {
    let mut file = File::open(filename).expect("File not found");
    let mut contents = String::new();

    file.read_to_string(&mut contents)
        .expect("Couldn't read file");
    let mut result = Vec::new();
    for line in contents.lines() {
        let price: Vec<u32> = line
            .split(',')
            .map(|x| x.parse::<u32>().expect("Couldn't parse to u32"))
            .collect();
        result.push(price);
    }
    result
}

pub fn create_empty_csv(filename: &str) -> File {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filename)
        .expect("Failed to create empty csv file")
}

pub fn write_to_csv(mut file: &File, numbers: &[u32]) {
    let line = numbers
        .iter()
        .map(|&n| n.to_string())
        .collect::<Vec<String>>()
        .join(",");
    file.write_all(line.as_bytes())
        .expect("Failed to write to csv file");
    file.write_all("\n".as_bytes())
        .expect("Failed to write to csv file");
}
