use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path="../util.rs"]
mod util;

#[tokio::main]
async fn main() {
    // Spawns a task to print output from the command window
    tokio::spawn(cmd_listener());

    println!("CMD client started, waiting for commands");

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect(&*format!("Failed to read input: {}", input));

        let id: u64 = input.split_whitespace().next().and_then(|s| s.parse().ok()).expect(&*format!("Failed to read ID from input string: {}", input));

        let addr = format!("{}:{}", "127.0.0.1", util::CMD_PORT_BASE + id);
        println!("Sending message to server on addr: {}", addr);
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut writer = tokio::io::split(stream).1;
        let message = input[2..].trim();
        let message_bytes = bincode::serialize(&message).unwrap();
        writer.write_all(&message_bytes).await.unwrap();
    }
}

async fn cmd_listener() {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", util::CMD_PORT_BASE)).await.unwrap();

    while let Ok((mut socket, _)) = listener.accept().await {
        let mut buffer = vec![0u8; 128];
        while let Ok(n) = socket.read(&mut buffer).await {
            if n == 0 {
                break;
            }
            let message: String = bincode::deserialize(&buffer[..n]).unwrap();
            println!("Received message: {:?}", message);
            buffer.clear();
        }
    }
}