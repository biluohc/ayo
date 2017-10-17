#![allow(dead_code)]

#[macro_use]
extern crate lazy_static;
extern crate num_cpus;
extern crate stderr;
extern crate mio;
extern crate slab;

// TcpStream::from_stream tokio-0.1.1也有类似api, 等tokio稳定了和futures一起折腾(tokio将大改).
// Tokio split了 I/O, 所以也不需要去分离Mio的I/O了

#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate time;

pub mod loops;
use loops::Server;
pub mod pool;
use pool::Pool;

fn main() {
    let addr = "0.0.0.0:8000";
    println!("http://{} : {} Workers", addr, Pool::len());

    Pool::run(addr).unwrap()
}
