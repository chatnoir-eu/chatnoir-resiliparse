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

//! Tests for the server binary's unix socket housekeeping.

use super::*;

#[tokio::test]
async fn socket_bind_preserves_existing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.sock");
    std::fs::write(&path, b"keep").unwrap();
    assert!(bind_unix_socket(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(bind_unix_socket(&link).is_err());
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    let socket = dir.path().join("stale.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(bind_unix_socket(&socket).is_err());
    assert!(socket.exists());
}

#[tokio::test]
async fn socket_bind_preserves_live_listener() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.sock");
    let original = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(bind_unix_socket(&path).is_err(), "replaced a live socket");
    let _client = std::os::unix::net::UnixStream::connect(&path).unwrap();
    original.set_nonblocking(true).unwrap();
    original.accept().expect("original listener lost its endpoint");
}

#[tokio::test]
async fn socket_shutdown_removes_own_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/server.sock");
    let _listener = bind_unix_socket(&path).unwrap();
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    remove_owned_socket(&path, &metadata).unwrap();
    assert!(!path.exists());
    remove_owned_socket(&path, &metadata).unwrap();
}

#[tokio::test]
async fn socket_shutdown_preserves_replacement_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.sock");
    let _original = bind_unix_socket(&path).unwrap();
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();
    remove_owned_socket(&path, &metadata).unwrap();
    let _client = std::os::unix::net::UnixStream::connect(&path).unwrap();
    replacement.set_nonblocking(true).unwrap();
    replacement.accept().expect("replacement listener lost its endpoint");

    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"keep").unwrap();
    remove_owned_socket(&path, &metadata).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");

    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("missing", &path).unwrap();
    remove_owned_socket(&path, &metadata).unwrap();
    assert!(std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
}
