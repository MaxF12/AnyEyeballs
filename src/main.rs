use std::net::{SocketAddr};
use anyeyeballs::{ThreadPool, MetaListener, State, Node, serve_connections};
use std::{thread, time};
use std::thread::{sleep};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::TryFrom;
use std::sync::atomic::AtomicBool;

const WORKERS: usize = 20;
const ADDR: &str = "127.0.0.1:9032";
const ADDR_V6: &str = "[::1]:9032";
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

    //RTT timestamp
    let mut rtt_ts = Arc::new(Mutex::new(SystemTime::now()));
    let mut rtt_threshold_passed = Arc::new(AtomicBool::new(false));
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
    let rtt_ts_v4 = rtt_ts.clone();
    let rtt_thrs_passed_v4 = rtt_threshold_passed.clone();
    let server_v4 = thread::spawn(move ||
        serve_connections(incoming_v4, active_v4, available_workers_server_v4, active_workers_server_v4, pool_v4, rtt_thrs_passed_v4, rtt_ts_v4)
    );

    // Clone Arcs and start v6 thread
    let active_v6 = Arc::clone(&listener_v6.active);
    let incoming_v6 = Arc::clone(&listener_v6.listener);
    let available_workers_server_v6 = available_workers.clone();
    let active_workers_server_v6 = active_workers_v6.clone();
    let pool_v6 = pool.clone();
    let rtt_ts_v6 = rtt_ts.clone();
    let rtt_thrs_passed_v6 = rtt_threshold_passed.clone();
    let server_v6 = thread::spawn(move ||
        serve_connections(incoming_v6, active_v6, available_workers_server_v6, active_workers_server_v6, pool_v6, rtt_thrs_passed_v6, rtt_ts_v6)
    );

    let connection = node.quic_connection.try_clone().unwrap();
    let active_v4 = Arc::clone(&listener_v4.active);
    let active_v6 = Arc::clone(&listener_v6.active);
    let orch_thread = thread::spawn(move || {
        loop {
            let mut buffer = [0; 10];
            connection.recv(&mut buffer).unwrap() as u8;
            println!("Got new message!");
            if buffer[0] == 3 {
                let ipv4_state = buffer[2];
                let ipv6_state = buffer[3];
                println!("IPv4 new state: {:?}", ipv4_state);
                let v4_active = listener_v4.active.load(SeqCst);
                if ipv4_state == 0 && v4_active {
                    listener_v4.stop();
                    if rtt_threshold_passed.load(SeqCst) {
                        let response_time = rtt_ts.lock().unwrap().elapsed().unwrap().as_millis();
                        println!("Response time was: {:?}", response_time);
                    }
                } else if ipv4_state == 2 && !v4_active {
                    listener_v4.start();
                    if rtt_threshold_passed.load(SeqCst) {
                        rtt_threshold_passed.store(false, SeqCst);
                    }
                }
                println!("IPv6 new state: {:?}", ipv6_state);
                let v6_active = listener_v6.active.load(SeqCst);
                if ipv6_state == 0 && v6_active {
                    listener_v6.stop();
                    if rtt_threshold_passed.load(SeqCst) {
                        let response_time = rtt_ts.lock().unwrap().elapsed().unwrap().as_millis();
                        println!("Response time was: {:?}", response_time);
                    }
                } else if ipv6_state == 2 && !v6_active {
                    listener_v6.start();
                    if rtt_threshold_passed.load(SeqCst) {
                        rtt_threshold_passed.store(false, SeqCst);
                    }
                }
            }
        }
    }
    );
    loop {
        println!("Sending status update");
        // Send status to orchestrator
        let avl_workers = *available_workers.lock().unwrap();
        let active_workers_v6 = *active_workers_v6.lock().unwrap();
        let active_workers_v4 = *active_workers_v4.lock().unwrap();
        node.send_status((WORKERS) as u8, (active_workers_v4) as u8, (active_workers_v6) as u8, active_v4.load(SeqCst), active_v6.load(SeqCst));
        sleep(time::Duration::from_secs(1));
        if avl_workers < WORKERS && active_v4.load(SeqCst) {
            println!("Not enough workers");
        }
        //connection.set_nonblocking(true);
    }
}