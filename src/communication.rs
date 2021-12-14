use std::convert::TryInto;

/// Transaction Message for the first phase: preparing
pub const PREPARE: u8 = b'P';
/// Transaction Message for the second phase: commiting
pub const COMMIT: u8 = b'C';
/// Transaction Message for the second phase: aborting
pub const ABORT: u8 = b'A';
/// Message to stop listening for incoming connections
pub const FINISH: u8 = b'F';

/// Message to acknowledge an operation being done
pub const ACK: u8 = 1;
/// Message when a payment wasn't accepted
pub const PAYMENT_ERR: u8 = 0;
/// Message when a payment was accepted
pub const PAYMENT_OK: u8 = 1;

/// The number of bytes needed in a DataMsg array
pub type DataMsgBytes = [u8; 9];

/// 9 byte message to communicate from alglobo to the agents
pub struct DataMsg {
    /// 4 bytes id of the transaction to operate on
    pub transaction_id: u32,
    /// 4 bytes for the transaction payment
    pub data: u32,
    /// 1 byte for the transaction operation
    pub opcode: u8,
}

impl DataMsg {
    /// Translate a 9 byte array into a DataMsg structure
    pub fn from_bytes(msg: DataMsgBytes) -> DataMsg {
        let transaction_id: u32 =
            u32::from_be_bytes(msg[0..4].try_into().expect("Couldn't convert to u32"));
        let data: u32 = u32::from_be_bytes(msg[4..8].try_into().expect("Couldn't convert to u32"));
        let opcode: u8 = msg[8];

        DataMsg {
            transaction_id,
            data,
            opcode,
        }
    }

    /// Translate a DataMsg structure into a 9 byte array
    pub fn to_bytes(data_msg: &DataMsg) -> DataMsgBytes {
        let mut bytes = Vec::new();
        bytes.extend(data_msg.transaction_id.to_be_bytes());
        bytes.extend(data_msg.data.to_be_bytes());
        bytes.push(data_msg.opcode as u8);
        bytes.try_into().expect("Couldn't form bytearray")
    }
}
