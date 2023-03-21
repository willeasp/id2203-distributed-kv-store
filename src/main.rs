use std::{path::Path, thread, time};
use std::sync::Arc;

use commitlog::LogOptions;
use omnipaxos_core::{
    messages::{
        Message::{BLE, SequencePaxos},
        sequence_paxos::{PaxosMessage, PaxosMsg::*}
    },
    omni_paxos::*,
    util::LogEntry::Decided
};
use omnipaxos_core::messages::Message;
use omnipaxos_core::util::LogEntry;
use omnipaxos_storage::{
    memory_storage::MemoryStorage,
    persistent_storage::{PersistentStorage, PersistentStorageConfig},
};
use serde::{Deserialize, Serialize};
use sled::Config;
use structopt::StructOpt;
use tokio::io::{split, WriteHalf};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use store::KVStore;

mod management;
mod util;
mod http;
mod store;

#[derive(Debug, StructOpt, Serialize, Deserialize)]
struct Node {
    #[structopt(long)]
    id: u64,
    #[structopt(long)]
    peers: Vec<u64>,
    #[structopt(parse(try_from_str), default_value = "false")]
    recover: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}


#[tokio::main]
async fn main() {
    let node = Node::from_args();
    let node_id = node.id;
    let recover_path = format!("./recv/node{}", node_id);
    let log_opts = LogOptions::new(&recover_path);
    let mut sled_opts = Config::default();
    sled_opts = sled_opts.path(&recover_path);
    let persistent_config = PersistentStorageConfig::with(recover_path.clone(), log_opts, sled_opts);

    let recover = Path::new(&recover_path).exists();

    let op_config = OmniPaxosConfig {
        pid: node.id,
        configuration_id: node_id.try_into().unwrap(),
        peers: node.peers.to_vec(),
        ..Default::default()
    };

    // let storage = MemoryStorage::<KeyValue, ()>::default();
    // let op = op_config.build(storage);
    let mut op: OmniPaxos<KeyValue, () , PersistentStorage<KeyValue, ()>>;
    if !recover
    {
        let persistent_storage = PersistentStorage::<KeyValue, ()>::new(persistent_config);
        op = op_config.build(persistent_storage);
        println!("New instance of Omni-paxos created with recovery path: {}", recover_path);
    }
    else
    {
        let recovered_storage: PersistentStorage<KeyValue, ()> = PersistentStorage::open(persistent_config);
        op = op_config.build(recovered_storage);
        op.fail_recovery();
        println!("Recovered old instance of Omni-paxos with recovery path: {}", recover_path);
    }

    let (sender1, receiver): (mpsc::Sender<(String, Vec<u8>)>, _) = mpsc::channel(32);
    let (man_sender, man_receiver): (mpsc::Sender<(String, Vec<u8>)>, _) = mpsc::channel(32);
    let (cmd_man_sender, cmd_man_receiver): (mpsc::Sender<(String, Vec<u8>)>, _) = mpsc::channel(32);
    let (sender_man_sender, sender_man_receiver): (mpsc::Sender<(String, Vec<u8>)>, _) = mpsc::channel(32);

    let kv_store = Arc::new(Mutex::new(KVStore::new()));

    tokio::spawn(async move {
        management::manager(man_receiver, cmd_man_receiver, sender_man_sender).await;
    });

    let new_kv_store = Arc::clone(&kv_store);
    tokio::spawn(async move {
        op_command_handler(&node.id, op, receiver, new_kv_store, sender_man_receiver, man_sender).await;
    });

    let new_sender = sender1.clone();
    tokio::spawn(async move {
        send_messages(new_sender).await;
    });

    let new_kv_store = Arc::clone(&kv_store);
    let new_sender = sender1.clone();
    tokio::spawn(async move {
        http::api_server(new_kv_store, new_sender, &node.id).await;
    });

    let new_sender = sender1.clone();
    tokio::spawn(async move {
        cmd_listener(new_sender, node.id).await;
    });

    let new_sender = sender1.clone();
    tokio::spawn(async move {
        election_timeout(new_sender).await;
    });




    let mut listen_addr: String = "127.0.0.1:".to_owned();
    let listen_port: u64 = util::SERV_PORT_BASE + node.id;
    listen_addr.push_str(&listen_port.to_string().to_owned());

    println!("Starting Server listener on addr: {}", listen_addr);

    let listener = TcpListener::bind(listen_addr).await.unwrap();

    let mut man_listen_addr: String = "127.0.0.1:".to_owned();
    let man_listen_port: u64 = util::MAN_PORT_BASE + node.id;
    man_listen_addr.push_str(&man_listen_port.to_string().to_owned());

    println!("Starting Manager listener on addr: {}", man_listen_addr);

    let man_listener = TcpListener::bind(man_listen_addr).await.unwrap();

    println!("Server successfully started - server ID: {}, Peer ID's: {:?}", node.id, node.peers);

    loop {
        let sender_n = sender1.clone();

        let mut socket: Option<TcpStream> = None;
        let mut man_socket: Option<TcpStream> = None;

        tokio::select! {
            r = listener.accept() => {
                match r {
                    Ok((_socket, _)) => {
                        socket = Some(_socket);
                    },
                    Err(..) => {}
                }
            },
            m = man_listener.accept() => {
                match m {
                    Ok((_socket, _)) => {
                        man_socket = Some(_socket);
                    },
                    Err(..) => {}
                }
            }
        }

        if socket.is_some() {
            tokio::spawn(async move {
                handle_commands(socket.unwrap(),sender_n).await;
            });
        }

        if man_socket.is_some() {
            println!("Received new management connection");
            let man_sender_c = cmd_man_sender.clone();
            tokio::spawn(async move {
                handle_man_commands(man_socket.unwrap(), man_sender_c).await;
            });
        }

    }
}


async fn handle_commands(mut read_socket: TcpStream, sender: mpsc::Sender<(String, Vec<u8>)>) {
    let mut buffer = Vec::with_capacity(8192); // use a dynamically allocated buffer
    loop {
        let n = match read_socket.read_buf(&mut buffer).await {
            Ok(n) if n == 0 => break, // connection closed by remote
            Ok(n) => n,
            Err(e) => {
                eprintln!("failed to read from socket: {}", e);
                break;
            }
        };
        // Resize the buffer to exactly fit the number of bytes read
        buffer.resize(n, 0);
        // Send the received bytes over the mpsc Sender to another thread with the tag "handle"
        if let Err(e) = sender.send(("handle".into(), buffer.clone())).await {
            eprintln!("failed to send message over channel: {}", e);
            break;
        }
    }
}

async fn handle_man_commands(mut read_socket: TcpStream, man_sender: mpsc::Sender<(String, Vec<u8>)>) {
    // println!("handle_man_read called");
    let mut buffer = Vec::with_capacity(8192); // use a dynamically allocated buffer
    loop {
        let n = match read_socket.read_buf(&mut buffer).await {
            Ok(n) if n == 0 => break, // connection closed by remote
            Ok(n) => n,
            Err(e) => {
                eprintln!("failed to read from socket: {}", e);
                break;
            }
        };
        // println!("Manager received command of size: {}", n);
        // Resize the buffer to exactly fit the number of bytes read
        buffer.resize(n, 0);
        // println!("sending command to manager: {:?}", buffer);
        // Send the received bytes over the mpsc Sender to another thread with the tag "handle"
        if let Err(e) = man_sender.send(("handle".into(), buffer.clone())).await {
            eprintln!("Failed to send message to manager thread over channel: {}", e);
            break;
        }
    }
}

async fn send_messages(sender: mpsc::Sender<(String, Vec<u8>)>) {
    // periodically check outgoing messages and send all in list
    loop {
        thread::sleep(time::Duration::from_millis(1));
        sender.send(("send_outgoing".into(), vec![])).await.unwrap();
    }
}

async fn election_timeout(sender: mpsc::Sender<(String, Vec<u8>)>){
    loop{
        thread::sleep(time::Duration::from_millis(100));
        sender.send(("election_timeout".into(), vec![])).await.unwrap();
    }
}

async fn op_command_handler(
    id: &u64,
    mut op: OmniPaxos<KeyValue, (), PersistentStorage<KeyValue, ()>>,
    mut receiver: mpsc::Receiver<(String, Vec<u8>)>,
    kv_store: Arc<Mutex<KVStore>>,
    mut man_receiver: mpsc::Receiver<(String, Vec<u8>)>,
    man_sender: mpsc::Sender<(String, Vec<u8>)>
) {
    let mut idx: u64 = 0;
    while let Some(action) = receiver.recv().await {
        match (action.0.as_str(), action.1) {
            ("handle", encrypted) => {
                let msg: Message<KeyValue, ()> = bincode::deserialize(&encrypted).unwrap();
                // println!("handling message, querying manager");
                man_sender.send(("get_broken_links".into(), Vec::new())).await.unwrap();
                let res = man_receiver.recv().await.unwrap();
                // println!("received from manager: {:?}", res);
                let sender = msg.get_sender();
                if (res.1.clone()).contains(&(sender as u8)) {
                    println!("link to receiver {} is broken, ignoring handling message", sender);
                    continue;
                }
                // println!("Handling incoming message: {:?}", msg);
                op.handle_incoming(msg);
            }
            ("send_outgoing", ..) => {
                for message in op.outgoing_messages() {
                    let out_receiver = message.get_receiver();
                    // NOTE: This is only for debug purposes - sometimes, we want to "break" connections
                    // manually, so we filter messages based on their receiver ID
                    // println!("sending outgoing messages, querying manager thread");
                    man_sender.send(("get_broken_links".into(), Vec::new())).await.unwrap();
                    let res = man_receiver.recv().await.unwrap();
                    // // println!("received from manager: {:?}", res);
                    if (res.1.clone()).contains(&(out_receiver as u8)) {
                        println!("link to receiver {} is broken, ignoring sending message", out_receiver);
                        continue;
                    }
                    let port = util::SERV_PORT_BASE + out_receiver;
                    let rec_addr = format!("127.0.0.1:{}", port);
                    // println!("Connecting to receiver: {}", rec_addr);
                    match TcpStream::connect(rec_addr).await {
                        Ok(stream) => {
                            let (_reader, mut writer) = io::split(stream);
                            let msg_enc: Vec<u8> = bincode::serialize(&message).unwrap();
                            writer.write_all(&msg_enc).await.unwrap();
                        }
                        Err(err) => {
                            eprintln!("Error connecting to TCP stream: {}", err);
                        }
                    }
                }
            }
            ("read", encrypted) => {
                // get key for reading
                let key: String = bincode::deserialize(&encrypted).unwrap();
                println!("Read received: {}", key);

                // get all decided values from index
                let decided: Option<Vec<LogEntry<KeyValue, ()>>> = op.read_decided_suffix(0);
                match decided {
                    Some(vec) => {
                        write_response_to_client(vec.to_vec(), key).await;
                    }
                    _ => eprintln!("Unexpected error received when reading decided suffix"),
                }
            }
            ("write", encrypted) => {
                let kv: KeyValue = bincode::deserialize(&encrypted).unwrap();
                let c = kv.clone();
                op.append(kv).expect("Failed to append");
                let k = c.key;
                let v = c.value;
                println!("key/value written to Omni-paxos: {} = {}", k, v);
            }
            ("election_timeout", ..) => {
                op.election_timeout()
            }
            other => {
                println!("Unexpected command received: {:?}", other);
            }
        }

        // update kv_store
        let new_idx = op.get_decided_idx();
        if new_idx > idx {
            println!("new idx: {}", new_idx);
            // TODO: might be a more performant implementation
            let decided = op.read_decided_suffix(idx);
            match decided {
                Some(suffix) => insert_suffix(id, suffix, &kv_store).await,
                None => {}
            }
            idx = new_idx;
        }
    }
}

/// Insert decided suffix into the kv_store
async fn insert_suffix(id: &u64, suffix: Vec<LogEntry<KeyValue, ()>>, kv_store: &Arc<Mutex<KVStore>>) {
    println!("insert_suffix");
    for entry in suffix {
        match entry {
            Decided(KeyValue { key, value } ) => {
                println!("Inserted {} -> {}", key, value);
                kv_store.lock().await.insert(key, value);
            },
            _ => {},
        }
    }
}

async fn cmd_listener(sender: mpsc::Sender<(String, Vec<u8>)>, id: u64) {
    let listen_addr = format!("127.0.0.1:{}", util::CMD_PORT_BASE + id);
    println!("listening on addr: {}", listen_addr);
    let listener = TcpListener::bind(listen_addr).await.unwrap();
    loop {
        if let Ok((socket, _)) = listener.accept().await {
            let (mut reader, _) = io::split(socket);
            let mut buffer = vec![0u8; 128];
            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        handle_command(&sender, &buffer[..n]).await;
                    },
                    Err(e) => {
                        eprintln!("Error reading from socket: {}", e);
                        break;
                    }
                }
            }
        } else {
            eprintln!("Failed to accept incoming connection");
        }
    }
}

async fn handle_command(sender: &mpsc::Sender<(String, Vec<u8>)>, buffer: &[u8]) {
    let message: String = bincode::deserialize(buffer).unwrap();
    let msg_vec: Vec<&str> = message.split_whitespace().collect();

    match msg_vec.get(0) {
        Some(&"read") => {
            // println!("handling read command");
            if let Some(key) = msg_vec.get(1) {
                sender.send(("read".into(), bincode::serialize(&key.to_string()).unwrap())).await.unwrap();
            }
        }
        Some(&"write") => {
            // println!("handling write command");
            let key = msg_vec.get(1).cloned().unwrap_or_default();
            let value = msg_vec.get(2).map(|s| s.trim()).unwrap_or_default().to_string();
            let kv = KeyValue { key: key.to_string(), value };
            sender.send(("write".into(), bincode::serialize(&kv).unwrap())).await.unwrap();
        }
        Some(&"delete") => {
            let key = msg_vec.get(1).cloned().unwrap_or_default();
            let value= "";
            let kv = KeyValue { key: key.to_string(), value: value.to_string() };
            sender.send(("write".into(), bincode::serialize(&kv).unwrap())).await.unwrap();
        }
        Some(cmd) => println!("Unknown command received: {:?}", cmd),
        None => {}
    }
}


async fn write_response_to_client(log: Vec<LogEntry<KeyValue, ()>>, key: String) {
    let stream = TcpStream::connect(format!("127.0.0.1:{}", util::CMD_PORT_BASE)).await.unwrap();
    let (_, mut writer): (_, WriteHalf<_>) = split(stream);

    // Search for the latest entry with the given key
    for entry in log.iter().rev() {
        match entry {
            Decided(kv) if kv.key == key => {
                let response = format!("{}", kv.value);
                let message: Vec<u8> = bincode::serialize(&response).unwrap();
                writer.write_all(&message).await.unwrap();
                return;
            }
            _ => continue,
        }
    }

    // Key not found, send empty response to command window server
    let empty_response: String = String::new();
    let message: Vec<u8> = bincode::serialize(&empty_response).unwrap();
    writer.write_all(&message).await.unwrap();
}