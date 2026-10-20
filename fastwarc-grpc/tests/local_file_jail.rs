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

//! Verifies that `archive_path` requests are confined to the local file root
//! configured with `WarcParser::with_local_files`.

mod common;

use std::path::{Path, PathBuf};

use common::{RecordOutcome, data_path, group_responses};
use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::warc_service::WarcParser;
use tonic::transport::Channel;

/// A client whose server is jailed to the directory holding the WARC
/// fixtures.
async fn fixture_jail_client() -> (pb::warc_service_client::WarcServiceClient<Channel>, PathBuf) {
    let root = data_path("warcfile.warc").parent().unwrap().to_path_buf();
    (common::warc_client_with_root(&root).await, root)
}

async fn parse_path(
    client: &mut pb::warc_service_client::WarcServiceClient<Channel>,
    archive_path: String,
) -> Result<Vec<pb::ParseWarcResponse>, tonic::Status> {
    let config = pb::ParseWarcConfig {
        parse_http: Some(false),
        archive_path,
        ..Default::default()
    };
    let requests = tokio_stream::iter(vec![pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(config)),
    }]);
    let mut stream = client.parse_warc(requests).await?.into_inner();
    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await? {
        responses.push(resp);
    }
    Ok(responses)
}

fn count_records(responses: &[pb::ParseWarcResponse]) -> usize {
    group_responses(responses)
        .iter()
        .filter(|outcome| matches!(outcome, RecordOutcome::Record { .. }))
        .count()
}

/// An absolute path inside the jail is served.
#[tokio::test]
async fn absolute_path_inside_root_is_served() {
    let (mut client, _root) = fixture_jail_client().await;
    let responses = parse_path(&mut client, data_path("warcfile.warc").to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(count_records(&responses), 50);
}

/// A relative path is resolved against the jail root and served.
#[tokio::test]
async fn relative_path_resolves_against_root() {
    let (mut client, _root) = fixture_jail_client().await;
    let responses = parse_path(&mut client, "warcfile.warc".to_owned()).await.unwrap();
    assert_eq!(count_records(&responses), 50);
}

/// An absolute path outside the jail is rejected with `PermissionDenied`.
#[tokio::test]
async fn absolute_path_outside_root_is_rejected() {
    let (mut client, _root) = fixture_jail_client().await;
    let status = parse_path(
        &mut client,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("README.md")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    .unwrap_err();
    assert_eq!(status.code(), tonic::Code::PermissionDenied);
}

/// `..` components cannot escape the jail, even when the traversal would
/// land back on an existing file.
#[tokio::test]
async fn parent_traversal_is_rejected() {
    let (mut client, root) = fixture_jail_client().await;
    // Escapes to the sibling fixture directory of the fastwarc crate.
    let escape = format!("{}/../../../fastwarc-rs/tests/fixtures/warcfile.warc.zst", root.display());
    for requested in [escape, "../data/warcfile.warc".to_owned(), "..".to_owned()] {
        let status = parse_path(&mut client, requested.clone()).await.unwrap_err();
        assert_eq!(status.code(), tonic::Code::PermissionDenied, "expected denial for {requested}");
    }
}

/// A symlink inside the jail pointing outside of it is rejected.
#[cfg(unix)]
#[tokio::test]
async fn symlink_escape_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let jail = directory.path();
    let link = jail.join("outside.warc");
    std::os::unix::fs::symlink(data_path("warcfile.warc"), &link).unwrap();

    let mut client = common::warc_client_with_root(&jail).await;
    let status = parse_path(&mut client, link.to_string_lossy().into_owned())
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::PermissionDenied);
}

/// The jail root itself must be an existing directory.
#[test]
fn root_must_be_an_existing_directory() {
    assert!(WarcParser::with_local_files(Path::new("/nonexistent/definitely-not-here")).is_err());
    assert!(WarcParser::with_local_files(data_path("warcfile.warc")).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn dangling_symlink_escape_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let jail = directory.path();
    let link = jail.join("missing.warc");
    std::os::unix::fs::symlink("../outside-missing.warc", &link).unwrap();
    let mut client = common::warc_client_with_root(&jail).await;
    let result = parse_path(&mut client, "missing.warc".to_owned()).await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
}

#[cfg(unix)]
#[tokio::test]
async fn relative_symlink_inside_root_is_served() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(data_path("warcfile.warc"), directory.path().join("archive.warc")).unwrap();
    std::os::unix::fs::symlink("archive.warc", directory.path().join("link.warc")).unwrap();
    let mut client = common::warc_client_with_root(directory.path()).await;
    let responses = parse_path(&mut client, "link.warc".into()).await.unwrap();
    assert_eq!(count_records(&responses), 50);
}

#[cfg(unix)]
#[tokio::test]
async fn replacing_root_path_does_not_redirect_reads() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::copy(data_path("warcfile.warc"), root.join("archive.warc")).unwrap();
    let mut client = common::warc_client_with_root(&root).await;
    std::fs::rename(&root, directory.path().join("original")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("archive.warc"), b"different file").unwrap();
    let responses = parse_path(&mut client, "archive.warc".into()).await.unwrap();
    assert_eq!(count_records(&responses), 50);
}
