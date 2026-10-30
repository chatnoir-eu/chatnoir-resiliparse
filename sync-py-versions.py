#!/usr/bin/env python3
# Copyright 2025 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Sync dependency requirements and vcpkg metadata with the Rust workspace version.
# Maturin obtains Python package versions from Cargo workspace inheritance.

import json
from pathlib import Path
import re
import tomllib
from subprocess import run

base_path = Path(__file__).resolve().parent
cargo_ver = tomllib.load(open(base_path / 'Cargo.toml', 'rb'))['workspace']['package']['version']

print(f'Updating Python package and dependency versions to {cargo_ver}.')


def sync_path_dependency_versions(manifest: Path) -> int:
    content = manifest.read_text()
    pattern = r'(?m)^(\s*[\w.-]+\s*=\s*\{[^{}\n]*\bpath\s*=\s*"[^"]+"[^{}\n]*\})'

    def update_dependency(match: re.Match) -> str:
        dependency = match.group(1)
        version_pattern = r'\bversion\s*=\s*"[^"]+"'
        if re.search(version_pattern, dependency):
            return re.sub(version_pattern, f'version = "{cargo_ver}"', dependency, count=1)
        return re.sub(r'\{\s*', f'{{ version = "{cargo_ver}", ', dependency, count=1)

    updated, replacements = re.subn(pattern, update_dependency, content)
    if updated != content:
        manifest.write_text(updated)
    return replacements


# Workspace members inherit dependencies from the root manifest. Standalone
# benchmark workspaces still need their own path dependency versions updated.
for manifest in base_path.rglob('Cargo.toml'):
    sync_path_dependency_versions(manifest)

# Registry requirements stay exact; Cargo configuration supplies local dev overrides.
local_crates = tomllib.loads((base_path / '.cargo/config.toml').read_text())['patch']['crates-io']
workspace_manifest = base_path / 'Cargo.toml'
workspace_content = workspace_manifest.read_text()
for name in local_crates:
    workspace_content = re.sub(
        rf'(?m)^({re.escape(name)}\s*=\s*)"[^"]+"',
        rf'\g<1>"={cargo_ver}"', workspace_content,
    )
workspace_manifest.write_text(workspace_content)

with open(base_path / 'vcpkg.json', 'r') as f:
    vcpkg_json = json.load(f)
with open(base_path / 'vcpkg.json', 'w') as f:
    vcpkg_json['version-string'] = cargo_ver
    json.dump(vcpkg_json, f, ensure_ascii=False, indent=2)
    f.write('\n')

# Keep the Python FastWARC requirement in sync with the workspace version.
with open(base_path / 'resiliparse-py/pyproject.toml', 'r') as f:
    resiliparse_toml = f.read()
with open(base_path / 'resiliparse-py/pyproject.toml', 'w') as f:
    f.write(re.sub(r'"fastwarc==[^"\s]+"',
                   f'"fastwarc=={cargo_ver}"',
                   resiliparse_toml))

run(['cargo', 'generate-lockfile'], check=True, cwd=base_path)
