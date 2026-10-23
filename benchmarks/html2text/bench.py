"""
Benchmark and compare Resiliparse HTML text extraction on the HTML records of a WARC file.

Run ``dump`` once to extract the HTML records, then ``run`` with each variant's Python (see build.sh) and
``compare`` the results:

    python bench.py dump CC-MAIN-...warc.gz records.bin
    .bench/venvs/upstream/bin/python bench.py run records.bin upstream.json
    .bench/venvs/ours/bin/python bench.py run records.bin ours.json
    python bench.py compare upstream.json ours.json

``run`` times the three stages of the WARC ingest path separately (encoding detection, parsing, extraction)
and records a digest of every extracted text, so ``compare`` can report both speed and output differences.
"""

import argparse
import hashlib
import importlib.metadata
import json
import struct
import time

# Extraction configurations: keyword arguments for extract_plain_text(), or a different extraction function
CONFIGS = {
    'main_content': dict(main_content=True),
    'all_text': dict(main_content=False),
    'minimal_html': dict(main_content=True, preserve_formatting='minimal_html'),
    'simplified_dom': dict(main_content=True, function='extract_simplified_dom'),  # Marin's fork only
}


def dump(warc_path, records_path):
    """Write the HTML response records of a WARC as length-prefixed (URL, payload) pairs."""
    from fastwarc.warc import ArchiveIterator, WarcRecordType

    num_records = 0
    with open(warc_path, 'rb') as warc, open(records_path, 'wb') as out:
        # Same record selection as DatologyAI's WARC ingest job
        for record in ArchiveIterator(warc, record_types=WarcRecordType.response, parse_http=True,
                                      auto_decode='all', max_header_len=1024 * 1024):
            content_type = record.http_headers.get('Content-Type')
            if content_type is None or 'html' not in content_type.lower():
                continue
            url = (record.headers.get('WARC-Target-URI') or '').encode()
            html = record.reader.read()
            out.write(struct.pack('<II', len(url), len(html)))
            out.write(url)
            out.write(html)
            num_records += 1
    print(f'{num_records} HTML records written to {records_path}')


def read_records(records_path, limit=None):
    """Read HTML payloads written by dump()."""
    records = []
    with open(records_path, 'rb') as f:
        while header := f.read(8):
            url_len, html_len = struct.unpack('<II', header)
            f.seek(url_len, 1)
            records.append(f.read(html_len))
            if limit and len(records) >= limit:
                break
    return records


def run(records_path, result_path, config, limit, repeat):
    from resiliparse.extract import html2text
    from resiliparse.parse.encoding import detect_encoding
    from resiliparse.parse.html import HTMLTree

    kwargs = dict(CONFIGS[config])
    extract = getattr(html2text, kwargs.pop('function', 'extract_plain_text'))
    records = [r for r in read_records(records_path, limit) if r.strip()]

    best = None
    for _ in range(repeat):
        seconds = dict(detect=0.0, parse=0.0, extract=0.0)
        texts = []
        for html in records:
            t0 = time.process_time()
            encoding = detect_encoding(html)
            t1 = time.process_time()
            tree = HTMLTree.parse_from_bytes(html, encoding)
            t2 = time.process_time()
            texts.append(extract(tree, **kwargs))
            t3 = time.process_time()
            seconds['detect'] += t1 - t0
            seconds['parse'] += t2 - t1
            seconds['extract'] += t3 - t2
        seconds['total'] = sum(seconds.values())
        if best is None or seconds['total'] < best['total']:
            best = seconds

    result = dict(
        resiliparse=importlib.metadata.version('resiliparse'),
        module=html2text.__file__,
        config=config,
        records=len(records),
        seconds=best,
        output_chars=sum(len(t) for t in texts),
        empty_outputs=sum(not t.strip() for t in texts),
        digests=[hashlib.blake2b(t.encode('utf-8', 'surrogatepass'), digest_size=8).hexdigest() for t in texts])
    with open(result_path, 'w') as f:
        json.dump(result, f)
    print(f'{result_path}: resiliparse {result["resiliparse"]}, {config}, {len(records)} records, '
          f'{best["total"]:.2f}s ({1000 * best["total"] / len(records):.2f} ms/record)')


def compare(result_paths):
    """Print timing and output differences of each result relative to the first one."""
    results = []
    for path in result_paths:
        with open(path) as f:
            results.append((path, json.load(f)))
    base_path, base = results[0]

    print(f'{"result":24s} {"version":>18s} {"config":>14s} {"detect":>7s} {"parse":>7s} {"extract":>8s} '
          f'{"total":>7s} {"ms/rec":>7s} {"speedup":>8s} {"differing":>10s} {"chars":>9s} {"empty":>6s}')
    for path, r in results:
        s = r['seconds']
        if r['records'] != base['records']:
            differing = 'n/a'
        else:
            differing = str(sum(a != b for a, b in zip(base['digests'], r['digests'])))
        print(f'{path:24s} {r["resiliparse"]:>18s} {r["config"]:>14s} {s["detect"]:7.2f} {s["parse"]:7.2f} '
              f'{s["extract"]:8.2f} {s["total"]:7.2f} {1000 * s["total"] / r["records"]:7.2f} '
              f'{base["seconds"]["total"] / s["total"]:7.2f}x {differing:>10s} '
              f'{r["output_chars"] / 1e6:8.2f}M {r["empty_outputs"]:6d}')
    print(f'\nSpeedup and differing outputs are relative to {base_path}.')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)

    p = commands.add_parser('dump', help='extract the HTML records of a WARC file')
    p.add_argument('warc')
    p.add_argument('records')

    p = commands.add_parser('run', help='benchmark the installed Resiliparse on dumped records')
    p.add_argument('records')
    p.add_argument('result')
    p.add_argument('--config', choices=CONFIGS, default='main_content')
    p.add_argument('--limit', type=int, help='use only the first LIMIT records')
    p.add_argument('--repeat', type=int, default=1, help='repeat and keep the fastest run')

    p = commands.add_parser('compare', help='compare run results to the first one')
    p.add_argument('results', nargs='+')

    args = parser.parse_args()
    if args.command == 'dump':
        dump(args.warc, args.records)
    elif args.command == 'run':
        run(args.records, args.result, args.config, args.limit, args.repeat)
    else:
        compare(args.results)


if __name__ == '__main__':
    main()
