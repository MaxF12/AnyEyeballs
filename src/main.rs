use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use std::net::{SocketAddr};
use socket2::{Socket, Domain, Type};
use anyeyeballs::check_for_new_connection;
use std::sync::atomic::{AtomicI32, Ordering};

const WORKERS: i32 = 4;

fn main() {

    let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
    socket.bind(&"127.0.0.1:80".parse::<SocketAddr>().unwrap().into()).unwrap();
    socket.listen(1).unwrap();
    let available_workers =  AtomicI32::new(WORKERS);
    let _listener = socket.into_tcp_listener();

    let interface = get_interface("en0");
    let (mut _lx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("libpnet: unknown channel type: {}"),
        Err(e) => panic!("libpnet: unable to create new  ethernet channel: {}", e),
    };

    loop {
        match rx.next() {
            Ok(eth_packet) => {
                if check_for_new_connection(eth_packet) {
                    println!("Got a new connection.");
                    if available_workers.load(Ordering::SeqCst) > 0 {

                    }
                }
            }
            Err(e) => panic!("libpnet: unable to receive packet: {}", e),
        }
    }
}

pub fn get_interface(iface: &str)  -> datalink::NetworkInterface {
    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    interfaces.into_iter().filter(|i|i.name == iface)
        .next().unwrap_or_else(|| panic!("No such network interface: {}", iface))
}