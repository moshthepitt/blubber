"""Check Rust names used in prose against declared and generated items."""
from collections import defaultdict
from pathlib import Path
import re
import tomllib

IDENT = r'[A-Za-z_][A-Za-z_0-9]*'
ATTR = re.compile(r'#\s*\[(?:"(?:\\.|[^"\\])*"|[^"\]])*\]\s*')
DECLARATION = re.compile(
    rf'^(?:(?:pub|default|async|const)\s+)*(?:fn|type|const|static|struct|enum|trait|union|mod)\s+({IDENT})')
FENCES = re.compile(r'^[ \t]*((`|~)\2{2,})(?!\2)[^\n]*\n(.*?)^[ \t]*\1\2*[ \t]*$', re.M | re.S)
SPAN = re.compile(r'(?<!`)(`+)(?!`)([^\n]*?)(?<!`)\1(?!`)')
SCHEMA = re.compile(rf'#\s*\[\s*(?:blubber::)?schema\b[^\]]*\]\s*(?:pub\s+)?struct\s+({IDENT})[^{{]*{{([^}}]*)}}')
# Rust syntax, application arguments, SDK names and benchmark case labels.
NON_API = {
    'str', 'Vec', 'String', 'Copy', 'FnMut', 'Ok', 'Err', 'Some', 'None',
    'true', 'false', 'unsafe', 'no_std', 'derive', 'inline',
    'account', 'namespace', 'semantic_epoch', 'discriminator', 'RequestHeapFrame',
    'mut_iter', 'mut_try_iter', 'cursor_set',
    'core::str::from_utf8',
    'funding::payer', 'funding::system_program',
    'program_id', 'payer', 'recipient', 'state', 'output_capacity', 'n',
    'credit', 'debit', 'try_', 'cargo', 'LICENSE', 'bytemuck', 'trusted',
    'process_instruction', 'creates_and_pushes_with_exact_lengths_and_rent',
    'AccountView', 'solana_pubkey::Pubkey', 'Rent::get', 'try_minimum_balance', 'is_signer',
    'ProgramError::AccountNotRentExempt',
}
PRIMITIVES = {f'{sign}{width}' for sign in 'iu' for width in (8, 16, 32, 64, 128, 'size')}
PRIMITIVES |= {'bool', 'char', 'f32', 'f64'}


def without_generics(text):
    """Keep an expression's outer path, including the owner of an associated item."""
    depth, output = 0, []
    for token in re.findall(r'->|[^<>]|[<>]', text):
        if token == '<':
            depth += 1
        elif token == '>' and depth:
            depth -= 1
        elif not depth:
            output.append(token)
    return ''.join(output)


def inventory(text):
    names, members = set(), defaultdict(set)
    for line in text.splitlines():
        if line.startswith('#') or not line:
            continue
        columns = line.split('\t', 3)
        if len(columns) == 3 and columns[-1] == 'fields':
            continue
        file, raw_scope, kind, declaration = columns
        crate = Path(file).parts[1].replace('-', '_')
        scope = without_generics(ATTR.sub('', raw_scope)).strip()
        written = ATTR.sub('', declaration).strip()
        names.add(crate)
        owner = None
        if scope.startswith('impl '):
            receiver = scope.removeprefix('impl ').split(' for ')[-1].strip()
            owner = re.split(r'\s*::\s*', receiver)[-1]
        elif scope.startswith('crate::'):
            owner = scope.split('::')[-1]
        elif scope == 'crate':
            source = Path(file)
            owner = source.parent.name if source.name == 'mod.rs' else source.stem
            if owner == 'lib':
                owner = crate
        if match := DECLARATION.match(written):
            symbols = {match[1]}
        elif kind == 'use':
            tree = written.removeprefix('pub ').removeprefix('use ').strip(' ;')
            # The terminal leaves, including aliases, are the exported names.
            symbols = set(re.findall(rf'\b({IDENT})\s*(?=[,}}]|$)', tree))
        elif kind in {'field', 'variant', 'mod'}:
            symbols = set(re.findall(rf'^({IDENT})', written))
        elif kind in {'fields', 'variants'}:
            if kind == 'variants':
                enum = re.search(rf'\benum\s+({IDENT})', scope)
                owner = enum[1] if enum else owner
            # Split top-level slots without picking up payload fields.
            slots = re.split(r'([(){}\[\]<>]|->)', written)
            depth = 0
            flat = []
            for part in slots:
                if part in {'(', '{', '[', '<'}:
                    depth += 1
                elif part in {')', '}', ']', '>'}:
                    depth -= 1
                elif not depth:
                    flat.append(part)
            symbols = set(re.findall(rf'(?:^|,)\s*(?:pub\s+)?({IDENT})\b', ''.join(flat)))
        else:
            continue
        names.update(symbols)
        if owner:
            members[owner].update(symbols)
    return names, members


def schema_items(source, documents):
    """Collect example schema names and the items emitted by the derive templates."""
    names, fields = set(), set()
    examples = [block[3] for text in documents.values() for block in FENCES.finditer(text)]
    for directory in source.workspace_members():
        if directory.parts[0] != 'programs' and directory != Path('tests/doc-examples'):
            continue
        examples.extend(source.read(path.relative_to(source.root))
                        for path in (source.root / directory / 'src').glob('*.rs'))
    for text in examples:
        for schema, body in SCHEMA.findall(text):
            names.add(schema)
            fields.update(re.findall(rf'\b({IDENT})\s*:(?!:)', body))
    derive = Path('crates/blubber-derive/src')
    schema = source.read(derive / 'schema.rs')
    templates = '\n'.join([schema, *(source.read(derive / f'{name}.rs') for name in
                                    ('schema_read', 'generic_schema', 'product', 'account'))])
    suffixes = re.findall(r'format_ident!\("\{\}([A-Z][a-zA-Z]+)", name\.unraw\(\)\)', templates)
    generated_names = {name + suffix for name in names for suffix in suffixes}
    items = set(re.findall(rf'\bpub (?:const )?fn ({IDENT})\s*[<(]', templates))
    creation_helpers = re.search(r'let creation_helpers = \[(.*?)\]\.map', schema, re.S)
    if creation_helpers:
        items.update(re.findall(rf'\("({IDENT})",', creation_helpers[1]))
    items.update(re.findall(rf'\bpub const ({IDENT})\s*:', templates))
    items.update(fields | {field + '_mut' for field in fields} | {'try_' + field for field in fields})
    return names | generated_names, items


def mentions(text):
    def clear(match):
        return '\n' * match[0].count('\n')

    text = re.sub(r'<!--.*?-->', clear, text, flags=re.S)
    text = FENCES.sub(clear, text)
    for span in SPAN.finditer(text):
        body = span[2].strip()
        if body.startswith('#['):
            paths = re.findall(rf'{IDENT}(?:::{IDENT})+', body)
            if match := re.match(rf'#\[\s*({IDENT})(?![\w:])', body):
                paths.append(match[1])
        else:
            body = without_generics(body.replace('::<', '<'))
            match = re.match(rf'^&?({IDENT}(?:::{IDENT})*)(?=$|[!?(){{.]|\s*\{{)', body)
            if not match:
                continue
            head = match[1]
            paths = [head, *re.findall(rf'\.({IDENT})\s*\(', body[match.end():])]
            if body[match.end():].startswith('.'):
                paths = paths[1:]
        for path in paths:
            yield text.count('\n', 0, span.start()) + 1, path


def findings(source):
    documents = {name: source.read(name) for name in source.documents()}
    names, members = inventory(source.read('verification/public-api.txt'))
    schemas, generated = schema_items(source, documents)
    for directory in source.workspace_members():
        if directory.parts[0] != 'crates':
            continue
        manifest = tomllib.loads(source.read(directory / 'Cargo.toml'))
        names.update(manifest.get('features', {}))
    bare = names | schemas | generated | NON_API | PRIMITIVES
    for path, text in documents.items():
        for line, identifier in mentions(text):
            parts = identifier.split('::')
            if len(parts) == 1:
                valid = identifier in bare
            else:
                owner, member = parts[-2:]
                valid = (identifier in NON_API or owner == 'T' and len(parts) == 2 and member in generated or
                         all(prefix in names | schemas for prefix in parts[:-1])
                         and all(right in members[left] for left, right in zip(parts, parts[1:-1]))
                         and (member in members[owner] or owner in schemas and member in generated))
            if not valid:
                yield f'{path}:{line}: unknown Rust item {identifier}'
