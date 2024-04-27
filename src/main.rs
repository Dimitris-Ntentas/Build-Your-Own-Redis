// Uncomment this block to pass the first stage
use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};
use tokio::time::{Duration, Instant};

const PING_RESPONSE: &'static [u8] = b"+PONG\r\n";
const SET_RESPONSE: &'static [u8] = b"+OK\r\n";

const PING_COMMAND: &'static [u8] = b"\r\n$4\r\nping\r\n";
const ECHO_COMMAND: &'static [u8] = b"\r\n$4\r\necho\r\n";
const SET_COMMAND: &'static [u8] = b"\r\n$3\r\nset\r\n";
const GET_COMMAND: &'static [u8] = b"\r\n$3\r\nget\r\n";
const INFO_COMMAND: &'static [u8] = b"\r\n$4\r\nINFO\r\n";

#[derive(Debug)]
struct Storage {
    name: String,
    value: String,
    expiry: Option<Duration>,
    stored_at: Instant,
}

impl Storage {
    fn new(name: String, value: String, expiry: Option<Duration>) -> Self {
        Storage {
            name,
            value,
            expiry,
            stored_at: Instant::now(),
        }
    }
    fn has_expired(&self) -> bool {
        match self.expiry {
            Some(expiry) => self.stored_at.elapsed() > expiry,
            None => false,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let port: String;
    if args.len() > 1 && (&args[1] == "--port" || (&args[1] == "-p")) {
        port = args[2].clone();
    } else {
        port = String::from("6379");
    }

    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let addr = format!("127.0.0.1:{}", port);

    println!("Listening on: {}", addr);

    let listener = TcpListener::bind(addr).unwrap();

    for stream in listener.incoming() {
        thread::spawn(move || match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        });
    }
}

fn handle_client(mut stream: TcpStream) {
    // buffer to read and store 512 bytes
    let mut buf = [0; 512];
    let mut storage: Vec<Storage> = Vec::new();
    loop {
        // read from the stream and store the number of bytes read
        let bytes_read = stream.read(&mut buf).expect("Failed to read from client");

        if bytes_read == 0 {
            return;
        }

        match buf {
            b if b[2..].starts_with(PING_COMMAND) => {
                stream
                    .write_all(PING_RESPONSE)
                    .expect("Failed to write to client");
            }
            b if b[2..].starts_with(ECHO_COMMAND) => {
                let echo_len = ECHO_COMMAND.len();
                // Collect elements from buf[echo_len..] into a Vec<u8>
                let response: Vec<u8> = buf[echo_len + 2..]
                    .iter()
                    .take_while(|&&x| x != 0)
                    .cloned()
                    .collect();

                // Create a slice from the whole Vec<u8>
                let slice: &[u8] = &response;
                stream.write_all(slice).expect("Failed to write to client");
            }
            b if b[2..].starts_with(SET_COMMAND) => {
                let args = find_args(buf, SET_COMMAND.len());
                let name = args.get(0).unwrap().to_string();
                let value = args.get(1).unwrap().to_string();

                if args.len() == 4 && args.get(2).unwrap() == "px" {
                    let expiry = args.get(3).unwrap().parse::<u64>().unwrap();
                    storage.push(Storage::new(
                        name,
                        value,
                        Some(Duration::from_millis(expiry)),
                    ));
                } else {
                    storage.push(Storage::new(name, value, None));
                }

                stream
                    .write_all(SET_RESPONSE)
                    .expect("Failed to write to client");
            }
            b if b[2..].starts_with(GET_COMMAND) => {
                let get_len = GET_COMMAND.len();
                let args = find_args(buf, get_len);
                let arg = args.get(0).unwrap();
                println!("arg: {}", arg);
                storage.iter().for_each(|storage| {
                    if storage.name == *arg && !storage.has_expired() {
                        let response = format!("${}\r\n{}\r\n", storage.value.len(), storage.value);
                        stream
                            .write_all(response.as_bytes())
                            .expect("Failed to write to client");
                    } else {
                        dbg!("Expired");
                        stream
                            .write_all(b"$-1\r\n")
                            .expect("Failed to write to client");
                    }
                });
            }
            b if b[2..].starts_with(INFO_COMMAND) => {
                let info_len = INFO_COMMAND.len() + 2;
            }
            _ => {
                stream
                    .write_all(b"-ERR unknown command\r\n")
                    .expect("Failed to write to client");
            }
        }
    }
}

fn find_args(buf: [u8; 512], command_len: usize) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let parts = buf[command_len + 2..]
        .split(|b| *b == b'\r' || *b == b'\n')
        .filter(|&part| !part.is_empty())
        .collect::<Vec<&[u8]>>();
    for (index, part) in parts.iter().enumerate() {
        if index % 2 != 0 {
            match std::str::from_utf8(part) {
                Ok(s) => args.push(s.to_string()),
                Err(_) => println!("Found invalid UTF-8"),
            };
        }
    }
    args
}
