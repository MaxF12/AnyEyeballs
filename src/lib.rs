use pnet::packet::Packet;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};
use pnet::packet::tcp::TcpPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::datalink;

use std::thread;
use std::sync::{Arc, Mutex, mpsc};
use pnet::util::MacAddr;
use std::net::{TcpListener};
use socket2::{Socket, Domain, Type, SockAddr};
use std::mem::swap;

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
    /// Create a new ThreadPool
    ///
    /// The size is the number of threads
    ///
    /// # Panics
    ///
    /// The 'new' function will panic if the size is zero.
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
                        println!("Worker {} was told to temrinate.", id);

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
    listener: Option<TcpListener>,
    active: bool,
    addr: SockAddr
}

pub struct ListenerError;
impl MetaListener {
    pub fn new(addr: SockAddr) -> MetaListener {
        MetaListener{ listener: None, addr, active: false }
    }

    pub fn start_listener(&mut self) {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        socket.bind(&self.addr).unwrap();
        socket.listen(1).unwrap();
        self.active = true;

        self.listener = Some(socket.into_tcp_listener());
    }

    pub fn stop_listener(&mut self) {
        self.active =false;
        let mut dropped = None;
        swap(&mut dropped, &mut self.listener);
        println!("Closing listener: {:?}", dropped);
        drop(dropped.unwrap());
        self.listener=None;
    }

    pub fn get_listener(&self) -> &TcpListener {
        self.listener.as_ref().unwrap()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}