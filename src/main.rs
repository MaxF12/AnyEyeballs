use std::net::{SocketAddr};
use anyeyeballs::{ThreadPool, MetaListener, State, Node, serve_connections};
use std::{thread, time};
use std::thread::{sleep};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

const WORKERS: usize = 20;
const ADDR: &str = "127.0.0.1:9033";
const ADDR_V6: &str = "[::1]:9033";
const ORCH_ADDR: &str = "127.0.0.1:7722";

fn main() {
    // Node state
    let mut _state = State::Idle;
    // Create worker pool and set available worker variable
    let pool = Arc::new(Mutex::new(ThreadPool::new(WORKERS).unwrap_or_else(|_|(panic!("workers: size has to be >0!")))));
    // Workers counts
    let mut available_workers =  Arc::new(Mutex::new(WORKERS));
    let mut active_workers_v4 = Arc::new(Mutex::new(0));
    let mut active_workers_v6 = Arc::new(Mutex::new(0));

    // Parse the addresses to SocketAddr and create the MetaListener objects; start the listeners
    let addr = ADDR.parse::<SocketAddr>().unwrap().into();
    let mut listener_v4 = MetaListener::new(addr);
    listener_v4.start();
    let addr = ADDR_V6.parse::<SocketAddr>().unwrap().into();
    let mut listener_v6 = MetaListener::new(addr);
    listener_v6.start();

    // Create connection to Orchestrator
    let mut node = Node::new(ORCH_ADDR, ADDR, ADDR_V6);
    node.send_join();
    println!("Node ID: {:?}", node.get_node_id());

    // Clone Arcs and start v4 thread
    let active_v4 = Arc::clone(&listener_v4.active);
    let incoming_v4 = Arc::clone(&listener_v4.listener);
    let available_workers_server_v4 = available_workers.clone();
    let active_workers_server_v4 = active_workers_v4.clone();
    let pool_v4 = pool.clone();
    let server_v4 = thread::spawn(move ||
        serve_connections(incoming_v4, active_v4, available_workers_server_v4, active_workers_server_v4, pool_v4)
    );

    // Clone Arcs and start v6 thread
    let active_v6 = Arc::clone(&listener_v6.active);
    let incoming_v6 = Arc::clone(&listener_v6.listener);
    let available_workers_server_v6 = available_workers.clone();
    let available_workers_server_v6 = available_workers.clone();
    let active_workers_server_v6 = active_workers_v6.clone();
    let pool_v6 = pool.clone();
    let server_v6 = thread::spawn(move ||
        serve_connections(incoming_v6, active_v6, available_workers_server_v6, active_workers_server_v6, pool_v6)
    );

    let connection = node.quic_connection.try_clone().unwrap();
    let active_v4 = Arc::clone(&listener_v4.active);
    let orch_thread = thread::spawn(move || {
        loop {
            let mut buffer = [0; 10];
            connection.recv(&mut buffer).unwrap() as u8;
            println!("Got new message!");
            if buffer[0] == 3 {
                let ipv4_state = buffer[2];
                let ipv6_state = buffer[3];
                println!("IPv4 new state: {:?}", ipv4_state);
                if ipv4_state == 0 && listener_v4.active.load(SeqCst) {
                    listener_v4.stop();
                } else if ipv4_state == 1 && !listener_v4.active.load(SeqCst) {
                    listener_v4.start();
                }
                println!("IPv6 new state: {:?}", ipv6_state);
                if ipv6_state == 0 && listener_v6.active.load(SeqCst) {
                    listener_v6.stop();
                } else if ipv4_state == 1 && !listener_v6.active.load(SeqCst) {
                    listener_v6.start();
                }
            }
        }
    }
    );
    loop {
        println!("Sending status update");
        // Send status to orchestrator
        let avl_workers = *available_workers.lock().unwrap();
        node.send_status((WORKERS - avl_workers) as u8, (WORKERS - avl_workers) as u8, 0, active_v4.load(SeqCst), false);
        sleep(time::Duration::from_secs(1));
        if avl_workers < WORKERS && active_v4.load(SeqCst) {
            println!("Not enough workers");
        }
        //connection.set_nonblocking(true);
    }
}