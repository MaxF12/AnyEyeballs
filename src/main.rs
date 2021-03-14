use pnet::datalink;
use pnet::datalink::Channel::Ethernet;
use pnet::packet::Packet;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::tcp::TcpPacket;
use std::net::{TcpListener, SocketAddr};

fn main() {
    use socket2::{Socket, Domain, Type, SockAddr};

    let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
    socket.bind(&"3.126.85.184:80".parse::<SocketAddr>().unwrap().into()).unwrap();
    socket.listen(1).unwrap();
    let listener = socket.into_tcp_listener();

    let interface = get_interface("lo0");
    let (mut lx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("libpnet: unknown channel type: {}"),
        Err(e) => panic!("libpnet: unable to create new  ethernet channel: {}", e),
    };

    loop {
        let mut buf: [u8; 1600] = [0u8; 1600];
        match rx.next() {
            Ok(eth_packet) => {
                let packet = EthernetPacket::new(eth_packet).unwrap();
                let packet = Ipv4Packet::new(packet.payload()).unwrap();
                let packet = TcpPacket::new(packet.payload()).unwrap();
                //let payload_offset;
                if  packet.get_destination() == 80 && packet.get_flags() == 2  {
                    println!("Got a new syn request!");
                    //lx.send_to(eth_packet, Option::from(get_interface("lo0")));
                }

                //println!("{:?}", packet);
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