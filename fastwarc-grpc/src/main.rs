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
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("FASTWARC_GRPC_ADDR").unwrap_or_else(|_| "[::]:50061".to_owned());
    let allow_local_files =
        matches!(std::env::var("FASTWARC_GRPC_ALLOW_LOCAL_FILES").as_deref(), Ok("1" | "true" | "TRUE"));
    let local_file_root = std::env::var_os("FASTWARC_GRPC_LOCAL_FILE_ROOT").map(PathBuf::from);
    let parser = if allow_local_files {
        let root = local_file_root.as_deref().ok_or(
            "FASTWARC_GRPC_ALLOW_LOCAL_FILES is set but FASTWARC_GRPC_LOCAL_FILE_ROOT is not; \
             set it to the directory that archive_path requests are confined to",
        )?;
        WarcParser::with_local_files(root)
            .map_err(|error| format!("invalid FASTWARC_GRPC_LOCAL_FILE_ROOT {}: {error}", root.display()))?
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
        match local_file_root.as_deref().filter(|_| allow_local_files) {
            Some(root) => format!("confined to {}", root.display()),
            None => "disabled".to_owned(),
        }
    );

    match unix_socket_path(&addr) {
        #[cfg(unix)]
        Some(path) => {
            remove_stale_socket(&path)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let listener = UnixListener::bind(&path)?;
            builder
                .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown_signal())
                .await?;
            remove_stale_socket(&path)?;
        }
        #[cfg(not(unix))]
        Some(path) => {
            return Err(format!(
                "unix domain socket address {} is not supported on this platform; \
                 set FASTWARC_GRPC_ADDR to a TCP address such as [::]:50061",
                path.display()
            )
            .into());
        }
        None => {
            builder.serve_with_shutdown(addr.parse()?, shutdown_signal()).await?;
        }
    }
    Ok(())
}

/// Removes a leftover socket file so the listener can rebind, refusing to
/// delete anything that is not a unix socket.
#[cfg(unix)]
fn remove_stale_socket(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::FileTypeExt;

    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => std::fs::remove_file(path),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists and is not a unix socket", path.display()),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// `unix:///path`, `unix:/path`, or an absolute filesystem path.
fn unix_socket_path(addr: &str) -> Option<PathBuf> {
    if let Some(path) = addr.strip_prefix("unix://").or_else(|| addr.strip_prefix("unix:")) {
        return Some(Path::new(path).to_path_buf());
    }
    let path = Path::new(addr);
    path.is_absolute().then(|| path.to_path_buf())
}

/// Resolves on SIGINT or SIGTERM where available.
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

#[cfg(all(test, unix))]
#[path = "main_test.rs"]
mod main_test;
