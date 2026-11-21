"""Resolve documentation figures from source rows."""
import csv
from decimal import Decimal, ROUND_HALF_UP
import io
from pathlib import Path
import re
import subprocess
import tomllib

SEEDS = Path('docs/facts')
ID = r'[a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+'
HUMAN_ROOTS = {'README.md', 'docs', 'crates'}
COMPARE = 'benchmarks/compare/results/all.csv'
VALUE_FORMATS = {
    'quarter': r'Q[1-4] \d{4}',
    'series': r'\d+\.\d+\.x',
    'toolchain': r'nightly-\d{4}-\d{2}-\d{2}',
}


class Source:
    """Read a commit, the working tree, or the working tree with staged results."""
    def __init__(self, root, *, ref=None, staged=None):
        self.root = root
        self.ref = ref
        self.staged = staged
        self.cache = {}

    def read(self, name):
        if name not in self.cache:
            if self.ref:
                self.cache[name] = subprocess.check_output(
                    ['git', 'show', f'{self.ref}:{name}'], cwd=self.root, text=True)
            else:
                override = self.staged / name if self.staged else None
                self.cache[name] = (override if override and override.is_file()
                                    else self.root / name).read_text()
        return self.cache[name]

    def workspace_members(self):
        return [Path(path) for path in tomllib.loads(self.read('Cargo.toml'))['workspace']['members']]

    def documents(self):
        if self.ref:
            names = subprocess.check_output(
                ['git', 'ls-tree', '-r', '--name-only', '-z', self.ref, '--', *sorted(HUMAN_ROOTS)],
                cwd=self.root, text=True).split('\0')
            paths = list(map(Path, filter(None, names)))
        else:
            paths = [Path('README.md'), *(p.relative_to(self.root) for p in
                     (self.root / 'docs').rglob('*.md')), *(p.relative_to(self.root) for p in
                     (self.root / 'crates').glob('*/README.md'))]
        crates = set(self.workspace_members()) if any(p.parts[0] == 'crates' for p in paths) else set()
        return sorted(p.as_posix() for p in paths if p == Path('README.md')
                      or (p.parts[0] == 'docs' and p.suffix == '.md' and not p.is_relative_to(SEEDS))
                      or (p.parts[0] == 'crates' and p.parent in crates and p.name == 'README.md'))


def tsv(text):
    return list(csv.DictReader(io.StringIO(text), delimiter='\t', strict=True))


def csv_rows(text):
    """Keep physical source lines even when a record spans multiple lines."""
    reader = csv.reader(io.StringIO(text, newline=''), strict=True)
    fields = next(reader, [])
    if not fields or len(set(fields)) != len(fields):
        raise ValueError('Missing or duplicate CSV columns')
    while True:
        line = reader.line_num + 1
        row = next(reader, None)
        if row is None:
            break
        if not row:
            continue
        if len(row) != len(fields):
            raise ValueError(f'Malformed CSV row at line {line}')
        yield line, dict(zip(fields, row))


def observed(source, rule, sources):
    path, selector = rule['source'], rule['selector']
    field, _, conversion = rule['field'].partition(':')
    if path.endswith('.csv'):
        conditions = dict(token.split('=', 1) for token in selector.split(','))
        hits = [(line, row[field.removeprefix('?')]) for line, row in sources[path]
                if all(row[key] == value for key, value in conditions.items())]
    else:
        pattern = re.compile(selector, re.M)
        text = source.read(path)
        hits = [(text.count('\n', 0, hit.start(field)) + 1, hit[field])
                for hit in pattern.finditer(text)]
    if field.startswith('?'):
        if not hits:
            return '', f'{path} ({selector})'
        if len(hits) == 1 and not hits[0][1]:
            return '', f'{path}:{hits[0][0]}'
    if len(hits) != 1 or not hits[0][1]:
        raise ValueError(f'{rule["id"]}: expected one populated match in {path} ({selector})')
    line, value = hits[0]
    if conversion == 'utf8_len':
        value = str(len(value.encode()))
    elif conversion == 'kib':
        value = format(Decimal(value) / 1024, 'f')
    elif conversion:
        raise ValueError(f'{rule["id"]}: unknown unit conversion {conversion}')
    return value.replace(',', ''), f'{path}:{line}'


def comparison_counts(rows):
    rows = [row for _, row in rows if row['arch'] == 'v0' and row['n'] == '1000']
    operations = {(row['workload'], row['op']): row for row in rows
                  if row['lib'] == 'blubber_trusted' and row['status'] == 'ok'}
    if not operations:
        raise ValueError('Missing successful lazy comparison rows')
    counts = dict(borsh_wins=0, borsh_completed=0, borsh_heap_exhausted=0,
                  other_wins=0, other_losses=0, other_ties=0, operations=0)
    for key, lazy in operations.items():
        peers = [row for row in rows if (row['workload'], row['op']) == key
                 and not row['lib'].startswith('blubber')]
        whole = int(lazy['whole_cu'])
        for borsh in (row for row in peers if row['lib'] == 'borsh'):
            if borsh['status'] == 'ok':
                counts['borsh_completed'] += 1
                counts['borsh_wins'] += whole < int(borsh['whole_cu'])
            counts['borsh_heap_exhausted'] += borsh['status'] == 'heap_exhausted'
        best = min((int(row['whole_cu']) for row in peers if row['status'] == 'ok'), default=None)
        if best is not None:
            counts['operations'] += 1
            result = 'other_wins' if whole < best else 'other_losses' if whole > best else 'other_ties'
            counts[result] += 1
    return counts


def resolve_numbers(source, rules):
    facts = {}
    tables = {}
    all_counts = None
    for rule in rules:
        key, path = rule['id'], rule.get('source')
        if key in facts or not re.fullmatch(ID, key):
            raise ValueError(f'Invalid or duplicate number ID: {key}')
        if path is None:
            value, where = rule['value'], 'verification/docs-inputs.json'
        elif path == '@calculate':
            left_id, right_id = rule['selector'].split(',')
            left, right = facts[left_id], facts[right_id]
            where = f'{left["location"]}; {right["location"]}'
            value = ''
            if left['value'] and right['value']:
                a, b = Decimal(left['value']), Decimal(right['value'])
                mode, _, precision = rule['field'].partition(':')
                if mode == 'difference':
                    result = a - b
                elif mode == 'difference_rounded':
                    step = Decimal(precision)
                    if step <= 0 or step != step.to_integral_value():
                        raise ValueError(f'{key}: expected a positive integer rounding step')
                    steps = int(((a - b) / step).quantize(Decimal(1), rounding=ROUND_HALF_UP))
                    result = Decimal(steps) * step
                    precision = '0'
                elif mode == 'ratio':
                    result = a / b
                elif mode == 'percent_above':
                    result = 100 * (a - b) / b
                else:
                    raise ValueError(f'Unknown calculation: {mode}')
                value = format(result, f'.{int(precision or "0")}f')
        else:
            if path.endswith('.csv') and path not in tables:
                tables[path] = list(csv_rows(source.read(path)))
            if rule['selector'] == '@v0_n1000_comparison':
                if all_counts is None:
                    all_counts = comparison_counts(tables[path])
                value = str(all_counts[rule['field']])
                where = f'{path}:{tables[path][0][0]}-{tables[path][-1][0]}'
            else:
                value, where = observed(source, rule, tables)
        pattern = VALUE_FORMATS.get(rule.get('unit', ''), r'[+-]?\d+(?:\.\d+)*(?:-[A-Za-z][A-Za-z0-9.]*\d)?')
        if value and not re.fullmatch(pattern, value):
            raise ValueError(f'{key}: expected a numeric value or version, got {value!r}')
        facts[key] = dict(id=key, value=value, unit=rule.get('unit', ''), location=where,
                          label=rule.get('label', ''), status='available' if value else 'missing')
    return facts


