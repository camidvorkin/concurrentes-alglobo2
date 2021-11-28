use std::fs::File;
use std::io::Read;
use std::error::Error;
use std::collections::HashMap;

use serde_yaml::{self, Sequence};
use std::convert::TryInto;

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
    .expect("Name must be a string").to_string()
}


pub fn get_prices(filename: &str, agents: Sequence) -> Result<Vec<HashMap<String, u32>>, Box<dyn Error>> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();

    file.read_to_string(&mut contents)?;
    let mut result = Vec::<HashMap<String, u32>>::new();
    for line in contents.lines() {
        let price: Vec<u32> = line.split(',').map(|x| x.parse::<u32>().unwrap()).collect();
        
        let mut map = HashMap::new();
        for (i, agent) in agents.iter().enumerate() {
            map.insert(agent_get_name(&agent), price[i]);
        }
        result.push(map);
    }
    Ok(result)
} 
