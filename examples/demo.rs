use std::{fs::File, net::{SocketAddr, TcpStream, ToSocketAddrs}, thread::{self, Thread}, time::Duration};

use logback::LogLevel;

pub fn main() {
    // let src = loop {
        // if let Ok(sock) = TcpStream::connect(("localhost", 6750)) {
        //     break sock;
        // }
        // thread::sleep(Duration::from_millis(200));
    // };
    // println!("Connected to server");
    let src = File::open("/tmp/i22_log.log").unwrap();
    let mut reader = jaded::Parser::new(src).expect("failed to create parser");

    let mut count = 0;
    loop {
        let evt = reader.read().unwrap();
        use jaded::FromJava;
        match logback::LogEvent::from_value(evt.value()){
            Ok(evt) => {
                if evt.level >= LogLevel::Info {
                    println!("{}", evt.message());
                }
                count += 1;
                if let Some(_) = &evt.marker {
                    println!("Read {} messages", count);
                    break;
                }
            },
            Err(e) => {
                println!("{}", e);
            }
        }
    }
}
