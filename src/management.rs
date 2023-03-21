use tokio::io::{AsyncWriteExt, split, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

#[path="./util.rs"]
mod util;

struct ManState {
    broken_links: Vec<u8>
}

pub async fn manager(
    mut receiver: mpsc::Receiver<(String, Vec<u8>)>,
    mut cmd_receiver: mpsc::Receiver<(String, Vec<u8>)>,
    sender: mpsc::Sender<(String, Vec<u8>)>
) {
    let mut state = ManState { broken_links: vec![] };
    loop {
        let mut cmd_rec = None;
        let mut rec = None;
        tokio::select! {
            f = receiver.recv() => rec = f,
            b = cmd_receiver.recv() => cmd_rec = b
        }

        // println!("manager received values: {:?}, {:?}", rec, cmd_rec);

        if cmd_rec.is_some() {
            // handle received command value
            state = handle_cmd_message(cmd_rec, state).await;
        }

        if rec.is_some() {
            state = handle_rec_message(rec, sender.clone(), state).await;
        }
    }
}

async fn handle_cmd_message(cmd_rec: Option<(String, Vec<u8>)>, state: ManState) -> ManState {
    let mut updated_state = ManState { broken_links: state.broken_links };
    match cmd_rec {
        Some(crecval) => {
            match (crecval.0.as_str(), crecval.1) {
                ("handle", msg_enc) => {
                    let dec: Result<&str, _> = bincode::deserialize(&msg_enc);
                    println!("deserialized management message: {:?}", dec);
                    match dec {
                        Ok(c) => {
                            let mut s = c.split(" ");
                            let command = s.next().unwrap();
                            let id = s.next().unwrap().parse::<u8>().unwrap();
                            match (command, id) {
                                ("break_link", id) => {
                                    println!("Breaking link: {}", id);
                                    // break links to specified ID
                                    updated_state.broken_links.push(id);
                                }
                                ("restore_links", ..) => {
                                    println!("Restoring links");
                                    // restore all links to original state
                                    updated_state.broken_links = vec![];
                                }
                                ("get_links", ..) => {
                                    println!("Returning broken links");
                                    write_response_to_client(updated_state.broken_links.clone()).await;

                                }
                                _ => {
                                    eprintln!("Unrecognized input from cmd-client received in manager process")
                                }
                            }
                        }
                        Err(..) => {}
                    }
                }
                _ => {
                    eprintln!("Unrecognized command received");
                }
            }
        }
        None => {} // should literally never happen
    }
    return updated_state;
}

async fn handle_rec_message(rec: Option<(String, Vec<u8>)>,
    sender: mpsc::Sender<(String, Vec<u8>)>,
    state: ManState
) -> ManState {
    let updated_state = ManState { broken_links: state.broken_links };
    match rec {
        Some(recval) => {
            match (recval.0.as_str(), recval.1) {
                ("get_broken_links", ..) => {
                    // return broken links
                    let cloned = updated_state.broken_links.clone();
                    let res = sender.send(("broken_links".into(), cloned)).await;
                    match res {
                        Ok(..) => {}
                        Err(..) => eprintln!("failed to send links to sender"),
                    }
                }
                _ => {
                    eprintln!("Unrecognized input from op-process received in manager process")
                }
            }
        }
        None => {} // should literally never happen
    }
    return updated_state;
}

async fn write_response_to_client(res: Vec<u8>) {
    // Connect to command window server
    let stream = TcpStream::connect(format!("127.0.0.1:{}", util::MAN_CLIENT_PORT)).await.unwrap();
    let (_reader, mut writer): (_, WriteHalf<_>) = split(stream);
    println!("sending msg to man client: {:?}", &res);
    let ser = bincode::serialize(&res).unwrap();
    writer.write_all(&ser).await.unwrap();
}