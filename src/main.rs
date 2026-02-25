use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Listen { port: u16 },
    Dial { address: String },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Listen { port } => {
            println!("listening on port {}", port);
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            let peers = Arc::new(Mutex::new(HashMap::<SocketAddr, OwnedWriteHalf>::new()));
            let peer_list = peers.clone();

            tokio::spawn(async move {
                loop {
                    let (socket, addr) = listener.accept().await.unwrap();
                    println!("New connection: {}", addr);

                    let (rd, wr) = socket.into_split();
                    {
                        peer_list.lock().await.insert(addr, wr);
                    }

                    tokio::spawn(async move {
                        let rd = BufReader::new(rd);
                        let mut lines = rd.lines();

                        while let Some(line) = lines.next_line().await.unwrap() {
                            println!("{}: {}", addr, line);
                        }
                    });
                }
            });

            let stdin = BufReader::new(io::stdin());
            let mut lines = stdin.lines();

            while let Some(line) = lines.next_line().await.unwrap() {
                let mut peer_list = peers.lock().await;
                for (_, wr) in peer_list.iter_mut() {
                    wr.write_all(line.as_bytes()).await.unwrap();
                    wr.write_all(b"\n").await.unwrap();
                }
            }
        }
        Command::Dial { address } => {
            println!("dialing {}", address);

            let peer_addr = address.clone();
            let socket = TcpStream::connect(address).await?;
            let (rd, mut wr) = io::split(socket);

            tokio::spawn(async move {
                let rd = BufReader::new(rd);
                let mut incoming = rd.lines();

                while let Some(line) = incoming.next_line().await.unwrap() {
                    println!("{}: {}", peer_addr, line);
                }
            });

            let stdin = BufReader::new(io::stdin());
            let mut lines = stdin.lines();

            while let Some(line) = lines.next_line().await.unwrap() {
                wr.write_all(line.as_bytes()).await.unwrap();
                wr.write_all(b"\n").await.unwrap();
            }
        }
    }

    Ok(())
}
