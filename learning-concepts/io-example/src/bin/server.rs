use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6142").await.unwrap();

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        println!("New connection: {}", addr);

        tokio::spawn(async move {
            let mut buf = vec![0; 128];

            loop {
                let n = socket.read(&mut buf).await.unwrap();

                if n == 0 {
                    break;
                }

                println!("GOT {:?}", &buf[..n]);

                socket.write_all(&buf[..n]).await.unwrap();
            }

        });
    }
}