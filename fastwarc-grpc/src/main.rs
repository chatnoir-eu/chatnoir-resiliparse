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

use fastwarc_grpc::proto;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("FASTWARC_GRPC_ADDR")
        .unwrap_or_else(|_| "[::]:50051".to_owned())
        .parse()?;

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<WarcServiceServer<WarcParser>>().await;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    println!("fastwarc-grpc listening on {addr}");
    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(WarcServiceServer::new(WarcParser))
        .serve_with_shutdown(addr, shutdown_signal())
        .await?;
    Ok(())
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
