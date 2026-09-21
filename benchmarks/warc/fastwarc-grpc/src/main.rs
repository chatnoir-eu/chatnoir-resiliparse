use std::io::{self, Read};
use std::time::{Duration, Instant};

use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use hyper_util::rt::TokioIo;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::transport::Uri;
use tower::service_fn;

const DEFAULT_BUFFER_SIZE: usize = 1024 << 10;

fn buffer_size() -> usize {
    std::env::var("BUFFER_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(DEFAULT_BUFFER_SIZE)
}

/// Uploads the archive once, stopping if the server closes the request stream.
fn feed_archive(
    mut file: std::fs::File,
    tx: tokio::sync::mpsc::Sender<pb::ParseWarcRequest>,
    buf_size: usize,
) -> io::Result<()> {
    loop {
        let mut buf = Vec::with_capacity(buf_size);
        if file.by_ref().take(buf_size as u64).read_to_end(&mut buf)? == 0 {
            return Ok(());
        }
        tx.blocking_send(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Chunk(buf.into())),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "server stopped accepting archive bytes"))?;
    }
}

/// Counts completed records, skipping recoverable errors and failing on fatal ones.
fn count_records(response: pb::ParseWarcResponse, count: &mut usize, bytes: &mut u64) -> io::Result<()> {
    match response.kind {
        Some(pb::parse_warc_response::Kind::RecordEnd(end)) => {
            *count += 1;
            *bytes += end.payload_length;
        }
        Some(pb::parse_warc_response::Kind::Batch(batch)) => {
            for item in batch.items {
                count_records(item, count, bytes)?;
            }
        }
        Some(pb::parse_warc_response::Kind::RecordError(error)) if !error.recoverable => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, error.message));
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} WARCFILE", args[0]);
        println!("  One uploaded archive over a Unix socket; no HTTP parsing, payload or header echo.");
        println!("  BUFFER_SIZE=<bytes>: input chunk size (default 1 MiB, matching fastwarc's benchmark)");
        return Ok(());
    }
    let buf_size = buffer_size();
    if buf_size > fastwarc_grpc::transport::MAX_MESSAGE_SIZE / 2 {
        return Err("BUFFER_SIZE must not exceed 8 MiB".into());
    }

    let directory = tempfile::tempdir()?;
    let sock = directory.path().join("warc.sock");
    let listener = UnixListener::bind(&sock)?;
    let server = tokio::spawn(
        fastwarc_grpc::transport::configure_server(tonic::transport::Server::builder())
            .add_service(fastwarc_grpc::transport::configure_warc_server(WarcServiceServer::new(WarcParser::new())))
            .serve_with_incoming(UnixListenerStream::new(listener)),
    );
    let channel =
        fastwarc_grpc::transport::configure_endpoint(tonic::transport::Endpoint::from_static("http://[::]:50061"))
            .connect_with_connector(service_fn(move |_: Uri| {
                let sock = sock.clone();
                async move { Ok::<_, io::Error>(TokioIo::new(UnixStream::connect(sock).await?)) }
            }))
            .await?;
    let mut client = WarcServiceClient::new(channel)
        .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
        .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE);

    println!("Reading WARC file: {} (one stream, upload over UDS, {} byte chunks, parse-only)", args[1], buf_size);
    let start = Instant::now();
    let file = std::fs::File::open(&args[1])?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(pb::ParseWarcConfig {
            parse_http: Some(false),
            include_payload: Some(false),
            include_headers: Some(false),
            response_batch_size: 64,
            ..Default::default()
        })),
    })
    .await?;
    let feeder = std::thread::spawn(move || feed_archive(file, tx, buf_size));
    let mut stream = client.parse_warc(ReceiverStream::new(rx)).await?.into_inner();
    let mut last_timer = start;
    let mut last_count = 0usize;
    let mut last_bytes = 0u64;
    let mut total_count = 0usize;
    let mut total_bytes = 0u64;
    while let Some(response) = stream.message().await? {
        count_records(response, &mut total_count, &mut total_bytes)?;
        let elapsed = last_timer.elapsed();
        if elapsed >= Duration::from_millis(500) {
            let count = total_count - last_count;
            let bytes = total_bytes - last_bytes;
            println!(
                "{:.0} records/s, {:.1} MiB/s, {:.1} KiB/rec ({} total, {:.1} MiB)",
                count as f64 / elapsed.as_secs_f64(),
                bytes as f64 / elapsed.as_secs_f64() / 1024.0 / 1024.0,
                bytes as f64 / count.max(1) as f64 / 1024.0,
                total_count,
                total_bytes as f64 / 1024.0 / 1024.0
            );
            last_timer = Instant::now();
            last_count = total_count;
            last_bytes = total_bytes;
        }
    }
    feeder.join().map_err(|_| "archive feeder panicked")??;
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
    server.abort();
    Ok(())
}

#[cfg(test)]
#[path = "main_test.rs"]
mod main_test;
