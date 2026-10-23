# html2text Benchmarks

Tools for building Resiliparse variants side by side and comparing their HTML text extraction (speed and output)
on the HTML records of real WARC files. This is how DatologyAI develops and evaluates changes to this fork before
shipping a wheel.

- `deps.sh` installs a source tree's native dependencies (lexbor, re2, uchardet, lz4, zlib).
- `build.sh` builds a variant (git ref, working tree, PyPI release or wheel) into its own virtualenv in `.bench/`.
- `bench.py` dumps the HTML records of a WARC, benchmarks one variant on them and compares the results.

## Set up

You need a C++17 compiler, CMake, [uv](https://docs.astral.sh/uv/) and Python 3.11. Dependencies come from
[vcpkg](https://github.com/microsoft/vcpkg), using each source tree's own manifest and overlay ports:

```bash
git clone https://github.com/microsoft/vcpkg ~/vcpkg && ~/vcpkg/bootstrap-vcpkg.sh -disableMetrics
export VCPKG_ROOT=~/vcpkg
```

Upstream release tags are not in this fork, so fetch them from upstream:

```bash
git remote add upstream https://github.com/chatnoir-eu/chatnoir-resiliparse
git fetch upstream --tags
```

If vcpkg cannot download sources (e.g. GitHub archive downloads are blocked), pass `--from-source` to `build.sh`.
It then builds lexbor from the repository and ref in the tree's lexbor port and uchardet with the tree's overlay
patches, and takes re2, lz4 and zlib from the system (`apt install libre2-dev liblz4-dev zlib1g-dev`).

Get a WARC file and extract its HTML records once:

```bash
curl -O https://data.commoncrawl.org/$(curl -s https://data.commoncrawl.org/crawl-data/CC-MAIN-2026-21/warc.paths.gz | zcat | head -1)
benchmarks/html2text/build.sh upstream --pypi 1.0.9
.bench/venvs/upstream/bin/python benchmarks/html2text/bench.py dump CC-MAIN-*.warc.gz .bench/records.bin
```

## Compare variants

```bash
B=benchmarks/html2text
$B/build.sh upstream-src v1.0.9                      # upstream release, built like the others
$B/build.sh ours origin/datology/perf-1.0.9          # this fork
$B/build.sh marin origin/marin/main-legacy           # Marin's fork (resiliparse 0.17.2)

for v in upstream-src ours marin; do
    .bench/venvs/$v/bin/python $B/bench.py run .bench/records.bin .bench/$v.json --limit 8000 --repeat 2
done
python3 $B/bench.py compare .bench/upstream-src.json .bench/ours.json .bench/marin.json
```

`compare` reports, relative to the first result, the CPU time of each stage of the ingest path (encoding detection,
parsing, extraction), the speedup, how many records have a different extracted text, the output size and the number
of empty outputs. `run --config` selects the extraction settings: `main_content` (default), `all_text`,
`minimal_html`, or `simplified_dom` (Marin's `extract_simplified_dom()`, only in Marin's fork).

Compare variants built the same way: release wheels bundle their own dependency versions (e.g. a newer re2), so
compare a source build against `build.sh <name> <tag>` of the baseline rather than against `--pypi`. Timings use
process CPU time, but shared machines are still noisy; use `--repeat` and run the variants back to back.

The `marin/main-legacy` and `marin/feature/rust-extract` branches mirror
[Marin's fork](https://github.com/marin-community/chatnoir-resiliparse) for comparison. Don't push to them: their
workflows publish wheels and tags on every push.

## Development loop

1. Change the code in a working tree and build it: `$B/build.sh dev .` (rebuild after every change).
2. Run the tests with that build: `.bench/venvs/dev/bin/python -m pytest tests/resiliparse/extract tests/resiliparse/parse`.
3. Check speed and output against the baseline: `run` both, then `compare`. Extraction changes meant as pure
   optimizations must show `0` differing records on a few thousand records of at least one full WARC.

## Ship a wheel

Pushing to a `datology/**` branch builds and tests wheels for the platforms DatologyAI uses
(`.github/workflows/datology-wheels.yml`); pushing a `datology-v*` tag also publishes them to CodeArtifact. To try a
build in the ingest pipeline before publishing, install the wheel (a workflow artifact, or
`.bench/venvs/<name>`'s build) into the pipeline's environment with `uv pip install <wheel>`.

## Results

One CC-MAIN-2022-05 WARC (7,943 HTML records), `main_content` config, Python 3.11, 4-core x86-64 container, all
variants built from source with the same dependencies (2026-10):

| variant | detect | parse | extract | total | speedup | differing records |
|---|---|---|---|---|---|---|
| upstream 1.0.9 | 2.01 s | 4.51 s | 8.37 s | 14.89 s | 1.00x | – |
| this fork (1.0.9+datology.1) | 1.64 s | 3.88 s | 2.95 s | 8.48 s | 1.76x | 0 |
| Marin 0.17.2 | 2.02 s | 4.76 s | 13.56 s | 20.35 s | 0.73x | 5,940 |

Marin's fork extracts differently (see the `marin/main-legacy` branch): it never updates the DOM depth it passes to the main
content heuristics, so every depth-dependent rule sees the same depth.
