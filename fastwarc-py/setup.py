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

from pathlib import Path
import sys

from setuptools import setup
from setuptools.command.sdist import sdist
from setuptools_rust import Binding, RustExtension

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
import tomli_w

ROOT_DIR = Path(__file__).parent.absolute()


class resiliparse_sdist(sdist):
    def make_release_tree(self, base_dir, files):
        super().make_release_tree(base_dir, files)
        base_dir = Path(base_dir)

        # Patch Cargo.toml
        pkg_name = base_dir.name.split('-', 1)[0]
        self.inline_workspace_dependencies(ROOT_DIR.parent / 'Cargo.toml', base_dir / pkg_name / 'Cargo.toml')

    @staticmethod
    def inline_workspace_dependencies(workspace_manifest, crate_manifest) -> None:
        workspace = tomllib.loads(workspace_manifest.read_text())
        crate = tomllib.loads(crate_manifest.read_text())

        workspace_deps = workspace.get('workspace', {}).get('dependencies', {})
        workspace_package = workspace.get('workspace', {}).get('package', {})

        for k in crate['package']:
            if isinstance(crate['package'][k], dict) and crate['package'][k].get('workspace') is True:
                crate['package'][k] = workspace_package[k]
        for section in ['dependencies', 'dev-dependencies', 'build-dependencies']:
            deps = crate.get(section, {})
            for name, spec in list(deps.items()):
                if name in ['fastwarc', 'resiliparse']:
                    deps[name] = workspace_package['version']
                if isinstance(spec, dict) and spec.get('workspace') is True:
                    inherited = workspace_deps[name]
                    deps[name] = inherited

        crate['workspace'] = {}

        crate_manifest.write_text(tomli_w.dumps(crate))


def main():
    setup(
        rust_extensions=[
            RustExtension(
                target="fastwarc._fastwarc",
                path="fastwarc/Cargo.toml",
                binding=Binding.PyO3
            )
        ],
        cmdclass=dict(sdist=resiliparse_sdist),
        exclude_package_data={
            '': [] if 'sdist' in sys.argv else ['*.rs', 'Cargo.toml', 'Cargo.lock']
        }
    )


if __name__ == '__main__':
    main()
