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

//! Verifies that the standard gRPC extras (health service and server
//! reflection) are wired up and answer over a live socket.

use fastwarc_grpc::proto;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Endpoint, Server};

/// Start an in-process server with the same service set as the binary and
/// return its address.
async fn start_full_server() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<WarcServiceServer<WarcParser>>().await;
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();

    tokio::spawn(async move {
        Server::builder()
            .add_service(health_service)
            .add_service(reflection_service)
            .add_service(WarcServiceServer::new(WarcParser))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    addr
}

/// Connect a channel to the test server.
async fn channel(addr: std::net::SocketAddr) -> Channel {
    Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap()
}

#[tokio::test]
async fn health_reports_warc_service_serving() {
    let addr = start_full_server().await;
    let mut client = tonic_health::pb::health_client::HealthClient::new(channel(addr).await);

    let response = client
        .check(tonic_health::pb::HealthCheckRequest {
            service: "fastwarc.v1.WarcService".to_owned(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.status, tonic_health::pb::health_check_response::ServingStatus::Serving as i32);
}

#[tokio::test]
async fn reflection_lists_warc_service() {
    use tonic_reflection::pb::v1::server_reflection_client::ServerReflectionClient;
    use tonic_reflection::pb::v1::{ServerReflectionRequest, server_reflection_request::MessageRequest};

    let addr = start_full_server().await;
    let mut client = ServerReflectionClient::new(channel(addr).await);

    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    let mut stream = client
        .server_reflection_info(tokio_stream::iter(vec![request]))
        .await
        .unwrap()
        .into_inner();
    let response = stream.message().await.unwrap().unwrap();
    let Some(tonic_reflection::pb::v1::server_reflection_response::MessageResponse::ListServicesResponse(services)) =
        response.message_response
    else {
        panic!("expected ListServicesResponse, got {:?}", response.message_response);
    };
    let names: Vec<&str> = services.service.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"fastwarc.v1.WarcService"),
        "fastwarc.v1.WarcService missing from reflection listing: {names:?}"
    );
}
