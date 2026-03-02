use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Start {
        #[arg(long)]
        port: u16,
        #[arg(long)]
        peer: Option<String>,
    },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let peers = Arc::new(Mutex::new(HashMap::<SocketAddr, OwnedWriteHalf>::new()));

    match cli.command {
        Command::Start { port, peer } => {
            // --- LISTENING FOR NEW PEERS ---
            println!("listening on port {}", port);
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            let peer_list = peers.clone();

            tokio::spawn(async move {
                loop {
                    let (socket, addr) = listener.accept().await.unwrap();
                    println!("New connection: {}", addr);

                    let (rd, mut wr) = socket.into_split();

                    // send peer list before adding new peer
                    let handshake_msg = compute_handshake_msg(&peer_list).await;
                    wr.write_all(handshake_msg.as_bytes()).await.unwrap();

                    // read hello to get their real listening port
                    let mut reader = BufReader::new(rd);
                    let mut hello = String::new();
                    reader.read_line(&mut hello).await.unwrap();

                    let real_addr: SocketAddr = if hello.starts_with("[hello]:") {
                        let listen_port: u16 = hello["[hello]:".len()..].trim().parse().unwrap();
                        format!("{}:{}", addr.ip(), listen_port).parse().unwrap()
                    } else {
                        addr
                    };

                    peer_list.lock().await.insert(real_addr, wr);

                    tokio::spawn(async move {
                        let mut lines = reader.lines();

                        while let Some(line) = lines.next_line().await.unwrap() {
                            println!("{}: {}", real_addr, line);
                        }
                    });
                }
            });

            let peer_list = peers.clone();

            // --- DIALING TO NEW PEERS ---
            if let Some(address) = peer {
                let peer_addr = address.clone();
                let Ok((rd, mut wr)) = connect_to_peer(peer_addr.to_string()).await else {
                    eprintln!("failed to connect to peer {}", address);
                    return Ok(());
                };

                // send our listening port
                wr.write_all(format!("[hello]:{}\n", port).as_bytes()).await.unwrap();

                // read peer list synchronously before starting stdin
                let mut reader = BufReader::new(rd);
                let mut peer_list_msg = String::new();
                reader.read_line(&mut peer_list_msg).await.unwrap();

                // insert the peer we dialed
                peer_list.lock().await.insert(peer_addr.parse().unwrap(), wr);

                // connect to all peers we learned about
                let content = peer_list_msg.trim_start_matches("[peer_list]:").trim().to_string();
                if !content.is_empty() {
                    for addr in content.split(',') {
                        let addr = addr.to_string();
                        let Ok((rd, mut wr)) = connect_to_peer(addr.clone()).await else {
                            eprintln!("failed to connect to peer {}", addr);
                            continue;
                        };

                        // send hello so the peer knows our real port
                        wr.write_all(format!("[hello]:{}\n", port).as_bytes()).await.unwrap();

                        // consume their peer list response (we already know all peers)
                        let mut discovered_reader = BufReader::new(rd);
                        let mut _discard = String::new();
                        discovered_reader.read_line(&mut _discard).await.unwrap();

                        peer_list.lock().await.insert(addr.parse().unwrap(), wr);

                        // spawn reader for ongoing messages
                        let addr_clone = addr.clone();
                        tokio::spawn(async move {
                            let mut lines = discovered_reader.lines();
                            while let Some(line) = lines.next_line().await.unwrap() {
                                println!("{}: {}", addr_clone, line);
                            }
                        });
                    }
                }

                // spawn reader for ongoing messages
                tokio::spawn(async move {
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap() {
                        println!("{}: {}", peer_addr, line);
                    }
                });
            }

            // --- READ MESSAGES FROM TERMINAL AND SEND TO PEERS ---
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
    }

    Ok(())
}

async fn compute_handshake_msg(peers: &Mutex<HashMap<SocketAddr, OwnedWriteHalf>>) -> String {
    let peer_list = peers.lock().await;
    let addrs: Vec<String> = peer_list.keys().map(|a| a.to_string()).collect();
    format!("[peer_list]:{}\n", addrs.join(","))
}

async fn connect_to_peer(address: String) -> io::Result<(OwnedReadHalf, OwnedWriteHalf)> {
    println!("dialing {}", address);
    let socket = TcpStream::connect(address).await?;
    Ok(socket.into_split())
}
