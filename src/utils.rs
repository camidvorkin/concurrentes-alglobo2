use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use serde_yaml::{self, Sequence};
use std::convert::TryInto;

const AGENTS_FILE: &str = "src/agents.yaml";

pub fn get_agents() -> Sequence {
    let agents_config = std::fs::File::open(AGENTS_FILE).expect("Couldn't open agents config file");
    serde_yaml::from_reader(agents_config).expect("Couldn't parse agents config yaml")
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

pub fn csv_to_prices(filename: &str, agents: &Sequence) -> Vec<HashMap<String, u32>> {
    let mut file = File::open(filename).expect("File not found");
    let mut contents = String::new();

    file.read_to_string(&mut contents)
        .expect("Couldn't read file");
    let mut result = Vec::<HashMap<String, u32>>::new();
    for line in contents.lines() {
        let price: Vec<u32> = line.split(',').map(|x| x.parse::<u32>().unwrap()).collect();

        let mut map = HashMap::new();
        for (i, agent) in agents.iter().enumerate() {
            map.insert(agent_get_name(&agent), price[i]);
        }
        result.push(map);
    }
    result
}
