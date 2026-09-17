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

use super::*;

#[test]
fn stale_socket_cleanup_preserves_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.sock");
    remove_stale_socket(&path).unwrap();
    std::fs::write(&path, b"keep").unwrap();
    assert_eq!(remove_stale_socket(&path).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert_eq!(remove_stale_socket(&link).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    let socket = dir.path().join("stale.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    remove_stale_socket(&socket).unwrap();
    assert!(!socket.exists());
}
