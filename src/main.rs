// NOTE: See `https://datatracker.ietf.org/doc/html/rfc6455`.
// NOTE: See `https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API/Writing_WebSocket_servers`.

mod handshake;
mod packet;

use handshake::handshake;
use packet::{serialize, Opcode, Packet};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

#[derive(Debug)]
enum Comm {
    Connect(Arc<TcpStream>),
    Handshake(Arc<TcpStream>, String),
    Close(Arc<TcpStream>, u16, Option<String>),
    Echo(Arc<TcpStream>, String),
}

fn client(stream: &Arc<TcpStream>, sender: &Sender<Comm>) {
    println!("{stream:?}");
    sender.send(Comm::Connect(stream.clone())).unwrap();

    stream.set_nodelay(true).unwrap();
    let mut reader = BufReader::new(stream.as_ref());
    let mut lines = Vec::new();
    loop {
        let mut buffer = String::new();
        let _ = reader.read_line(&mut buffer).unwrap();
        if buffer == "\r\n" {
            break;
        }
        assert!(!buffer.is_empty());
        lines.push(buffer);
    }

    {
        let mut tokens = lines[0].split_whitespace();
        let method = tokens.next().unwrap();
        let path = tokens.next().unwrap();
        let version = tokens.next().unwrap();

        assert_eq!(method, "GET");
        assert_eq!(path, "/");
        assert_eq!(version, "HTTP/1.1");
    }

    let accept = {
        let mut headers = HashMap::new();

        for line in &lines[1..] {
            let mut tokens = line.split(':');
            let key = tokens.next().unwrap().trim().to_owned();
            let value = tokens.next().unwrap().trim().to_owned();
            headers.insert(key, value);
        }

        assert_eq!(headers["Connection"].to_lowercase(), "upgrade");
        assert_eq!(headers["Upgrade"].to_lowercase(), "websocket");
        assert_eq!(headers["Sec-WebSocket-Version"], "13");

        handshake(headers["Sec-WebSocket-Key"].clone())
    };

    sender
        .send(Comm::Handshake(stream.clone(), accept))
        .unwrap();

    while let Ok(packet) = packet::read(&mut reader) {
        println!("{packet:?}");
        match packet {
            Packet::Text(text) => sender.send(Comm::Echo(stream.clone(), text)).unwrap(),
            Packet::Close(status_code, text) => sender
                .send(Comm::Close(stream.clone(), status_code, text))
                .unwrap(),
        }
    }
}

fn server(receiver: &Receiver<Comm>) {
    let mut clients = HashMap::new();
    loop {
        let comm = receiver.recv().unwrap();
        println!("{comm:?}");
        match comm {
            Comm::Connect(stream) => {
                let ip = stream.peer_addr().unwrap().ip();
                if clients.contains_key(&ip) {
                    stream.shutdown(Shutdown::Both).unwrap();
                    continue;
                }
                let _ = clients.insert(ip, stream);
            }
            Comm::Handshake(stream, accept) => {
                write!(
                    stream.as_ref(),
                    "HTTP/1.1 101 Switching Protocols\r\n\
                     Upgrade: websocket\r\n\
                     Connection: Upgrade\r\n\
                     Sec-WebSocket-Accept: {accept}\r\n\
                     \r\n",
                )
                .unwrap();
                stream.as_ref().flush().unwrap();
            }
            Comm::Close(stream, status_code, text) => {
                let buffer = text.map_or_else(
                    || {
                        let mut buffer = Vec::with_capacity(2);
                        buffer.extend_from_slice(&status_code.to_be_bytes());
                        buffer
                    },
                    |text| {
                        let mut buffer = Vec::with_capacity(2 + text.len());
                        buffer.extend_from_slice(&status_code.to_be_bytes());
                        buffer.extend_from_slice(text.as_bytes());
                        buffer
                    },
                );
                stream
                    .as_ref()
                    .write_all(&serialize(Opcode::Close, None, &buffer))
                    .unwrap();
                stream.as_ref().flush().unwrap();
                stream.shutdown(Shutdown::Both).unwrap();
                clients.remove(&stream.peer_addr().unwrap().ip());
            }
            Comm::Echo(stream, text) => {
                stream
                    .as_ref()
                    .write_all(&serialize(Opcode::Text, None, text.as_bytes()))
                    .unwrap();
                stream.as_ref().flush().unwrap();
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let (sender, receiver) = channel();
    thread::spawn(move || {
        server(&receiver);
    });
    for stream in listener.incoming() {
        let sender = sender.clone();
        thread::spawn(move || {
            client(&Arc::new(stream.unwrap()), &sender);
        });
    }
}
