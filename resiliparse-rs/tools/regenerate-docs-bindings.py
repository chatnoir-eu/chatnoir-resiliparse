#!/usr/bin/env python3
# Copyright 2026 Janek Bevendorff
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

"""Regenerate bindgen headers for docs.rs builds from vcpkg dependencies."""

import json
from pathlib import Path
from subprocess import check_output

crate_dir = Path(__file__).resolve().parent.parent
package_id = check_output(['cargo', 'pkgid', '-p', 'resiliparse'], cwd=crate_dir, text=True).strip()
messages = check_output(
    ['cargo', 'build', '-p', 'resiliparse', '--target', 'x86_64-unknown-linux-gnu',
     '--message-format=json-render-diagnostics'], cwd=crate_dir, text=True,
)
out_dir = next(
    Path(message['out_dir']) for message in map(json.loads, messages.splitlines())
    if message['reason'] == 'build-script-executed' and message['package_id'] == package_id
)
manifest = json.loads((crate_dir / 'vcpkg.json').read_text())
license_header = (crate_dir / 'build.rs').read_text().split('\n\n', 1)[0] + '\n\n'
destination = crate_dir / 'src/third_party/docs'
destination.mkdir(parents=True, exist_ok=True)
for dependency in manifest['dependencies']:
    if '$bindgen' in dependency:
        name = dependency['name']
        snapshot = destination / f'{name}.rs'
        snapshot.write_text(license_header + (out_dir / f'{name}.rs').read_text())
        print(f'Updated {snapshot}')
