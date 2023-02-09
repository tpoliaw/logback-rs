use gumdrop::Options;
use std::{fs::File, io::Read, net::TcpStream, path::PathBuf, thread, time::Duration};
use yansi::{Color, Style};

use logback::LogLevel;
 
pub fn main() {
    let command = Command::parse_args_default_or_exit();

    let src: Box<dyn Read> = if let Some(file) = command.file {
        Box::new(File::open(file).unwrap())
    } else {
        let host = command.host.as_deref().unwrap_or("localhost");
        let port = command.port.unwrap_or(6750);
        loop {
            if let Ok(sock) = TcpStream::connect((host, port)) {
                println!("Connected to server");
                break Box::new(sock);
            }
            thread::sleep(Duration::from_millis(200));
        }
    };

    let mut reader = jaded::Parser::new(src).expect("failed to create parser");

    let mut count = 0;
    let threshold = command.level.unwrap_or(LogLevel::Info);
    loop {
        match reader.read_as::<logback::LogEvent>() {
            Ok(evt) => {
                if evt.level >= threshold {
                    let style = match evt.level {
                        LogLevel::Trace => Style::default().dimmed(),
                        LogLevel::Debug => Style::default(),
                        LogLevel::Info => Style::default().bold(),
                        LogLevel::Warn => Style::new(Color::Yellow),
                        LogLevel::Error => Style::new(Color::Red),
                        _ => Style::default(),
                    };
                    let dt = evt.time();
                    println!("{} {} {} {:.40} - {}", dt.date_naive(), dt.time(), evt.level, evt.logger_name, style.paint(evt.message()));
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

#[derive(Debug, Default, Options)]
struct Command {
    #[options(help = "Read log messages from file")]
    file: Option<PathBuf>,
    #[options(help = "Connect to server to read messages")]
    host: Option<String>,
    #[options(help = "Server port broadcasting log messages - default: 6750")]
    port: Option<u16>,
    startup: bool,
    level: Option<LogLevel>,
}
