use tokio::net::UdpSocket;
use crate::protocol::{Packet, PacketType};
use bincode;
use uuid::Uuid;

pub async fn run_udp_client() -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    log::info!("[Client] Socket created: {}", socket.local_addr()?);

    let server_addr = "127.0.0.1:8080";

    // Отправляем ConnectRequest
    let packet = Packet {
        packet_type: PacketType::ConnectRequest,
        payload: b"Desktop".to_vec(),
    };

    let encoded = bincode::serialize(&packet).expect("Failed to serialize packet");
    socket.send_to(&encoded, server_addr).await?;
    log::info!("[Client] Sent ConnectRequest to server at {}", server_addr);

    // Ждём ConnectResponse
    let mut buf = vec![0u8; 2048];
    let (len, _) = socket.recv_from(&mut buf).await?;
    let received = &buf[..len];
    let mut my_id: Option<Uuid> = None; 
    
    let response_packet: Packet = bincode::deserialize(received).expect("Failed to deserialize packet");

    // Проверяем, что это ConnectResponse
    if let PacketType::ConnectResponse = response_packet.packet_type {
        // десериализуем UUID 
        let id: Uuid = bincode::deserialize(&response_packet.payload)
                                    .expect("Failed to deserialize my ID");
        log::debug!("[Client] Successfully connected to server! My ID: {:?}", my_id);
        my_id = Some(id);

        
    } else {
        log::error!("[Client] Unexpected packet type: {:?}", response_packet.packet_type);
        return Ok(());
    }

    // Отправляем ClientListRequest
    let list_request_packet = Packet {
        packet_type: PacketType::ClientListRequest,
        payload: vec![],
    };

    let encoded_list_request = bincode::serialize(&list_request_packet).expect("Failed to serialize list request");
    socket.send_to(&encoded_list_request, server_addr).await?;
    log::info!("[Client] Sent ClientListRequest to server");

    //  Ждём ClientListResponse
    let (len, _) = socket.recv_from(&mut buf).await?;
    let received = &buf[..len];

    let packet: Packet = bincode::deserialize(received).expect("Failed to deserialize packet");

    if let PacketType::ClientListResponse = packet.packet_type {
        let client_list: Vec<Uuid> = bincode::deserialize(&packet.payload).expect("Failed to deserialize client list");

        log::info!("[Client] Active clients:");
        for id in &client_list {
            log::info!("- {}", id);
        }

        if let Some(my_id_value) = my_id {
            if let Some(first_client_id) = client_list.iter().find(|&&id| id != my_id_value) {
                log::info!("[Client] Requesting info for client: {}", first_client_id);
        
                // Отправляем ClientInfoRequest на информацию о первом найденном клиенте, но не о себе
                let info_request_packet = Packet {
                    packet_type: PacketType::ClientInfoRequest,
                    payload: bincode::serialize(first_client_id).expect("Failed to serialize client ID"),
                };
        
                let encoded_info_request = bincode::serialize(&info_request_packet).expect("Failed to serialize info request packet");
                socket.send_to(&encoded_info_request, server_addr).await?;
                log::info!("[Client] Sent ClientInfoRequest to server");
        
                // Ждём ClientInfoResponse
                let mut buf = vec![0u8; 2048];
                let (len, _) = socket.recv_from(&mut buf).await?;
                let received = &buf[..len];
        
                let packet: Packet = bincode::deserialize(received).expect("Failed to deserialize packet");
        
                if let PacketType::ClientInfoResponse = packet.packet_type {
                    let (address, device_type): (std::net::SocketAddr, String) =
                        bincode::deserialize(&packet.payload).expect("Failed to deserialize client info");
        
                        log::info!("[Client] Client info:");
                        log::info!("  Address: {}", address);
                        log::info!("  Device: {}", device_type);
                }
            } else {
                log::error!("[Client] No other clients found.");
            }
        } else {
            log::error!("[Client] Cannot request client info — no connection ID available.");
        }
        
    }

    Ok(())
}
