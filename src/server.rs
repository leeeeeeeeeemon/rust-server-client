use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::net::UdpSocket;
use uuid::Uuid;
use crate::protocol::{Packet, PacketType};
use bincode;
use tokio::time::{sleep, Duration};

type ClientId = Uuid;

#[derive(Debug)]
pub struct ClientInfo {
    pub address: SocketAddr,
    pub device_type: String,
    pub last_seen: Instant
    
}

pub async fn run_udp_server() -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:8080").await?;
    log::info!("[Server] Listening on {}", socket.local_addr()?);

    let clients: Arc<Mutex<HashMap<ClientId, ClientInfo>>> = Arc::new(Mutex::new(HashMap::new()));

    let clients_cleanup = Arc::clone(&clients); // клонируем для фоновой задачи

    //Очистка клиентов которые больше не пингуют
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;

            let mut clients_map = clients_cleanup.lock().unwrap();
            let now = Instant::now();

            let before = clients_map.len();

            clients_map.retain(|client_id, info| {
                let alive = now.duration_since(info.last_seen) <= Duration::from_secs(15);
                if !alive {
                    log::warn!("[Server] Removing inactive client: {}", client_id);
                }
                alive
            });

            let after = clients_map.len();

            if before != after {
                log::info!("[Server] Active clients after cleanup: {}", after);
            }
        }
    });

    let mut buf = vec![0u8; 2048];

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let received = &buf[..len];

        match bincode::deserialize::<Packet>(received) {
            Ok(packet) => {
                log::info!("[Server] Received packet from {}: {:?}", addr, packet.packet_type);

                match packet.packet_type {
                    PacketType::ConnectRequest => {
                        let client_id = Uuid::new_v4();
                        let device_type = String::from_utf8(packet.payload.clone()).unwrap_or_else(|_| "Unknown".into());

                        let client_info = ClientInfo {
                            address: addr,
                            device_type,
                            last_seen: Instant::now()
                        };

                        let mut clients_map = clients.lock().unwrap();
                        clients_map.insert(client_id, client_info);

                        log::info!("[Server] New client added: {}", client_id);

                        let response_packet = Packet {
                            packet_type: PacketType::ConnectResponse,
                            payload: bincode::serialize(&client_id).expect("Failed to serialize UUID"),
                        };

                        let encoded = bincode::serialize(&response_packet).expect("Failed to serialize response packet");
                        socket.send_to(&encoded, addr).await?;
                    }

                    PacketType::ClientListRequest => {
                        log::info!("[Server] Client requested list of active sessions");

                        let clients_map = clients.lock().unwrap();
                        log::info!("[Server] Current clients in HashMap:");
                        for (id, info) in clients_map.iter() {
                            log::info!("- {} ({})", id, info.device_type);
                        }

                        let client_ids: Vec<Uuid> = clients_map.keys().cloned().collect();

                        let payload = bincode::serialize(&client_ids).expect("Failed to serialize client list");

                        let response_packet = Packet {
                            packet_type: PacketType::ClientListResponse,
                            payload,
                        };

                        let encoded = bincode::serialize(&response_packet).expect("Failed to serialize response packet");
                        socket.send_to(&encoded, addr).await?;
                    }

                    PacketType::ClientInfoRequest => {
                        log::info!("[Server] Client requested info for specific client");

                        let requested_id: Uuid = match bincode::deserialize(&packet.payload) {
                            Ok(id) => id,
                            Err(_) => {
                                log::error!("[Server] Failed to deserialize requested client ID");
                                continue;
                            }
                        };

                        let clients_map = clients.lock().unwrap();

                        if let Some(info) = clients_map.get(&requested_id) {
                            log::info!("[Server] Found client {}: {}", requested_id, info.device_type);

                            let response_data = (info.address, info.device_type.clone());
                            let payload = bincode::serialize(&response_data).expect("Failed to serialize client info");

                            let response_packet = Packet {
                                packet_type: PacketType::ClientInfoResponse,
                                payload,
                            };

                            let encoded = bincode::serialize(&response_packet).expect("Failed to serialize response packet");
                            socket.send_to(&encoded, addr).await?;
                        } else {
                            log::error!("[Server] Client ID not found: {}", requested_id);
                        }
                    }

                    PacketType::Ping => {
                        let client_id: Uuid = match bincode::deserialize(&packet.payload) {
                            Ok(id) => id,
                            Err(_) => {
                                log::warn!("[Server] Invalid Ping payload");
                                continue;
                            }
                        };
                    
                        let mut clients_map = clients.lock().unwrap();
                    
                        if let Some(client_info) = clients_map.get_mut(&client_id) {
                            client_info.last_seen = Instant::now(); 
                            log::debug!("[Server] Received Ping from {}", client_id);
                        } else {
                            log::warn!("[Server] Ping from unknown client: {}", client_id);
                        }
                    
                        // Подтверждаем keep-alive
                        let response_packet = Packet {
                            packet_type: PacketType::PingAck,
                            payload: vec![],
                        };
                    
                        let encoded = bincode::serialize(&response_packet).expect("Failed to serialize PingAck");
                        socket.send_to(&encoded, addr).await?;
                    }

                    _ => {
                        log::error!("[Server] Unknown packet type: {:?}", packet.packet_type);
                    }
                }
            }
            Err(e) => {
                log::error!("[Server] Failed to deserialize packet: {}", e);
            }
        }
    }
}
