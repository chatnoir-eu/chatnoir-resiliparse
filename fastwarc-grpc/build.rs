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

//! Build script: compiles the buf-managed protobuf contracts in `proto`
//! into Rust server and client stubs with tonic-prost-build. Clients are
//! generated as well so integration tests can exercise the server in-process.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "proto";
    let protos = ["proto/fastwarc/v1/warc.proto", "proto/fastwarc/v1/warc_service.proto"];
    for proto in &protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("descriptor.bin"))
        .compile_protos(&protos, &[proto_root])?;
    Ok(())
}
