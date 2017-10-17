
#![allow(deprecated)] 
use mio::{Events, Ready, Poll, PollOpt, Token};
use mio::channel::Receiver;
use mio::net::TcpStream;
use slab::Slab;

use super::pool::WTcpStream;

use std::io::{self, Write, Read};
use std::io::ErrorKind::WouldBlock;
use std::sync::Arc;
use std::usize;

#[derive(Debug)]
pub struct Client {
    pub socket: TcpStream,
    pub reader: Vec<u8>,
    pub writer: Vec<u8>,
    writer_bytes: usize, // offset for write woudleblock
    pub done: bool,
    live: Arc<()>,
}

impl Client {
    // HTTP's keepalive(not tcp's) reset status: ready for new read/write?
    pub fn keep_alive(&mut self) -> io::Result<()> {
        // verify keep-alive
        let client_addr = self.socket.peer_addr()?;
        debug!("keep-alive: {}", client_addr);
        self.reader =Vec::new();
        self.writer = Vec::new();
        self.writer_bytes = 0;
        self.done = false;
        Ok(())
    }
    pub fn new(socket: TcpStream, live: Arc<()>) -> Self {
        Client {
            socket: socket,
            reader: Vec::new(),
            writer: Vec::new(),
            writer_bytes: 0usize, // offset for write woudleblock
            // read_write alrendy complete once
            done: false,
            live: live,
        }
    }
    pub fn read(&mut self) -> io::Result<()> {
        let mut buf = [0; 1024];
        loop {
            match self.socket.read(&mut buf) {
                Ok(size) => {
                    if size == 0 {
                        return Ok(());
                    } else {
                        self.reader.extend_from_slice(&buf[..size]);
                    }
                }
                Err(err) => {
                    if WouldBlock == err.kind() {
                        return Ok(());
                    }
                    error!("Client.read(): {:?}", err);
                    return Err(err);
                }
            }
        }
    }
    pub fn write(&mut self) -> io::Result<()> {
        // 临时
        let send_head = "HTTP/1.1 200 OK\nContent-Type: text/html;charset=UTF-8\nContent-Length: ";
        let send_body = "<!DOCTYPE HTML><html><body>Hello Web!| 我 |Love Rust!</body></html>";
        let data_str = format!("{}{}\n\n{}", send_head, send_body.len(), send_body);
        let data = data_str.as_bytes();

        loop {
            match self.socket.write(&data[self.writer_bytes..]) {
                // continue wait writeable
                Ok(len) => {
                    self.writer_bytes += len;
                    // write complete
                    if self.writer_bytes == data_str.len() {
                        self.done = true;
                        return Ok(());
                    }
                }
                Err(err) => {
                    if WouldBlock == err.kind() {
                        return Ok(());
                    }
                    // other error, close socket
                    error!("Client.write(): {:?}", err);
                    return Err(err);
                }
            }
        }
    }
}

use std::fmt;
impl fmt::Debug for Server {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Server")
           .field("clients",&self.clients)
           .field("id",&self.id)
           .field("poll",&self.poll)
           .field("events",&self.events)
           .finish()
    }
}

pub struct Server {
    pub clients: Slab<Client>,
    pub id: usize,
    pub poll: Poll,
    pub events: Events,
    pub sc: Receiver<WTcpStream>,
}

const CLIENTS_ADVENT: Token = Token(usize::MAX - 1);
impl Server {
    pub fn new(sc: Receiver<WTcpStream>, id: usize) -> Self {
        let poll = Poll::new()
            .map_err(|e| error!("Server::new({})#Poll::new(): {:?}", &id, e))
            .unwrap();
        let clients = Slab::with_capacity(1024);
        let events = Events::with_capacity(1024);


        poll.register(&sc, CLIENTS_ADVENT, Ready::readable(), PollOpt::edge())
            .map_err(|e| {
                error!("Server::new({})#poll.register(&sc): {:?}", &id, e)
            })
            .unwrap();

        Server {
            clients: clients,
            poll: poll,
            events: events,
            id: id,
            sc: sc,
        }

    }
    pub fn len(&self) -> usize {
        self.clients.len()
    }
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }
    pub fn id(&self) -> &usize {
        &self.id
    }
}


impl Server {
    pub fn event_loop(&mut self) -> io::Result<()> {
        loop {
            self.poll
                .poll(&mut self.events, None)
                .map_err(|e| error!("pool.poll(): {:?}", e))
                .unwrap();
            for event in &self.events {
                debug!(
                    "Worker({:02}) -> events/clients: {}/{}",
                    self.id(),
                    self.events.len(),
                    self.len()
                );

                match event.token() {
                    CLIENTS_ADVENT => {
                        // empty/disconnet? don't need consider
                        while let Ok((wts, count)) = self.sc.try_recv() {
                            if let Ok(socket) = TcpStream::from_stream(wts) {
                                let client = Client::new(socket, count);
                                let entry = self.clients.vacant_entry();
                                let token = Token(entry.key());
                                // 应该先监听read,处理完数据(得出响应内容), 再监听写? KeepAlive是不是太烦
                                self.poll
                                    .register(
                                        &client.socket,
                                        token,
                                        Ready::readable() | Ready::writable(),
                                        PollOpt::edge(),
                                    )
                                    .map_err(|e| error!("pool.register(socket): {:?}", e))
                                    .map(|_| entry.insert(client))
                                    .unwrap();
                            }
                        }
                    }
                    token => {
                        if  !self.clients.contains(token.0) {
                            continue;
                        }
                        if event.readiness().is_hup() ||
                            event.readiness().is_error()
                        {
                            self.poll.deregister(&self.clients.remove(token.0).socket).map_err(|e| {
                            error!("event.readiness().is_hup()|.is_error() -> poll.deregister(&clients.socket): {:?}", e)
                        }).unwrap()
                        }

                        if event.readiness().is_readable() {
                            let close = self.clients
                                .get_mut(token.0)
                                .map(|x| x.read().is_err())
                                .unwrap_or(false);
                            if close {
                                let client = self.clients.remove(token.0);
                                self.poll.deregister(&client.socket).map_err(|e| {
                            error!("event.readiness().is_hup()|.is_error() -> poll.deregister(&clients.socket): {:?}", e)
                        }).unwrap();
                            }
                        }
                        if event.readiness().is_writable() {
                            let mut close = false;
                            {
                                if let Some(client) = self.clients.get_mut(token.0) {
                                    // 是错误, 应该停止.
                                    close = client.write().is_err();
                                    // 非错误 -> Keep-Alive
                                    if !close {
                                        // 重置错误, 比如对方已经关闭连接, 应该停止.
                                        close = client.keep_alive().is_err();
                                    }
                                }
                            }
                            if close {
                                // 这一轮已经读写完毕,  如果出错或不keepAlive就应该取消事件关注, drop 连接.
                                let client = self.clients.remove(token.0);
                                self.poll
                                    .deregister(&client.socket)
                                    .map_err(|e| error!("poll.deregister(&conn.socket): {:?}", e))
                                    .unwrap();
                            }
                        }
                    }
                }
            }
        }

    }
}
