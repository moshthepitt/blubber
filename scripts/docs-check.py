#!/usr/bin/env python3
"""Check rendered documentation against the examples and data shipped with it."""
import json
import os
from pathlib import Path
import re
import runpy
import sys

ROOT = Path(__file__).resolve().parents[1]
INPUTS = Path('verification/docs-inputs.json')
CONTENT = runpy.run_path(str(ROOT / 'scripts/docs-content.py'))
NUMBERS = runpy.run_path(str(ROOT / 'scripts/docs-numbers.py'))
API = runpy.run_path(str(ROOT / 'scripts/docs-api.py'))


def rust_fences(text, name):
    """Enumerate every Rust fence; ignored examples require an adjacent reason."""
    fences = re.compile(r'^([`~]{3,})([^\n]*)\n(.*?)^\1[ \t]*$', re.M | re.S)
    blocks = []
    for match in fences.finditer(text):
        info = match[2].strip()
        tags = [tag.strip() for tag in info.split(',')]
        if tags[0] not in ('rust', 'rs', '', 'no_run', 'compile_fail', 'ignore', 'should_panic'):
            continue
        line = text[:match.start()].count('\n') + 1
        if tags[0] != 'rust':
            raise ValueError(f'{name}:{line}: spell Rust fences rust explicitly')
        if 'ignore' in tags:
            reason = re.search(r'<!-- doctest-ignore: (\S[^\n]+) -->\s*$', text[:match.start()])
            if not reason:
                raise ValueError(f'{name}:{line}: ignored Rust fence needs an adjacent doctest-ignore reason')
        elif any(tag.startswith('ignore') for tag in tags):
            raise ValueError(f'{name}:{line}: use ignore with a doctest-ignore reason')
        blocks.append((line, tags, match[3]))
    return blocks


def markdown_paths(root):
    paths = []
    for directory, dirs, names in os.walk(root):
        dirs[:] = [name for name in dirs
                   if name not in {'.git', '.tmp', 'target', '__pycache__'}]
        paths.extend(path.relative_to(root) for name in names
                     if (path := Path(directory) / name).suffix == '.md'
                     and not path.relative_to(root).is_relative_to('docs/facts'))
    return sorted(paths)


def rustdoc_paths(root):
    return sorted(path.relative_to(root) for path in (root / 'crates').glob('*/src/**/*.rs'))


def check_api_mentions(source):
    if errors := list(API['findings'](source)):
        raise ValueError('\n'.join(errors))


def check_public(root=ROOT):
    source = NUMBERS['Source'](root)
    bindings = json.loads(source.read(INPUTS))
    facts = NUMBERS['resolve_numbers'](source, bindings['numbers'])
    excerpts = CONTENT['code_excerpts'](source, bindings['excerpts'])
    scaffold = runpy.run_path(str(root / '.github/check-links.py'))['rustdoc_scaffolding']
    fixtures = source.read('tests/doc-examples/build.rs')
    for path in markdown_paths(root):
        text = source.read(path)
        CONTENT['marked_code'](text, path, excerpts)
        CONTENT['marked_numbers'](text, path, facts)
        if errors := list(scaffold(text, path)):
            raise ValueError('\n'.join(errors))
        if path == Path('README.md') or path.parts[0] == 'docs':
            if rust_fences(text, path) and f'"../../{path}"' not in fixtures:
                raise ValueError(f'{path}: Rust fences are missing from repository doctest inputs')
    for path in rustdoc_paths(root):
        CONTENT['rustdoc_numbers'](source.read(path), path, facts)
    check_api_mentions(source)
    print(f'Documentation markers and Rust fences passed ({len(excerpts)} excerpts)')


if __name__ == '__main__':
    try:
        check_public()
    except (KeyError, OSError, ValueError) as error:
        sys.exit(str(error))
