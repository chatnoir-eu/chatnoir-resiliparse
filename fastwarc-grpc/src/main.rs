// Copyright 2026 Kristian Rickert
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Server binary: binds the gRPC endpoint and serves `WarcService`, the
//! standard health service, and server reflection until SIGINT or SIGTERM.

use std::path::{Path, PathBuf};

use fastwarc_grpc::proto;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("FASTWARC_GRPC_ADDR").unwrap_or_else(|_| "[::]:50051".to_owned());
    // Serving server-side files to remote clients is a deliberate operator
    // decision, so `archive_path` support is off unless explicitly enabled.
    let allow_local_files =
        matches!(std::env::var("FASTWARC_GRPC_ALLOW_LOCAL_FILES").as_deref(), Ok("1" | "true" | "TRUE"));
    let parser = if allow_local_files {
        WarcParser::with_local_files()
    } else {
        WarcParser::new()
    };

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<WarcServiceServer<WarcParser>>().await;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let builder = fastwarc_grpc::transport::configure_server(Server::builder())
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(fastwarc_grpc::transport::configure_warc_server(WarcServiceServer::new(parser)));

    println!(
        "fastwarc-grpc listening on {addr} (http2 stream {} MiB, connection {} MiB, local files {})",
        f64::from(fastwarc_grpc::transport::stream_window()) / 1024.0 / 1024.0,
        f64::from(fastwarc_grpc::transport::connection_window()) / 1024.0 / 1024.0,
        if allow_local_files { "allowed" } else { "disabled" }
    );

    if let Some(path) = unix_socket_path(&addr) {
        let _ = std::fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(&path)?;
        builder
            .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown_signal())
            .await?;
        let _ = std::fs::remove_file(&path);
    } else {
        builder.serve_with_shutdown(addr.parse()?, shutdown_signal()).await?;
    }
    Ok(())
}

/// `unix:///path`, `unix:/path`, or an absolute filesystem path.
fn unix_socket_path(addr: &str) -> Option<PathBuf> {
    if let Some(path) = addr.strip_prefix("unix://").or_else(|| addr.strip_prefix("unix:")) {
        return Some(Path::new(path).to_path_buf());
    }
    let path = Path::new(addr);
    path.is_absolute().then(|| path.to_path_buf())
}

/// Resolves on SIGINT, or on SIGTERM where available (the signal container
/// runtimes send on stop).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result {
                    eprintln!("failed to listen for shutdown signal: {e}");
                }
            }
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("failed to listen for shutdown signal: {e}");
        }
    }
}
