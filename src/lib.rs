
use std::{thread, io, time, fs};
use std::sync::{Arc, Mutex, mpsc};
use std::net::{TcpListener, Ipv4Addr, Ipv6Addr, UdpSocket, IpAddr, Shutdown};
use socket2::{Socket, Domain, Type, SockAddr};
use std::mem::swap;
use std::str::FromStr;
use std::fmt::Error;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::thread::sleep;
use std::io::{Read, Write};
use std::time::SystemTime;

const RTT_THRESHOLD: f64 = 0.5;

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
        println!("{:?}", self.addr.family());
        if self.addr.family() == 2 {
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
        } else if self.addr.family() == 10 {
            let socket = Socket::new(Domain::ipv6(), Type::stream(), None).unwrap();
            loop {
                match socket.bind(&self.addr) {
                    Ok(_) => {break},
                    Err(ref e) if e.kind() == io::ErrorKind::AddrInUse => {
                        println!("Addr still busy....");
                        sleep(time::Duration::from_millis(100));
                        continue;
                    },
                    Err(e) => {
                        println!("{:?}",e);
                        panic!()}
                }
            }
            socket.listen(1).unwrap();
            socket.set_nonblocking(true);
            self.active.store(true, SeqCst);

            *self.listener.lock().unwrap() = Some(socket);
        }
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
        let ipv6: Vec<_> = ipv6.split("]").collect();
        let ipv4 = Ipv4Addr::from_str(ipv4[0]).unwrap();
        let ipv6 = Ipv6Addr::from_str(&ipv6[0][1..]).unwrap();
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

    pub fn send_status(&self, total_load: u8, ipv4_load: u8, ipv6_load: u8, v4_active: bool, v6_active: bool) -> Result<(), Error>{
        self.quic_connection.connect(&self.orch_addr).unwrap();
        let mut buf:Vec<u8> = Vec::with_capacity(5);
        buf.push(2_u8);
        buf.push(self.node_id);
        buf.push(total_load);
        buf.push(ipv4_load);
        buf.push(ipv6_load);
        if v4_active {
            buf.push(1)
        } else {
            buf.push(0)
        }
        if v6_active {
            buf.push(1)
        } else {
            buf.push(0)
        }
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

pub fn serve_connections(mut incoming: Arc<Mutex<Option<Socket>>>, mut active: Arc<AtomicBool>, mut available_workers: Arc<Mutex<usize>>, mut active_workers: Arc<Mutex<usize>>, pool: Arc<Mutex<ThreadPool>>, rtt_threshold_passed: Arc<AtomicBool>, rtt_ts: Arc<Mutex<SystemTime>>) {
    loop {
        if active.load(SeqCst) {
            sleep(time::Duration::from_millis(1));
            // If we still have worker threads available...
            if *available_workers.lock().unwrap() > 0 {
                let stream = incoming.lock().unwrap();
                let stream = match stream.as_ref() {
                    Some(i) => i,
                    None => continue
                };
                let stream = stream.accept();
                // Accept the new connection
                match stream {
                    Ok(stream) => {
                        // Reduce the amount of available workers and increase active workers v4
                        *available_workers.lock().unwrap() -= 1;
                        *active_workers.lock().unwrap() += 1;
                        // start the RTT ts if we passed the threshold
                        if (*available_workers.lock().unwrap() as f64) < (*available_workers.lock().unwrap() + *active_workers.lock().unwrap()) as f64 * RTT_THRESHOLD {
                            if !rtt_threshold_passed.load(SeqCst) {
                                *rtt_ts.lock().unwrap() = SystemTime::now();
                                rtt_threshold_passed.store(true, SeqCst);
                            }
                        }
                        // Clone worker counts for thread that handles connection
                        let avl_workers = available_workers.clone();
                        let act_workers = active_workers.clone();
                        // Hand over task to worker; serve webpage
                        println!("Serving page");
                        pool.lock().unwrap().execute(move || {
                            handle_connection(stream.0);
                            sleep(time::Duration::from_secs(60));
                            *avl_workers.lock().unwrap() += 1;
                            *act_workers.lock().unwrap() -= 1;
                        });
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    _ => {}
                }
            }
        }
    }
}

// Serves simple HTTP replies to a connection
fn handle_connection(mut stream: Socket) {
    // Create a buffer and read from TCP stream
    let mut buffer = [0; 512];
    stream.set_nonblocking(false);
    stream.read(&mut buffer).unwrap();
    // If its a GET request, set response header and load hello.html
    let (status_line, filename) = if buffer.starts_with(b"GET") {
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else {
        println!("http: received bad request!");
        stream.shutdown(Shutdown::Both);
        stream.flush().unwrap();
        return;
    };
    println!("Wrote response!");
    let contents = fs::read_to_string(filename).unwrap();
    // format HTTP response and write it on the tcp stream
    let response = format!("{}{}", status_line, contents);
    stream.write(response.as_bytes()).unwrap();
    stream.shutdown(Shutdown::Both);
    stream.flush().unwrap();
    drop(stream);
}