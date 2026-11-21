#!/usr/bin/env python3
"""Generate declared public Rust signatures and check their reviewed listing."""
import argparse
import difflib
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
LISTING = 'verification/public-api.txt'
PARSER_INPUTS = ('scripts/api-listing/Cargo.toml', 'scripts/api-listing/Cargo.lock',
                 'scripts/api-listing/src/main.rs')


def parser(arguments, source=None):
    manifest = ROOT / PARSER_INPUTS[0]
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target/t2-tools-api')).resolve()
    result = subprocess.run(['cargo', '+1.97.1', 'run', '--quiet', '--offline', '--locked',
                             '--manifest-path', str(manifest), '--', *arguments], cwd=ROOT,
                            env=dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS=os.environ.get('CARGO_BUILD_JOBS', '8')),
                            capture_output=True, text=True, input=source, timeout=180)
    if result.returncode:
        raise ValueError('Rust parser failed: ' + result.stderr)
    return result.stdout


def tokens(source):
    return parser(['--tokens'], source)


def generate(root=ROOT):
    output = parser([str(root)])
    if not output.strip():
        raise ValueError('Empty public API listing')
    return output


def check(root=ROOT):
    expected = (root / LISTING).read_text()
    actual = generate(root)
    if actual != expected:
        difference = ''.join(difflib.unified_diff(expected.splitlines(True), actual.splitlines(True),
                                                fromfile='reviewed API', tofile='current API'))
        raise ValueError('API source changed; review signatures and regenerate public-api.txt:\n' + difference)
    return len(actual.splitlines()) - 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    if args.write:
        (ROOT / LISTING).write_text(generate())
    else:
        print(f'Public API: {check()} declarations; reviewed listing matches')


if __name__ == '__main__':
    main()
