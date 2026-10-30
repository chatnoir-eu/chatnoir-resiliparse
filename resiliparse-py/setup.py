# Copyright 2023 Janek Bevendorff
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

import os
import sys
from pathlib import Path

from setuptools import setup
from setuptools.command.sdist import sdist
from setuptools_rust import Binding, RustExtension
import tomli_w

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

DEBUG = bool(int(os.getenv('DEBUG', 0)))

ROOT_DIR = Path(__file__).parent.absolute()


class resiliparse_sdist(sdist):
    def make_release_tree(self, base_dir, files):
        super().make_release_tree(base_dir, files)
        base_dir = Path(base_dir)
        crate_name = base_dir.name.split('-', 1)[0]
        workspace_manifest = ROOT_DIR.parent / 'Cargo.toml'
        crate_manifest = base_dir / crate_name / 'Cargo.toml'
        self.inline_workspace_dependencies(workspace_manifest, crate_manifest)

    @staticmethod
    def inline_workspace_dependencies(workspace_manifest, crate_manifest) -> None:
        workspace = tomllib.loads(workspace_manifest.read_text())
        crate = tomllib.loads(crate_manifest.read_text())
        workspace_deps = workspace.get('workspace', {}).get('dependencies', {})
        workspace_package = workspace.get('workspace', {}).get('package', {})

        for name, value in crate['package'].items():
            if isinstance(value, dict) and value.get('workspace') is True:
                crate['package'][name] = workspace_package[name]
        for section in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            dependencies = crate.get(section, {})
            for name, spec in list(dependencies.items()):
                if name in ('fastwarc', 'resiliparse'):
                    dependencies[name] = workspace_package['version']
                elif isinstance(spec, dict) and spec.get('workspace') is True:
                    dependencies[name] = workspace_deps[name]

        crate['workspace'] = {}
        crate_manifest.write_text(tomli_w.dumps(crate))

def main():
    setup(
        rust_extensions=[
            RustExtension(
                target='resiliparse._resiliparse',
                path='resiliparse/Cargo.toml',
                binding=Binding.PyO3,
                debug=DEBUG,
            )
        ],
        cmdclass=dict(sdist=resiliparse_sdist),
        exclude_package_data={
            '': [] if 'sdist' in sys.argv else ['*.h', '*.cpp']
        }
    )


if __name__ == '__main__':
    main()
