
// 这样还是不行, 某个socket 文件io阻塞会阻塞整个线程所有的连接,
// 还要基于rayor 写个依靠函数调用, 线程复用,通过channel通信的公共阻塞io库 --> futures-cpu-pool
//
// Unix Tcplisten + cpu-nums worker(envent-loop)?
// windows iocp单进程只支持一个event-loop?, 所以考虑windows只开一个worker,并且for listen同步分发.
//
// 文件async有下面这两个
// https://github.com/asomers/tokio-file
// https://github.com/Rufflewind/tokio-file-unix/
#![allow(deprecated)]
use mio::channel::{channel, SendError, Sender};
use pretty_env_logger as logger;
use stderr::StaticMut;
use num_cpus;

use std::thread::{Builder, JoinHandle};
// use std::time::Duration;
// use std::thread::sleep;
use std::sync::Arc;
use std::{io, net};

use super::Server;

pub type WTcpStream = (net::TcpStream, Arc<()>);

// #[derive(Debug)]
pub struct Worker {
    mp: StaticMut<Sender<WTcpStream>>,
    live: Arc<()>,
    count: StaticMut<usize>,
    id: usize,
    join_handle: JoinHandle<()>,
}

impl Worker {
    fn new(mp: Sender<WTcpStream>, id: usize, join_handle: JoinHandle<()>) -> Self {
        Worker {
            mp: StaticMut::new(mp),
            live: Arc::from(()),
            count: StaticMut::new(0),
            id: id,
            join_handle: join_handle,
        }
    }
    fn send(&self, sender: net::TcpStream) -> Result<(), SendError<WTcpStream>> {
        self.mp.as_ref().send((sender, Arc::clone(&self.live)))
    }
    fn live(&self) -> usize {
        // sub own
        Arc::strong_count(&self.live) - 1
    }
    fn count(&self) -> &usize {
        self.count.as_ref()
    }
    fn counter(&self) -> &usize {
        *self.count.as_mut() += 1;
        self.count.as_ref()
    }
    fn id(&self) -> &usize {
        &self.id
    }
}



// #[derive(Debug)]
pub struct Pool {
    pub workers: Vec<Worker>,
    pub count: StaticMut<usize>,
}

impl Pool {
    pub fn new() -> io::Result<Self> {
        let cpus = if cfg!(unix) { num_cpus::get() } else { 1usize };
        let mut loops = Vec::with_capacity(cpus);
        for idx in 0..cpus {
            let (mp, sc) = channel::<WTcpStream>();
            let join_handle =
                Builder::new().spawn(move || if let Err(e) = Server::new(sc, idx).event_loop() {
                    panic!("Worker({}) start failed: {:?}", idx, e);
                })?;
            loops.push(Worker::new(mp, idx, join_handle));
        }
        Ok(Pool {
            workers: loops,
            count: StaticMut::new(0usize),
        })
    }
    pub fn send(sender: net::TcpStream) -> Result<(), SendError<WTcpStream>> {
        Self::as_ref()
            .workers
            .as_slice()
            .iter()
            .min_by_key(|w| w.count())
            .map(|w| {
                w.send(sender).map(|_| {
                    Self::as_ref().counter();
                    w.counter();
                    debug!(
                        "Client({}) -> Woker({:02}): live/all -> {}/{}",
                        Self::as_ref().count(),
                        w.id(),
                        w.live(),
                        w.count()
                    );
                })
            })
            .unwrap()
    }
    pub fn as_ref() -> &'static Self {
        &POOL
    }
    pub fn len() -> usize {
        Self::as_ref().workers.len()
    }
    pub fn is_empty() -> bool {
        Self::as_ref().workers.is_empty()
    }
    pub fn counter(&self) -> &usize {
        *self.count.as_mut() += 1;
        self.count.as_ref()
    }
    fn count(&self) -> &usize {
        self.count.as_ref()
    }
}

lazy_static! {
       static ref POOL:Pool =  Pool::new().unwrap();
}


impl Pool {
    pub fn run<A: net::ToSocketAddrs>(socket_addr: A) -> io::Result<()> {
        logger::init().unwrap();
        let listener = net::TcpListener::bind(socket_addr).unwrap();
        let mut count = 0usize;
        let mut count24 = 0usize;
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    count += 1;
                    Pool::send(stream)
                        .map_err(|e| error!("run() -> Pool::send(): {:?}", e))
                        .unwrap();
                }
                // Error { repr: Os { code: 24, message: "Too many open files" } }
                Err(ref e) if e.raw_os_error() == Some(24) => {
                    count24 += 1;
                    error!("{}/{}: {:?}", count24, count, e);
                    // sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    error!("{}: {:?}", count, e);
                }
            }
        }
        Ok(())
    }
}
