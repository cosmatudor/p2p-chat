use clap::{Parser, Subcommand};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

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

            loop {
                let (socket, addr) = listener.accept().await.unwrap();
                println!("New connection: {}", addr);

                let (rd, mut wr) = io::split(socket);

                // read from socket, print to terminal
                tokio::spawn(async move {
                    let rd = BufReader::new(rd);
                    let mut lines = rd.lines();

                    while let Some(line) = lines.next_line().await.unwrap() {
                        println!("{}: {}", addr, line);
                    }
                });

                // read from stdin, send over socket
                let stdin = BufReader::new(io::stdin());
                let mut lines = stdin.lines();

                while let Some(line) = lines.next_line().await.unwrap() {
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

            // read from socket, print to terminal
            tokio::spawn(async move {
                let rd = BufReader::new(rd);
                let mut incoming = rd.lines();

                while let Some(line) = incoming.next_line().await.unwrap() {
                    println!("{}: {}", peer_addr, line);
                }
            });

            // read from stdin, send over socket
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
