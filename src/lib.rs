use pnet::packet::Packet;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};
use pnet::packet::tcp::TcpPacket;
use pnet::packet::ip::IpNextHeaderProtocols;

/// Checks if the packet is a TCP packet with the SYN flag set and destination port 80
///
/// The packet is the packet to be analyzed
///
/// Returns true if packet is first of a new connection, false else
pub fn check_for_new_connection(eth_packet: &[u8]) -> bool {
    let packet = EthernetPacket::new(eth_packet).unwrap();
    match packet.get_ethertype() {
        EtherTypes::Ipv4 => {
            let packet = Ipv4Packet::new(packet.payload()).unwrap();
            if  packet.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {return false;}
            let packet = TcpPacket::new(packet.payload()).unwrap();
            if  packet.get_destination() == 80 && packet.get_flags() == 2  {
                println!("Got a new syn request!");
                true
            } else {
                false
            }
        },
        EtherTypes::Ipv6 =>  {
            let packet = Ipv6Packet::new(packet.payload()).unwrap();
            if  packet.get_next_header() != IpNextHeaderProtocols::Tcp {return false;}
            let packet = TcpPacket::new(packet.payload()).unwrap();
            if  packet.get_destination() == 80 && packet.get_flags() == 2  {
                println!("Got a new syn request!");
                true
            } else {
                false
            }
        }
        _ => {
            return false;
        }
    }
}

/// Sends a TCP packet with the RST flag set in response to the packet
///
/// The packet is the packet to which to respond
pub fn send_rst(_eth_packet: &[u8]) {
    //TODO send a RST
}