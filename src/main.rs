mod board;
mod server;

use std::env;

fn main() -> std::io::Result<()> {
    let listen_address =
        env::var("SIX_QUI_PREND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    server::run(&listen_address)
}
