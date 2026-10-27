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

# Keep Python package versions in sync with Rust workspace version

import json
from pathlib import Path
import tomllib
import re
from subprocess import run

base_path = Path(__file__).parent
cargo_ver = tomllib.load(open(base_path / 'Cargo.toml', 'rb'))['workspace']['package']['version']

print(f'Updating Python package versions to {cargo_ver}.')

run(['poetry', 'version', '--project=fastwarc-py', cargo_ver], check=True, cwd=base_path)
run(['poetry', 'version', '--project=resiliparse-py', cargo_ver], check=True, cwd=base_path)

with open(base_path / 'vcpkg.json', 'r') as f:
    vcpkg_json = json.load(f)
with open(base_path / 'vcpkg.json', 'w') as f:
    vcpkg_json['version-string'] = cargo_ver
    json.dump(vcpkg_json, f, ensure_ascii=False, indent=2)
    f.write('\n')

# Patch fastwarc dependency (cannot use `poetry add` for this if fastwarc is not yet published)
with open(base_path / 'resiliparse-py/pyproject.toml', 'r') as f:
    resiliparse_toml = f.read()
with open(base_path / 'resiliparse-py/pyproject.toml', 'w') as f:
    f.write(re.sub(r'"fastwarc==\d+\.\d+\.\d+"',
                   f'"fastwarc=={cargo_ver}"',
                   resiliparse_toml))

run(['cargo', 'generate-lockfile'], cwd=base_path)
