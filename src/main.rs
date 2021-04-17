use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use std::net::{SocketAddr, TcpStream};
use anyeyeballs::{check_for_new_connection, get_interface, ThreadPool, MetaListener};
use std::io::{Read, Write};
use std::{fs};

const WORKERS: usize = 10;
const ADDR: &str = "127.0.0.1:80";

fn main() {

    // Number of total received packets
    let mut _packets = 0;
    // Create worker pool and set available worker variable
    let pool = ThreadPool::new(WORKERS).unwrap_or_else(|_|(panic!("workers: size has to be >0!")));
    let mut available_workers =  WORKERS;
    // Parse the address to SocketAddr and create the MetaListener object; start the listener
    let addr = ADDR.parse::<SocketAddr>().unwrap().into();
    let mut listener = MetaListener::new(addr);
    listener.start();
    // Get the correct interface and create a receive buffer for all incoming packets on that interface
    let interface = get_interface("lo0");
    let (mut _lx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("libpnet: unknown channel type: {}"),
        Err(e) => panic!("libpnet: unable to create new  ethernet channel: {}", e),
    };

    // Main program loop
    loop {
        // Check each packet incoming on specified interface
        match rx.next() {
            Ok(eth_packet) => {
                _packets += 1;
                // Call check_for_new_connection which checks if the packet is the first packet of a new TCP connection (SYN)
                if check_for_new_connection(eth_packet) {
                    println!("Got a new connection.");
                    // Check if we are currently listening
                    if listener.is_active() {
                        // If we still have worker threads available...
                        if available_workers > 0 {
                            // Accept the new connection
                            let stream = listener.get_listener().accept().unwrap().0;
                            // Reduce the amount of available workers
                            available_workers -= 1;
                            // Hand over task to worker; serve webpage
                            println!("Serving page");
                            pool.execute(|| {
                                handle_connection(stream);
                            });
                            // If that worker was the last one available, stop the listener
                            if available_workers == 0 {
                                listener.stop()
                            }
                        }
                    }

                }
            }
            Err(e) => panic!("libpnet: unable to receive packet: {}", e),
        }
    }
}


// Serves simple HTTP replies to a connection
fn handle_connection(mut stream: TcpStream) {
    // Create a buffer and read from TCP stream
    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();
    // If its a GET request, set response header and load hello.html
    let (status_line, filename) = if buffer.starts_with(b"GET") {
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else {
        panic!("http: received bad request!")
    };
    let contents = fs::read_to_string(filename).unwrap();
    // format HTTP response and write it on the tcp stream
    let response = format!("{}{}", status_line, contents);
    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}