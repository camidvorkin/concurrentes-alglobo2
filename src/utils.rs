use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use serde_yaml::{self, Sequence};
use std::convert::TryInto;

pub const TransactionPrepare: u8 = b'P';
pub const TransactionCommit: u8 = b'C';
pub const TransactionAbort: u8 = b'A';

pub struct DataMsg {
    pub transaction_id: u32,
    pub data: u32,
    pub opcode: u8,
}

pub fn data_msg_to_bytes(data_msg: &DataMsg) -> [u8; 9] {
    let mut bytes = Vec::new();
    bytes.extend(data_msg.transaction_id.to_be_bytes());
    bytes.extend(data_msg.data.to_be_bytes());
    bytes.push(data_msg.opcode as u8);
    let arr: [u8; 9] = bytes.try_into().expect("Couldn't form bytearray");
    arr
}

pub fn data_msg_from_bytes(msg: [u8; 9]) -> DataMsg {
    let transaction_id: u32 = u32::from_be_bytes(msg[0..4].try_into().unwrap());
    let data: u32 = u32::from_be_bytes(msg[4..8].try_into().unwrap());
    let opcode: u8 = msg[8];

    DataMsg {
        transaction_id,
        data,
        opcode,
    }
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

pub fn get_prices(
    filename: &str,
    agents: Sequence,
) -> Result<Vec<HashMap<String, u32>>, Box<dyn Error>> {
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
