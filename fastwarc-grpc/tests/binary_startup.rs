// Copyright 2026 Janek Bevendorff
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

//! Exercises configuration, service registration, and shutdown through the actual server binary.

use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use fastwarc_grpc::proto::fastwarc::v1 as pb;
use rstest::rstest;
use tempfile::NamedTempFile;
use tonic::transport::{Channel, Endpoint};

const TIMEOUT: Duration = Duration::from_secs(10);

// Own a child and file-backed logs so failed tests cannot leave a running server or blocked pipes.
struct TestServer {
    child: Child,
    stdout: NamedTempFile,
    stderr: NamedTempFile,
}

impl TestServer {
    // Start the binary with independent configuration and captured diagnostics.
    fn spawn(command: &mut Command) -> Self {
        let stdout = NamedTempFile::new().unwrap();
        let stderr = NamedTempFile::new().unwrap();
        let child = command
            .stdin(Stdio::null())
            .stdout(stdout.reopen().unwrap())
            .stderr(stderr.reopen().unwrap())
            .spawn()
            .unwrap();
        Self { child, stdout, stderr }
    }

    // Read the captured standard output.
    fn stdout(&self) -> String {
        std::fs::read_to_string(self.stdout.path()).unwrap()
    }

    // Read the captured error output.
    fn stderr(&self) -> String {
        std::fs::read_to_string(self.stderr.path()).unwrap()
    }

    // Wait for natural or graceful termination, with diagnostics on timeout.
    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "server did not exit: {}", self.stderr());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // Obtain the address bound by the child, rather than reserving a port with a bind/close race.
    fn listening_address(&mut self) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let output = self.stdout();
            if output.ends_with('\n')
                && let Some(address) = output
                    .strip_prefix("fastwarc-grpc listening on ")
                    .and_then(|line| line.split_once(" ("))
                    .map(|(address, _)| address)
            {
                return address.to_owned();
            }
            assert!(self.child.try_wait().unwrap().is_none(), "server exited before listening: {}", self.stderr());
            assert!(Instant::now() < deadline, "server did not announce readiness: {}", self.stderr());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // Signal only our child, then verify graceful termination and profile flushing.
    #[cfg(unix)]
    async fn shutdown(&mut self, signal: &str) {
        assert!(
            Command::new("kill")
                .args([signal, &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        // Keep polling client tasks so they can process the server's graceful HTTP/2 shutdown.
        let status = tokio::time::timeout(TIMEOUT, async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("server did not exit: {}", self.stderr()));
        assert!(status.success(), "server failed during shutdown: {}", self.stderr());
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Reap the process even when an assertion or an RPC times out.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// Avoid inheriting developer/CI settings without mutating the test process's environment.
fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fastwarc-grpc"));
    for name in [
        "FASTWARC_GRPC_ADDR",
        "FASTWARC_GRPC_ALLOW_LOCAL_FILES",
        "FASTWARC_GRPC_LOCAL_FILE_ROOT",
        "FASTWARC_GRPC_MAX_CONCURRENT_PARSERS",
        "FASTWARC_HTTP2_CONNECTION_WINDOW",
        "FASTWARC_HTTP2_STREAM_WINDOW",
    ] {
        command.env_remove(name);
    }
    command
        .env("FASTWARC_GRPC_ADDR", "127.0.0.1:0")
        .env("TOKIO_WORKER_THREADS", "2");
    command
}

// A failed configuration must never be announced as a listening server.
fn assert_startup_failure(command: &mut Command, diagnostic: &str) {
    let mut server = TestServer::spawn(command);
    assert!(!server.wait().success());
    assert!(!server.stdout().contains("listening on"));
    assert!(server.stderr().contains(diagnostic), "missing {diagnostic:?}: {}", server.stderr());
}

#[rstest]
#[case::not_a_number("invalid", "must be a positive integer")]
#[case::negative("-1", "must be a positive integer")]
#[case::zero("0", "invalid concurrent parser limit")]
fn invalid_parser_limits(#[case] value: &str, #[case] diagnostic: &str) {
    assert_startup_failure(command().env("FASTWARC_GRPC_MAX_CONCURRENT_PARSERS", value), diagnostic);
}

#[test]
fn oversized_parser_limit() {
    assert_startup_failure(
        command().env("FASTWARC_GRPC_MAX_CONCURRENT_PARSERS", usize::MAX.to_string()),
        "invalid concurrent parser limit",
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_parser_limit() {
    use std::os::unix::ffi::OsStringExt;
    assert_startup_failure(
        command().env("FASTWARC_GRPC_MAX_CONCURRENT_PARSERS", std::ffi::OsString::from_vec(vec![0xff])),
        "NotUnicode",
    );
}

#[cfg(not(unix))]
#[test]
fn unsupported_unix_socket_address() {
    assert_startup_failure(command().env("FASTWARC_GRPC_ADDR", "unix:/server.sock"), "not supported on this platform");
}

#[test]
fn invalid_tcp_address() {
    assert_startup_failure(command().env("FASTWARC_GRPC_ADDR", "not-an-address"), "Error:");
}

#[test]
fn occupied_tcp_address() {
    // Keep the listener alive so the child cannot steal the supposedly occupied port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    assert_startup_failure(command().env("FASTWARC_GRPC_ADDR", listener.local_addr().unwrap().to_string()), "Error:");
}

#[test]
fn local_files_require_root() {
    assert_startup_failure(
        command().env("FASTWARC_GRPC_ALLOW_LOCAL_FILES", "true"),
        "FASTWARC_GRPC_LOCAL_FILE_ROOT is not",
    );
}

#[rstest]
#[case::missing(false)]
#[case::regular_file(true)]
fn invalid_local_file_root(#[case] create_file: bool) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("not-a-directory");
    if create_file {
        std::fs::write(&root, b"keep").unwrap();
    }
    assert_startup_failure(
        command()
            .env("FASTWARC_GRPC_ALLOW_LOCAL_FILES", "1")
            .env("FASTWARC_GRPC_LOCAL_FILE_ROOT", &root),
        "invalid FASTWARC_GRPC_LOCAL_FILE_ROOT",
    );
    if create_file {
        assert_eq!(std::fs::read(root).unwrap(), b"keep");
    }
}

// Check real RPCs: a listening socket alone does not prove that main registered its services.
async fn assert_services(address: String) -> Channel {
    tokio::time::timeout(TIMEOUT, async {
        let channel = Endpoint::from_shared(address)
            .unwrap()
            .connect_timeout(TIMEOUT)
            .timeout(TIMEOUT)
            .connect()
            .await
            .unwrap();
        let mut health = tonic_health::pb::health_client::HealthClient::new(channel.clone());
        let response = health
            .check(tonic_health::pb::HealthCheckRequest { service: "fastwarc.v1.WarcService".to_owned() })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.status, tonic_health::pb::health_check_response::ServingStatus::Serving as i32);

        use tonic_reflection::pb::v1::{ServerReflectionRequest, server_reflection_request::MessageRequest};
        let mut reflection = tonic_reflection::pb::v1::server_reflection_client::ServerReflectionClient::new(channel.clone());
        let mut responses = reflection
            .server_reflection_info(tokio_stream::iter([ServerReflectionRequest {
                host: String::new(),
                message_request: Some(MessageRequest::ListServices(String::new())),
            }]))
            .await
            .unwrap()
            .into_inner();
        let response = responses.message().await.unwrap().unwrap();
        let Some(tonic_reflection::pb::v1::server_reflection_response::MessageResponse::ListServicesResponse(services)) = response.message_response else {
            panic!("expected service listing");
        };
        assert!(services.service.iter().any(|service| service.name == "fastwarc.v1.WarcService"));
        channel
    }).await.expect("server services did not become ready")
}

#[tokio::test]
async fn tcp_serves_services_with_local_files_disabled() {
    let mut server = TestServer::spawn(&mut command());
    let address = server.listening_address();
    let channel = assert_services(format!("http://{address}")).await;
    assert!(server.stdout().contains("local files disabled"));
    let mut client = pb::warc_service_client::WarcServiceClient::new(channel);
    let request = pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(pb::ParseWarcConfig {
            archive_path: "not-accessible.warc".to_owned(),
            ..Default::default()
        })),
    };
    let error = tokio::time::timeout(TIMEOUT, client.parse_warc(tokio_stream::iter([request])))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    drop(client);
    #[cfg(unix)]
    server.shutdown("-TERM").await;
}

#[cfg(unix)]
#[rstest]
#[case::term_prefixed("unix:", "-TERM")]
#[case::int_uri("unix://", "-INT")]
#[case::term_absolute("", "-TERM")]
#[tokio::test]
async fn unix_serves_and_removes_socket(#[case] prefix: &str, #[case] signal: &str) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nested/server.sock");
    let mut server = TestServer::spawn(
        command()
            .env("FASTWARC_GRPC_ADDR", format!("{prefix}{}", socket.display()))
            .env("FASTWARC_GRPC_ALLOW_LOCAL_FILES", "TRUE")
            .env("FASTWARC_GRPC_LOCAL_FILE_ROOT", dir.path())
            .env("FASTWARC_GRPC_MAX_CONCURRENT_PARSERS", "2"),
    );
    let address = server.listening_address();
    let channel = assert_services(address).await;
    assert!(
        server
            .stdout()
            .contains(&format!("local files confined to {}", dir.path().display()))
    );
    assert!(socket.exists());
    drop(channel);
    // Readiness RPCs ensure the runtime has polled its shutdown future before signalling it.
    server.shutdown(signal).await;
    assert!(!socket.exists());
    assert!(dir.path().is_dir());
}
