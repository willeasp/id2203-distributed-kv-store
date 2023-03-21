use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Router;
use axum::routing::get;
use tokio::sync::{mpsc, Mutex};

use crate::KeyValue;
use crate::store::KVStore;

struct HandlerData {
    kv_store: Arc<Mutex<KVStore>>,
    sender: mpsc::Sender<(String, Vec<u8>)>,
}

type ServerState = Arc<Mutex<HandlerData>>;

async fn hello_world(State(state): State<ServerState>) -> String {
    let mut s = String::new();
    s.push_str("[ \n");
    for (key, value) in &state.lock().await.kv_store.lock().await.clone() {
        s.push_str(&*format!("\t{} -> {}, \n", key, value));
    }
    s.push_str("]");
    s
}

#[axum_macros::debug_handler]
async fn put_kv(
    State(state): State<ServerState>,
    Path(params): Path<HashMap<String, String>>,
) -> String {
    let key= params.get("key").unwrap().to_owned();
    let value = match
        params.get("value") {
        Some(value) => value.clone(),
        None => {
            eprintln!("No value found");
            return "No value found".into();
        }
    };

    // create key value
    let kv = KeyValue { key: key.to_string(), value: value.clone() };

    // send to omnipaxos
    state.lock().await.sender.send((
            "write".into(),
            bincode::serialize(&kv).unwrap())
        ).await.unwrap();

    // return ok
    format!("Inserted ({}, {})", kv.key, kv.value)
}

async fn get_kv(State(state): State<ServerState>, Path(key): Path<String>) -> String {
    match &state.lock().await.kv_store.lock().await.get(key.as_str()) {
        Some(val) => format!("{} -> {}", key, val),
        None => format!("No value for key {} found", key)
    }
}

pub async fn api_server(kv_store: Arc<Mutex<KVStore>>, sender: mpsc::Sender<(String, Vec<u8>)>, id: &u64) {
    // let kv_store: HashMap<String, u64> = HashMap::new();

    let state: ServerState = Arc::new( Mutex::new(HandlerData {
        kv_store,
        sender,
    }));

    println!("Registering routes");
    let app = Router::new()
        .route("/", get(hello_world))
        .route("/kv/:key/:value", get(put_kv))
        .route("/kv/:key", get(get_kv))
        .with_state(state);

    // have to convert id to u16 since SocketAddr doesn't accept u64
    // portn will be 0 if id > u16::max_value()
    let portn = id.to_owned() as u16;

    let addr = SocketAddr::from(([0, 0, 0, 0], 9000 + portn));

    println!("Starting server on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
