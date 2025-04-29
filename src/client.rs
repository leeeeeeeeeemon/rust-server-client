use tokio::net::UdpSocket;
use uuid::Uuid;
use crate::protocol::{Packet, PacketType};
use bincode;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use inquire::{Select, Text};

pub async fn run_udp_client() -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
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

    // Получаем ConnectResponse
    let mut buf = vec![0u8; 2048];
    let (len, _) = socket.recv_from(&mut buf).await?;
    let received = &buf[..len];
    let response_packet: Packet = bincode::deserialize(received).expect("Failed to deserialize packet");

    let my_id: Uuid = match response_packet.packet_type {
        PacketType::ConnectResponse => {
            bincode::deserialize(&response_packet.payload).expect("Failed to deserialize my ID")
        }
        _ => {
            log::error!("[Client] Unexpected packet type: {:?}", response_packet.packet_type);
            return Ok(());
        }
    };

    log::info!("[Client] Connected! My ID: {}", my_id);

    // Фоновая задача для keep-alive
    let socket_clone = Arc::clone(&socket);
    let my_id_bytes = bincode::serialize(&my_id).expect("Failed to serialize UUID");
    let server_addr_str = server_addr.to_string();

    tokio::spawn(async move {
        loop {
            let ping_packet = Packet {
                packet_type: PacketType::Ping,
                payload: my_id_bytes.clone(),
            };
            let encoded = bincode::serialize(&ping_packet).unwrap();
            if let Err(e) = socket_clone.send_to(&encoded, &server_addr_str).await {
                log::error!("[Client] Failed to send Ping: {}", e);
            } else {
                log::debug!("[Client] Sent Ping");
            }
            sleep(Duration::from_secs(5)).await;
        }
    });

    // Меню команд
    loop {
        let actions = vec![
            "Показать список клиентов",
            "Запросить информацию о клиенте",
            "Выйти",
        ];

        let choice = Select::new("Выберите действие", actions).prompt();

        match choice {
            Ok(action) => match action {
                "Показать список клиентов" => {
                    let list_request = Packet {
                        packet_type: PacketType::ClientListRequest,
                        payload: vec![],
                    };

                    let encoded = bincode::serialize(&list_request).unwrap();
                    socket.send_to(&encoded, server_addr).await?;

                    loop {
                        let (len, _) = socket.recv_from(&mut buf).await?;
                        let packet: Packet = bincode::deserialize(&buf[..len]).unwrap();

                        match packet.packet_type {
                            PacketType::ClientListResponse => {
                                let clients: Vec<Uuid> = bincode::deserialize(&packet.payload).unwrap();
                                log::info!("[Client] Активные клиенты:");
                                for id in &clients {
                                    log::info!("- {}", id);
                                }
                                break;
                            }
                            PacketType::PingAck => {
                                log::debug!("[Client] Получен PingAck (пропускаем)");
                                continue;
                            }
                            _ => {
                                log::warn!("[Client] Неожиданный пакет: {:?}", packet.packet_type);
                                continue;
                            }
                        }
                    }
                }


                "Запросить информацию о клиенте" => {
                    let input = Text::new("Введите UUID клиента:").prompt();
                    if let Ok(id_str) = input {
                        if let Ok(client_id) = Uuid::parse_str(&id_str) {
                            let info_request = Packet {
                                packet_type: PacketType::ClientInfoRequest,
                                payload: bincode::serialize(&client_id).unwrap(),
                            };
                            let encoded = bincode::serialize(&info_request).unwrap();
                            socket.send_to(&encoded, server_addr).await?;

                            loop {
                                let (len, _) = socket.recv_from(&mut buf).await?;
                                let packet: Packet = bincode::deserialize(&buf[..len]).unwrap();

                                match packet.packet_type {
                                    PacketType::ClientInfoResponse => {
                                        let (addr, device): (std::net::SocketAddr, String) =
                                            bincode::deserialize(&packet.payload).unwrap();
                                        log::info!("[Client] Client info:");
                                        log::info!("  Address: {}", addr);
                                        log::info!("  Device: {}", device);
                                        break;
                                    }
                                    PacketType::PingAck => {
                                        log::debug!("[Client] Получен PingAck (пропускаем)");
                                        continue;
                                    }
                                    _ => {
                                        log::warn!("[Client] Неожиданный пакет: {:?}", packet.packet_type);
                                        continue;
                                    }
                                }
                            }
                        } else {
                            log::warn!("[Client] Некорректный UUID");
                        }
                    }
                }


                "Выйти" => {
                    log::info!("[Client] Завершение работы.");
                    break;
                }

                _ => {}
            },
            Err(_) => {
                log::warn!("Ошибка ввода. Завершаем.");
                break;
            }
        }
    }

    Ok(())
}
