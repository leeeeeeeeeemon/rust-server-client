mod protocol;
mod client;
mod server;

fn init_logger() {
    log4rs::init_file("log4rs.yaml", Default::default()).expect("Failed to initialize logger");
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_logger();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "client" {
        client::run_udp_client().await
    } else {
        server::run_udp_server().await
    }
}
