use pnet::packet::Packet;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};
use pnet::packet::tcp::TcpPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::datalink;

use std::{thread, io, time};
use std::sync::{Arc, Mutex, mpsc};
use pnet::util::MacAddr;
use std::net::{TcpListener, Ipv4Addr, Ipv6Addr, UdpSocket, IpAddr, Shutdown};
use socket2::{Socket, Domain, Type, SockAddr};
use std::mem::swap;
use std::str::FromStr;
use std::fmt::Error;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::thread::sleep;

/// Checks if the packet is a TCP packet with the SYN flag set and destination port 80
///
/// The packet is the packet to be analyzed
///
/// Returns true if packet is first of a new connection, false else
pub fn check_for_new_connection(eth_packet: &[u8]) -> bool {
    let packet = EthernetPacket::new(eth_packet).unwrap();
    if  packet.get_source() == MacAddr::new(0x06,0x7b,0x45,0x7f,0xcf,0x64) {return false;}
    println!("Correct MAC Addr: {}", packet.get_source());
    match packet.get_ethertype() {
        EtherTypes::Ipv4 => {
            let packet = Ipv4Packet::new(packet.payload()).unwrap();
            if  packet.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {return false;}
            let packet = TcpPacket::new(packet.payload()).unwrap();
            if  packet.get_destination() == 80 && packet.get_flags() == 2  {
                println!("Got a new syn request!");
                true
            } else {
                if packet.get_destination() != 53431 && packet.get_destination() != 22{
                    println!("TCP wasnt correct, flag: {};port: {}",packet.get_flags(),  packet.get_destination());
                }
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
                if packet.get_destination() != 53431 {
                    println!("TCP wasnt correct, flag: {};port: {}",packet.get_flags(),  packet.get_destination());
                }
                false
            }
        }
        _ => {
            println!("Got different packet: {}", packet.get_ethertype());
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


pub fn get_interface(iface: &str)  -> datalink::NetworkInterface {
    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    interfaces.into_iter().filter(|i|i.name == iface)
        .next().unwrap_or_else(|| panic!("No such network interface: {}", iface))
}


// Code related to running multiple threads
enum Message {
    NewJob(Job),
    Terminate
}
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>
}

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}
type Job = Box<dyn FnBox + Send + 'static>;

pub struct PoolCreationError;
impl ThreadPool {
    pub fn new(size: usize) -> Result<ThreadPool, PoolCreationError> {
        if size == 0 {
            Err(PoolCreationError)
        } else {

            let (sender, receiver) = mpsc::channel();

            let receiver = Arc::new(Mutex::new(receiver));
            let mut workers = Vec::with_capacity(size);
            for id in 0..size{
                workers.push(Worker::new(id, Arc::clone(&receiver)))
            }
            Ok(ThreadPool{workers, sender})
        }

    }

    pub fn execute<F>(&self, f:F)
        where F: FnOnce() + Send + 'static
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &mut self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        for worker in &mut self.workers{
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv().unwrap();

                match message {
                    Message::NewJob(job) => {
                        println!("Worker {} got a job; executing.", id);
                        job.call_box();
                    },
                    Message::Terminate => {
                        println!("Worker {} was told to terminate.", id);
                        break;
                    }
                }
            }});
        Worker {
            id,
            thread: Some(thread)
        }
    }
}


pub struct MetaListener {
    pub listener: Arc<Mutex<Option<Socket>>>,
    pub active: Arc<AtomicBool>,
    addr: SockAddr
}

pub struct ListenerError;
impl MetaListener {
    pub fn new(addr: SockAddr) -> MetaListener {
        MetaListener{ listener: Arc::new(Mutex::new(None)), addr, active: Arc::new(AtomicBool::new(false)) }
    }

    pub fn start(&mut self) {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        loop {
            match socket.bind(&self.addr) {
                Ok(_) => {break},
                Err(ref e) if e.kind() == io::ErrorKind::AddrInUse => {
                    println!("Addr still busy....");
                    sleep(time::Duration::from_millis(100));
                    continue;
                },
                Err(_) => {panic!()}
            }
        }
        socket.listen(1).unwrap();
        socket.set_nonblocking(true);
        self.active.store(true, SeqCst);

        *self.listener.lock().unwrap() = Some(socket);
    }

    pub fn stop(&mut self) {
        self.listener.lock().unwrap().as_ref().unwrap().shutdown(Shutdown::Both);
        self.active.store(false, SeqCst);
        let mut dropped = None;
        swap(&mut dropped, &mut self.listener.lock().unwrap());
        println!("Closing listener: {:?}", dropped);
        drop(dropped.unwrap());
        *self.listener.lock().unwrap()=None;
    }

}

pub enum State {
    Idle,
    Pending,
    NoActive,
    V4Active,
    V6Active,
    BothActive
}


pub struct Node {
    pub quic_connection: UdpSocket,
    orch_addr: String,
    ipv4: Ipv4Addr,
    ipv6: Ipv6Addr,
    node_id: u8
}

impl Node {
    pub fn new(orch_addr: &str, ipv4: &str, ipv6: &str) -> Node {
        let quic_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let ipv4: Vec<_> = ipv4.split(":").collect();
        let ipv4 = Ipv4Addr::from_str(ipv4[0]).unwrap();
        let ipv6 = Ipv6Addr::from_str(ipv6).unwrap();
        let orch_addr = orch_addr.clone().to_owned();
        Node { quic_connection: quic_socket, orch_addr, ipv4, ipv6, node_id: 0 }
    }

    pub fn send_join(&mut self) -> Result<(), Error>{
        self.quic_connection.connect(&self.orch_addr).unwrap();
        let mut buf:Vec<u8> = Vec::with_capacity(21);
        // Flag 000 for join
        buf.push(0_u8);
        for oct in self.ipv4.octets().iter() {
            buf.push(*oct);
        }
        for oct in self.ipv6.octets().iter() {
            buf.push(*oct);
        }
        println!("{:?}", buf);

        self.quic_connection.send(&*buf).unwrap();
        let mut buffer = [0; 10];
        self.quic_connection.recv(&mut buffer).unwrap() as u8;
        if buffer[0] == 4 {
            self.node_id = buffer[1];
        }
        Ok(())
    }

    pub fn send_status(&self, total_load: u8, ipv4_load: u8, ipv6_load: u8) -> Result<(), Error>{
        self.quic_connection.connect(&self.orch_addr).unwrap();
        let mut buf:Vec<u8> = Vec::with_capacity(5);
        buf.push(2_u8);
        buf.push(self.node_id);
        buf.push(total_load);
        buf.push(ipv4_load);
        buf.push(ipv6_load);
        self.quic_connection.send(&*buf).unwrap();
        Ok(())
    }
    pub fn get_node_id(&self) -> u8 {
        self.node_id
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        println!("Sending leave to orchestrator.");
        self.quic_connection.connect(&self.orch_addr).unwrap();
        let mut buf:Vec<u8> = Vec::with_capacity(2);
        buf.push(2_u8);
        buf.push(self.node_id);
        self.quic_connection.send(&*buf).unwrap();
    }
}