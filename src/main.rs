use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use std::net::{SocketAddr, TcpStream};
use anyeyeballs::{check_for_new_connection, get_interface, ThreadPool, MetaListener};
use std::io::{Read, Write};
use std::{thread, fs};
use std::time::Duration;

const WORKERS: usize = 10;

fn main() {
    let pool = ThreadPool::new(WORKERS).unwrap_or_else(|_|(panic!("size has to be >0!")));
    let mut _packets = 0;

    let mut available_workers =  WORKERS;

    let addr = "127.0.0.1:80".parse::<SocketAddr>().unwrap().into();
    let mut listener = MetaListener::new(addr);
    listener.start_listener();

    let interface = get_interface("lo0");
    let (mut _lx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("libpnet: unknown channel type: {}"),
        Err(e) => panic!("libpnet: unable to create new  ethernet channel: {}", e),
    };

    loop {
        match rx.next() {
            Ok(eth_packet) => {
                _packets += 1;
                //println!("Packet number {}", packets);
                if check_for_new_connection(eth_packet) {
                    println!("Got a new connection.");
                    if listener.is_active() {
                        if available_workers > 0 {
                            let stream = listener.get_listener().accept().unwrap().0;
                            println!("Serving page");
                            available_workers -= 1;
                            pool.execute(|| {
                                handle_connection(stream);
                            });
                        } else {
                            listener.stop_listener();
                        }
                    }

                }
            }
            Err(e) => panic!("libpnet: unable to receive packet: {}", e),
        }
    }
}



fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();

    println!("Got message: {:?}", buffer);
    let get = b"GET / HTTP/1.1\r\n";
    let sleep = b"GET /sleep HTTP/1.1\r\n";

    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else if  buffer.starts_with(sleep){
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND\r\n\r\n", "404.html")
    };


    let contents = fs::read_to_string(filename).unwrap();

    let response = format!("{}{}", status_line, contents);
    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();

}