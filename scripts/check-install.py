#!/usr/bin/env python3
"""Build and run the README's host example with its documented features."""
import argparse
from pathlib import Path
import re
import runpy
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
CONTENT = runpy.run_path(str(ROOT / 'scripts/docs-content.py'))
EXCERPTS = runpy.run_path(str(ROOT / 'scripts/docs-excerpts.py'))


def read_examples(root):
    readme = (root / 'README.md').read_text()
    commands = [span['body'] for span in CONTENT['INLINE_NUMBERS'].finditer(readme)
                if re.match(r'cargo +add\b', span['body'])]
    if len(commands) != 1:
        raise ValueError('README needs one inline cargo add command')
    regions = EXCERPTS['regions']((root / 'crates/blubber/src/lib.rs').read_text(),
                                  'crates/blubber/src/lib.rs')
    examples = regions.get('host-round-trip', [])
    if len(examples) != 1 or not examples[0][1]:
        raise ValueError('The host-round-trip excerpt must appear in a rustdoc test')
    example = f'fn main() -> blubber::Result<()> {{\n{examples[0][0]}\nOk(())\n}}\n'
    return shlex.split(commands[0]), example


def install_arguments(root, command):
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']
    crate = tomllib.loads((root / 'crates/blubber/Cargo.toml').read_text())
    version = crate['package']['version']
    if version == {'workspace': True}:
        version = workspace['version']
    if command[:2] != ['cargo', 'add'] or len(command) < 3 or command[2] != f'blubber@{version}':
        raise ValueError(f'README install command must use cargo add blubber@{version}')
    options = argparse.ArgumentParser(add_help=False, allow_abbrev=False, exit_on_error=False)
    options.add_argument('-F', '--features', action='append', default=[])
    defaults = options.add_mutually_exclusive_group()
    defaults.add_argument('--no-default-features', action='store_true')
    defaults.add_argument('--default-features', action='store_true')
    try:
        parsed, extra = options.parse_known_args(command[3:])
    except argparse.ArgumentError as error:
        raise ValueError(f'README install flag: {error}') from error
    if extra:
        raise ValueError('Unsupported README install flags: ' + ' '.join(extra))
    features = {name for argument in parsed.features for name in re.split(r'[,\s]+', argument.strip())}
    if unknown := features - crate['features'].keys():
        raise ValueError('Unknown README install features: ' + ', '.join(sorted(unknown)))
    return command[3:]


def check(root, scratch):
    command, example = read_examples(root)
    flags = install_arguments(root, command)

    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='readme-install-', dir=scratch) as directory:
        project = Path(directory)
        (project / 'Cargo.toml').write_text(
            '[package]\nname = "blubber-readme-install"\nversion = "0.0.0"\nedition = "2021"\n\n[workspace]\n')
        shutil.copyfile(root / 'Cargo.lock', project / 'Cargo.lock')
        (project / 'src').mkdir()
        (project / 'src/main.rs').write_text(example)
        subprocess.run(['cargo', '+1.97.1', 'add', '--path', str(root / 'crates/blubber'), *flags],
                       cwd=project, check=True)
        subprocess.run(['cargo', '+1.97.1', 'run', '--quiet', '--locked'], cwd=project, check=True)
    print('README install and host example passed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    try:
        check(ROOT, args.scratch.resolve())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
