use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use std::net::{SocketAddr, TcpStream};
use socket2::{Socket, Domain, Type};
use anyeyeballs::{check_for_new_connection, get_interface, ThreadPool};
use std::io::{Read, Write};
use std::{thread, fs};
use std::time::Duration;
use smoltcp::socket::{TcpSocketBuffer, TcpSocket};
use smoltcp::wire::{IpEndpoint, IpAddress, Ipv4Address};

const WORKERS: usize = 10;

fn main() {
    let pool = ThreadPool::new(WORKERS).unwrap_or_else(|_|(panic!("size has to be >0!")));
    let mut packets = 0;

    // smoltcp
    let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 64]);
    let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 128]);
    let mut tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);
    let endpoint = IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::new(127,0,0,1)), 8888);
    tcp_socket.listen(endpoint).unwrap();
    // Socket 2
    //let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
    //socket.bind(&"172.31.38.115:80".parse::<SocketAddr>().unwrap().into()).unwrap();
    //socket.listen(1).unwrap();
    // Workers aka threads
    let mut available_workers =  WORKERS;
    //let listener = socket.into_tcp_listener();

    let interface = get_interface("lo0");
    let (mut _lx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("libpnet: unknown channel type: {}"),
        Err(e) => panic!("libpnet: unable to create new  ethernet channel: {}", e),
    };

    loop {
        if !tcp_socket.is_open() {
            tcp_socket.listen(endpoint).unwrap();
        } else if tcp_socket.can_send() {
            println!("yuff")
            //writeln!(tcp_socket, "hello").unwrap();
        } else if tcp_socket.is_open(){
            println!("Socket oops!")
        }

        /*match rx.next() {
            Ok(eth_packet) => {
                packets += 1;
                //println!("Packet number {}", packets);
                if check_for_new_connection(eth_packet) {
                    println!("Got a new connection.");
                    if available_workers > 0 {
                      let stream = listener.accept().unwrap().0;
                        println!("Serving page");
                        available_workers -= 1;
                        pool.execute(|| {
                            handle_connection(stream);
                        });
                    }
                }
            }
            Err(e) => panic!("libpnet: unable to receive packet: {}", e),
        }*/
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
    } else {
        ("HTTP/1.1 404 NOT FOUND\r\n\r\n", "404.html")
    };


    let contents = fs::read_to_string(filename).unwrap();

    let response = format!("{}{}", status_line, contents);
    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();

}