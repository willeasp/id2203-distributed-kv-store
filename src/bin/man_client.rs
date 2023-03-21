use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

#[path="../util.rs"]
mod util;

#[tokio::main]
async fn main() {
    // Spawns a task to print output from the command window
    tokio::spawn(man_listener());

    println!("Management client started, waiting for commands");

    // TODO move IP into config file?
    let ip = "127.0.0.1";

    loop {
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect(&*format!("Failed to read input: {}", input));

        let id: u64 = input
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .expect(&*format!("Failed to read ID from input string: {}", input));

        // TODO use HTTP requests?
        let addr = format!("{}:{}", ip, util::MAN_PORT_BASE + id);
        println!("Sending message to manager on addr: {}", addr);
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut writer = tokio::io::split(stream).1;

        let message = input[2..].trim();
        let message_bytes = bincode::serialize(&message).unwrap();

        // println!("Serialized message: {:?}, original: {}", message_bytes, line[2..].trim());
        // let dec: String = bincode::deserialize(&message_bytes).unwrap();
        // println!("Deserialized message: {:?}", dec);

        writer.write_all(&message_bytes).await.unwrap();
    }
}

async fn man_listener() {
    let ip = "127.0.0.1";
    let port = util::MAN_CLIENT_PORT;
    let addr = format!("{}:{}", ip, port);

    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("Starting man_client listener on addr: {}", addr);

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let (mut reader, _) = io::split(socket);
        let mut buffer = vec![1; 2048];
        loop {
            let n = reader.read(&mut buffer).await.unwrap();
            if n == 0 { break; }
            let resized = &buffer[..n];
            let msg: Vec<u8> = bincode::deserialize(resized).unwrap();
            println!("Response received: {:?}", msg);
        }
    }
}