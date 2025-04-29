use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum PacketType {
    ConnectRequest,
    ConnectResponse,
    ClientListRequest,
    ClientListResponse,
    ClientInfoRequest,
    ClientInfoResponse,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub packet_type: PacketType,
    pub payload: Vec<u8>,
}
