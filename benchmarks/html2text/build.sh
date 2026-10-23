#!/usr/bin/env bash
#
# Build one Resiliparse variant into its own virtualenv under $BENCH_DIR (default: .bench in the repository root).
#
# Usage:
#   build.sh <name> <git-ref or directory> [--from-source]   # build a branch, tag, commit or working tree
#   build.sh <name> --pypi <version>                          # install a published release
#   build.sh <name> --wheel <file>                            # install a wheel, e.g. a CI artifact
#
# Examples:
#   build.sh upstream --pypi 1.0.9
#   build.sh ours origin/datology/perf-1.0.9
#   build.sh marin marin/main-legacy
#   build.sh dev .                       # the current working tree, rebuilt after every change
#
# Each source build gets its native dependencies from deps.sh (pass --from-source if vcpkg can't download sources).
# Set PYTHON to choose the interpreter (default: python3.11, the version DatologyAI's pipelines use).

set -euo pipefail

HERE=$(dirname "$(realpath "$0")")
REPO=$(git -C "$HERE" rev-parse --show-toplevel)
BENCH_DIR=${BENCH_DIR:-$REPO/.bench}
PYTHON=${PYTHON:-python3.11}
NAME=$1
VENV="$BENCH_DIR/venvs/$NAME"

rm -rf "$VENV"
uv venv -q -p "$PYTHON" "$VENV"
pip_install() { uv pip install -q -p "$VENV/bin/python" "$@"; }
# The benchmark reads WARCs with FastWARC, which every variant can share
pip_install fastwarc==1.0.9

if [ "$2" = --pypi ]; then
    pip_install "resiliparse==$3"
    echo "$NAME: resiliparse $3 from PyPI"
    exit 0
fi
if [ "$2" = --wheel ]; then
    pip_install "$3"
    echo "$NAME: $(basename "$3")"
    exit 0
fi

# Source tree: a directory as-is, or a git ref checked out into its own worktree
if [ -d "$2" ]; then
    SRC=$(realpath "$2")
else
    SRC="$BENCH_DIR/src/$NAME"
    git -C "$REPO" worktree remove --force "$SRC" 2>/dev/null || rm -rf "$SRC"
    git -C "$REPO" worktree add -q --detach "$SRC" "$2"
fi
DEPS="$BENCH_DIR/deps/$NAME"
"$HERE/deps.sh" "$SRC" "$DEPS" ${3:-}

# The Python package lives in resiliparse-py/ (1.x) or resiliparse/ (0.x)
PKG="$SRC/resiliparse-py"
[ -f "$PKG/setup.py" ] || PKG="$SRC/resiliparse"
LIB_DIR=$(echo "$DEPS"/*/lib)
pip_install setuptools 'cython<3.2'
# 1.x setup.py reads RESILIPARSE_VCPKG_PATH; 0.x setup.py relies on the compiler's search paths
RESILIPARSE_VCPKG_PATH="$DEPS" CPATH="$(dirname "$LIB_DIR")/include" LIBRARY_PATH="$LIB_DIR" \
    LDFLAGS="-Wl,-rpath,$LIB_DIR" \
    uv pip install -q -p "$VENV/bin/python" --no-build-isolation --no-deps --reinstall "$PKG"
echo "$NAME: built $(git -C "$SRC" describe --always --dirty 2>/dev/null || echo "$SRC") into $VENV"
