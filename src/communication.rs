use std::convert::TryInto;

pub const PREPARE: u8 = b'P';
pub const COMMIT: u8 = b'C';
pub const ABORT: u8 = b'A';

pub type DataMsgBytes = [u8; 9];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataMsg {
    pub transaction_id: u32,
    pub data: u32,
    pub opcode: u8,
}

pub fn data_msg_to_bytes(data_msg: &DataMsg) -> DataMsgBytes {
    let mut bytes = Vec::new();
    bytes.extend(data_msg.transaction_id.to_be_bytes());
    bytes.extend(data_msg.data.to_be_bytes());
    bytes.push(data_msg.opcode as u8);
    bytes.try_into().expect("Couldn't form bytearray")
}

pub fn data_msg_from_bytes(msg: DataMsgBytes) -> DataMsg {
    let transaction_id: u32 = u32::from_be_bytes(msg[0..4].try_into().unwrap());
    let data: u32 = u32::from_be_bytes(msg[4..8].try_into().unwrap());
    let opcode: u8 = msg[8];

    DataMsg {
        transaction_id,
        data,
        opcode,
    }
}
