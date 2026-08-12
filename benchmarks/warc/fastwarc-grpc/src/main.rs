use std::io::Read;
use std::time::{Duration, Instant};

use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};

const DEFAULT_BUFFER_SIZE: usize = 1024 << 10;

fn buffer_size() -> usize {
    std::env::var("BUFFER_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(DEFAULT_BUFFER_SIZE)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} WARCFILE", args[0]);
        return Ok(());
    }
    let path = args[1].clone();

    // The server runs in-process but is dialed over a real TCP socket, so the
    // numbers include protobuf encoding and HTTP/2 transport on both sides.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(WarcServiceServer::new(WarcParser))
            .serve_with_incoming(TcpListenerStream::new(listener)),
    );

    let (tx, rx) = tokio::sync::mpsc::channel::<pb::ParseWarcRequest>(4);
    std::thread::spawn(move || {
        // Default config: no HTTP parsing and no digest verification,
        // matching the options of the plain fastwarc benchmark.
        let config = pb::ParseWarcConfig::default();
        if tx
            .blocking_send(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Config(config)),
            })
            .is_err()
        {
            return;
        }
        let mut file = std::fs::File::open(&path).expect("File error");
        let mut buf = vec![0u8; buffer_size()];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let request = pb::ParseWarcRequest {
                        kind: Some(pb::parse_warc_request::Kind::Chunk(buf[..n].to_vec())),
                    };
                    if tx.blocking_send(request).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("read error: {}", e);
                    break;
                }
            }
        }
    });

    println!("Reading WARC file: {}", args[1]);
    let mut client = WarcServiceClient::connect(format!("http://{}", addr)).await?;
    let mut stream = client.parse_warc(ReceiverStream::new(rx)).await?.into_inner();

    let start = Instant::now();
    let mut last_timer = start;
    let mut last_count = 0usize;
    let mut last_bytes = 0u64;
    let mut total_count = 0usize;
    let mut total_bytes = 0u64;

    while let Some(response) = stream.message().await? {
        match response.kind {
            Some(pb::parse_warc_response::Kind::RecordEnd(end)) => {
                last_count += 1;
                last_bytes += end.payload_length;
                total_count += 1;
                total_bytes += end.payload_length;

                let elapsed = last_timer.elapsed();
                if elapsed >= Duration::from_millis(500) {
                    println!(
                        "{:.0} records/s, {:.1} MiB/s, {:.1} KiB/rec ({} total, {:.1} MiB)",
                        last_count as f64 / elapsed.as_secs_f64(),
                        last_bytes as f64 / elapsed.as_secs_f64() / 1024.0 / 1024.0,
                        last_bytes as f64 / last_count.max(1) as f64 / 1024.0,
                        total_count,
                        total_bytes as f64 / 1024.0 / 1024.0
                    );
                    last_count = 0;
                    last_bytes = 0;
                    last_timer = Instant::now();
                }
            }
            Some(pb::parse_warc_response::Kind::RecordError(error)) => {
                if !error.recoverable {
                    eprintln!("Error: {}", error.message);
                }
            }
            _ => {}
        }
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    println!(
        "Summary: {:.1}s, {:.0} records/s, {:.1} MiB/s, {:.1} KiB/rec ({} total, {:.1} MiB)",
        total_elapsed,
        total_count as f64 / total_elapsed,
        total_bytes as f64 / total_elapsed / 1024.0 / 1024.0,
        total_bytes as f64 / total_count.max(1) as f64 / 1024.0,
        total_count,
        total_bytes as f64 / 1024.0 / 1024.0
    );
    Ok(())
}
