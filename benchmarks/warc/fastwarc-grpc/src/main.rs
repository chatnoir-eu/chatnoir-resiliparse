use std::io::Read;
use std::time::{Duration, Instant};

use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use hyper_util::rt::TokioIo;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::transport::{Channel, Uri};
use tower::service_fn;

const DEFAULT_BUFFER_SIZE: usize = 64 << 10;
const DEFAULT_PAYLOAD_CHUNK_SIZE: usize = 1024 << 10;
const REQUEST_CHANNEL_BYTES: usize = 32 * 1024 * 1024;

fn buffer_size() -> usize {
    std::env::var("BUFFER_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(DEFAULT_BUFFER_SIZE)
}

fn request_channel_bound(buf_size: usize) -> usize {
    (REQUEST_CHANNEL_BYTES / buf_size.max(1)).clamp(2, 256)
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name).as_deref(), Ok("1") | Ok("true") | Ok("TRUE"))
}

/// Stream every payload byte and header block back over gRPC.
fn full_echo() -> bool {
    env_flag("FASTWARC_GRPC_FULL")
}

/// Remote server URL (`http://host:port`). Unset = in-process Unix socket.
fn server_url() -> Option<String> {
    std::env::var("FASTWARC_GRPC_URL")
        .ok()
        .filter(|value| !value.is_empty())
}

/// Server opens `WARCFILE` on disk; the client does not upload archive bytes.
fn local_file() -> bool {
    env_flag("FASTWARC_GRPC_LOCAL")
}

/// Number of concurrent ParseWarc streams (one connection each).
fn jobs() -> usize {
    std::env::var("FASTWARC_GRPC_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(1)
}

/// Feed `config` and, unless `local`, the archive bytes into the request channel.
fn spawn_feeder(
    path: String,
    tx: tokio::sync::mpsc::Sender<pb::ParseWarcRequest>,
    buf_size: usize,
    full: bool,
    local: bool,
) {
    std::thread::spawn(move || {
        let config = pb::ParseWarcConfig {
            parse_http: Some(false),
            verify_digests: false,
            input_buffer_size: buf_size as u32,
            payload_chunk_size: DEFAULT_PAYLOAD_CHUNK_SIZE as u32,
            include_payload: Some(full),
            include_headers: Some(full),
            response_batch_size: 64,
            archive_path: if local { path.clone() } else { String::new() },
            ..Default::default()
        };
        if tx
            .blocking_send(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Config(config)),
            })
            .is_err()
        {
            return;
        }
        if local {
            return;
        }
        let mut file = std::fs::File::open(&path).expect("File error");
        loop {
            // read_to_end into spare capacity: no per-chunk buffer zeroing.
            let mut buf = Vec::with_capacity(buf_size);
            match Read::take(Read::by_ref(&mut file), buf_size as u64).read_to_end(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let request = pb::ParseWarcRequest {
                        kind: Some(pb::parse_warc_request::Kind::Chunk(buf.into())),
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
}

async fn connect_client(
    remote: Option<&str>,
    sock: &std::path::Path,
) -> Result<WarcServiceClient<Channel>, tonic::transport::Error> {
    let channel = if let Some(url) = remote {
        fastwarc_grpc::transport::connect(url).await?
    } else {
        let sock_for_client = sock.to_path_buf();
        fastwarc_grpc::transport::configure_endpoint(tonic::transport::Endpoint::from_static("http://[::]:50061"))
            .connect_with_connector(service_fn(move |_: Uri| {
                let sock = sock_for_client.clone();
                async move { Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(sock).await?)) }
            }))
            .await?
    };
    Ok(WarcServiceClient::new(channel)
        .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
        .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE))
}

/// Shared record/byte counters across all streams, read by the progress printer.
struct Totals {
    count: std::sync::atomic::AtomicUsize,
    bytes: std::sync::atomic::AtomicU64,
}

/// Drive one ParseWarc stream to completion; returns (records, payload bytes).
async fn run_stream(
    mut client: WarcServiceClient<Channel>,
    rx: tokio::sync::mpsc::Receiver<pb::ParseWarcRequest>,
    totals: std::sync::Arc<Totals>,
) -> Result<(usize, u64), Box<dyn std::error::Error + Send + Sync>> {
    use std::sync::atomic::Ordering::Relaxed;

    let mut stream = client.parse_warc(ReceiverStream::new(rx)).await?.into_inner();
    let mut total_count = 0usize;
    let mut total_bytes = 0u64;
    while let Some(response) = stream.message().await? {
        visit_ends(&response, |end| {
            total_count += 1;
            total_bytes += end.payload_length;
            totals.count.fetch_add(1, Relaxed);
            totals.bytes.fetch_add(end.payload_length, Relaxed);
        });
    }
    Ok((total_count, total_bytes))
}

/// Print aggregate throughput every 500 ms until aborted. With multiple
/// streams the lines cover all of them combined.
async fn print_progress(totals: std::sync::Arc<Totals>) {
    use std::sync::atomic::Ordering::Relaxed;

    let mut last_timer = Instant::now();
    let mut last_count = 0usize;
    let mut last_bytes = 0u64;
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let count = totals.count.load(Relaxed);
        let bytes = totals.bytes.load(Relaxed);
        let delta_count = count - last_count;
        let delta_bytes = bytes - last_bytes;
        if delta_count == 0 {
            continue;
        }
        let elapsed = last_timer.elapsed();
        println!(
            "{:.0} records/s, {:.1} MiB/s, {:.1} KiB/rec ({} total, {:.1} MiB)",
            delta_count as f64 / elapsed.as_secs_f64(),
            delta_bytes as f64 / elapsed.as_secs_f64() / 1024.0 / 1024.0,
            delta_bytes as f64 / delta_count.max(1) as f64 / 1024.0,
            count,
            bytes as f64 / 1024.0 / 1024.0
        );
        last_timer = Instant::now();
        last_count = count;
        last_bytes = bytes;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} WARCFILE", args[0]);
        println!("  parse-only (default): matches fastwarc inplace (no payload/header echo)");
        println!("  FASTWARC_GRPC_FULL=1: stream every payload byte back over gRPC");
        println!("  BUFFER_SIZE=<bytes>: gRPC input chunk size (default 64 KiB)");
        println!("  FASTWARC_GRPC_URL=http://host:port: remote TCP client (no in-process server)");
        println!("  FASTWARC_GRPC_LOCAL=1: server reads WARCFILE from disk; no archive upload");
        println!("  FASTWARC_GRPC_JOBS=N: N concurrent streams; reports aggregate throughput");
        return Ok(());
    }
    let path = args[1].clone();
    let full = full_echo();
    let buf_size = buffer_size();
    let remote = server_url();
    let local = local_file();
    let jobs = jobs();

    let sock = std::env::temp_dir().join(format!("fastwarc-grpc-bench-{}.sock", std::process::id()));
    if remote.is_none() {
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock)?;
        tokio::spawn(
            fastwarc_grpc::transport::configure_server(tonic::transport::Server::builder())
                .add_service(fastwarc_grpc::transport::configure_warc_server(WarcServiceServer::new(
                    WarcParser::with_local_files(),
                )))
                .serve_with_incoming(UnixListenerStream::new(listener)),
        );
    }

    let mode = if full { "full payload echo" } else { "parse-only" };
    let transport = if remote.is_some() { "tcp" } else { "uds" };
    let source = if local {
        "local file".to_owned()
    } else {
        format!("{:.0} KiB chunks", buf_size as f64 / 1024.0)
    };
    println!(
        "Reading WARC file: {} ({mode}, {transport}, {source}, {jobs} job(s), http2 stream {} MiB / conn {} MiB)",
        args[1],
        f64::from(fastwarc_grpc::transport::stream_window()) / 1024.0 / 1024.0,
        f64::from(fastwarc_grpc::transport::connection_window()) / 1024.0 / 1024.0
    );
    if jobs > 1 {
        println!(
            "{jobs} concurrent streams, each parsing the FULL archive ({jobs}x total work); rates below are the aggregate"
        );
    }

    let totals = std::sync::Arc::new(Totals {
        count: std::sync::atomic::AtomicUsize::new(0),
        bytes: std::sync::atomic::AtomicU64::new(0),
    });
    let printer = tokio::spawn(print_progress(totals.clone()));

    let start = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..jobs {
        let (tx, rx) = tokio::sync::mpsc::channel::<pb::ParseWarcRequest>(request_channel_bound(buf_size));
        spawn_feeder(path.clone(), tx, buf_size, full, local);
        let client = connect_client(remote.as_deref(), &sock).await?;
        handles.push(tokio::spawn(run_stream(client, rx, totals.clone())));
    }
    let mut total_count = 0usize;
    let mut total_bytes = 0u64;
    for handle in handles {
        let (count, bytes) = handle.await?.map_err(|e| e.to_string())?;
        total_count += count;
        total_bytes += bytes;
    }
    printer.abort();

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
    if jobs > 1 {
        let per_stream_bytes = total_bytes as f64 / jobs as f64;
        println!(
            "Aggregate over {jobs} streams x {:.1} MiB each; per-stream average {:.1} MiB/s",
            per_stream_bytes / 1024.0 / 1024.0,
            per_stream_bytes / total_elapsed / 1024.0 / 1024.0
        );
    }
    if remote.is_none() {
        let _ = std::fs::remove_file(&sock);
    }
    Ok(())
}

fn visit_ends(resp: &pb::ParseWarcResponse, mut visit: impl FnMut(&pb::RecordEnd)) {
    fn walk(resp: &pb::ParseWarcResponse, visit: &mut impl FnMut(&pb::RecordEnd)) {
        match resp.kind.as_ref() {
            Some(pb::parse_warc_response::Kind::RecordEnd(end)) => visit(end),
            Some(pb::parse_warc_response::Kind::Batch(batch)) => {
                for item in &batch.items {
                    walk(item, visit);
                }
            }
            Some(pb::parse_warc_response::Kind::RecordError(error)) if !error.recoverable => {
                eprintln!("Error: {}", error.message);
            }
            _ => {}
        }
    }
    walk(resp, &mut visit);
}
