use std::{error::Error, time::Duration};

use clap::{Parser, Subcommand};

use futures::stream::StreamExt;
use libp2p::{
    Swarm, gossipsub, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use tokio::{io, io::AsyncBufReadExt, select};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
}

#[derive(Parser)]
#[command(name = "p2p-chat", version, about = "Chat")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(long)]
        port: u16,
        #[arg(long)]
        peer: Option<String>,
    },
}

fn build_swarm() -> Result<Swarm<MyBehaviour>, Box<dyn std::error::Error>> {
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .build()
                .map_err(io::Error::other)?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            Ok(MyBehaviour { gossipsub })
        })?
        .build();

    Ok(swarm)
}

async fn run(port: u16, dial_addr: Option<libp2p::Multiaddr>) -> Result<(), Box<dyn Error>> {
    let mut swarm = build_swarm()?;

    let topic = gossipsub::IdentTopic::new("test");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{port}").parse()?)?;
    if let Some(addr) = dial_addr {
        swarm.dial(addr)?;
    }

    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.as_bytes()) {
                    println!("Publish error: {e:?}");
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => println!(
                        "Got message: '{}' with id: {id} from peer: {peer_id}",
                        String::from_utf8_lossy(&message.data),
                    ),
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connected to peer: {peer_id}");
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                _ => {}
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Start { port, peer } => {
            let dial_addr =
                peer.map(|s| s.parse::<libp2p::Multiaddr>().expect("invalid multiaddr"));

            run(port, dial_addr).await?;
        }
    }

    Ok(())
}
