#!/usr/bin/env python3
"""Render and check number markers and report internal terminology."""
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import runpy

ID = r'[a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+'
DIGITS = r'[+-]?(?:[0-9]{1,3}(?:[, _][0-9]{3})+|[0-9]+)(?:\.[0-9]+)*'
DISPLAY = rf'(?:nightly-\d{{4}}-\d{{2}}-\d{{2}}|Q[1-4] \d{{4}}|\d+\.\d+\.x|{DIGITS}(?:-[A-Za-z][A-Za-z0-9.]*\d)?)'
MARKER = re.compile(rf'(?:\{{\{{num:({ID})\}}\}}|({DISPLAY})<!-- num:({ID}) -->)')
MARKER_PREFIX = re.compile(r'(?:\{\{\s*num:|<!--\s*num:)')
ONES = r'zero|one|two|three|four|five|six|seven|eight|nine'
TENS = r'twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety'
TEENS = r'ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen'
UNDER_HUNDRED = rf'(?:(?:{TENS})(?:[- ](?:{ONES}))?|{TEENS}|{ONES})'
WORD_NUMBER = rf'{UNDER_HUNDRED}(?:[- ](?:hundred|thousand|million)(?:[- ](?:and[- ])?{UNDER_HUNDRED})?)*'
FIGURE = rf'(?:{DIGITS}|{WORD_NUMBER}|~marked~)'
UNIT = r'CU\b|compute[ -]units?\b|(?:(?:heap|account|storage|encoded|physical|used|payload) )?bytes?\b|[KMGT]i?B\b|B\b|bits?\b'
PAIRED_FIGURES = re.compile(
    rf'(?<![\w.-])(?P<amounts>{FIGURE}(?:(?:\s*(?:/|–|—|-)\s*|\s+(?:and|to|or)\s+){FIGURE})*)'
    rf'(?:\s+|[- ])?(?:{UNIT})', re.I)
FIGURE_AFTER_UNIT = re.compile(
    rf'(?<![\w.-])(?:{UNIT}|heap(?:\s+(?:size|use|usage|budget|limit|watermark|of))?)'
    rf'\s*(?:[:=]\s*|(?:is|was|of|uses|reached)\s+|(?<=\s))(?P<amounts>{FIGURE})', re.I)
COMPARISON = re.compile(
    rf'(?<![\w.-]){FIGURE}\s*[- ]?(?:{UNIT})\s+(?:versus|vs\.?|compared (?:with|to)|against)\s+'
    rf'(?P<amounts>{FIGURE}(?:(?:\s*[/–—-]\s*|\s+(?:and|to|or)\s+){FIGURE})*)', re.I)
ARCH_PAIR = re.compile(
    rf'{FIGURE}\s+(?:{UNIT})\s+on\s+v[03]\s+and\s+(?P<amounts>{FIGURE})\s+on\s+v[03]\b', re.I)
QUANTITY = re.compile(rf'(?<![\w.-]){FIGURE}(?![\w.])', re.I)
METRIC_HEADER = re.compile(
    r'\b(?:CU|compute[ -]units?|bytes?|[KMGT]i?B|heap|account size|instruction cost)\b', re.I)
TERM = re.compile(
    r'\b(?:retained|cohorts?|certif(?:y|ies|ied|ying|ication)|envelopes?|publications?|'
    r'old[- ]dependent|tracked)\b', re.I)
STEP = re.compile(r'\b(?:W\d+|Q\d+|[SMTPDAR]\d+[a-z]?|E[1-9]\d*[a-z]?)\b')
RECEIPT = re.compile(r'\breceipts?\b', re.I)
EVIDENCE = re.compile(r'\b(?:evidence|performance|measurements?|benchmarks?|verification|comparison|'
                      r'reproduc(?:e|tion))\b', re.I)
HEADING = re.compile(r'^\s{0,3}(#{1,6})\s+(.+)$')
FENCE = re.compile(r'^\s{0,3}(`{3,}|~{3,})')
FENCED_NUMBERS = re.compile(
    r'^(?P<open>[ \t]{0,3}(?P<ticks>`{3,}|~{3,})(?![`~])(?P<info>[^\n]*)\n)'
    r'(?P<body>.*?)^(?P<close>[ \t]{0,3}(?P=ticks)[`~]*[ \t]*)$'
    r'(?:\n<!-- num-fence:(?P<bindings>[^\n]*) -->)?', re.M | re.S)
INLINE_NUMBERS = re.compile(
    r'(?<!`)(?P<ticks>`+)(?!`)(?P<body>[^`\n]+)(?P=ticks)(?!`)'
    r'(?:<!-- num-code:(?P<binding>[^\n]*?) -->)?')
TABLE_RULE = re.compile(r'\s*:?-{3,}:?\s*')
LINK = re.compile(r'\]\([^\n)]*\)')
COMMENT = re.compile(r'<!--.*?-->', re.S)


def display(value):
    return f'{int(value):,}' if re.fullmatch(r'-?\d+', value) else value


def marked(text, path, facts, *, render=False, inline=True):
    """Return text with verified markers hidden, or fresh values for rendering."""
    def value(match):
        key = match[1] or match[3]
        fact = facts.get(key)
        if not fact or not fact['value']:
            raise ValueError(f'{path}:{text.count(chr(10), 0, match.start()) + 1}: unavailable number {key}')
        if render:
            return f'{display(fact["value"])}<!-- num:{key} -->' if inline else fact['value']
        actual = re.sub(r'[, _]', '', match[2] or '')
        if actual != re.sub(r'[, _]', '', fact['value']):
            raise ValueError(f'{path}:{text.count(chr(10), 0, match.start()) + 1}: '
                             f'{key} has {match[2] or "an unrendered marker"}, expected {display(fact["value"])}')
        return '~marked~'

    leftovers = MARKER.sub('', text)
    if match := MARKER_PREFIX.search(leftovers):
        raise ValueError(f'{path}:{leftovers.count(chr(10), 0, match.start()) + 1}: malformed number marker')
    return MARKER.sub(value, text)


def marked_numbers(text, path, facts, *, render=False):
    """Keep rendered commands valid and put their templates after the fence."""
    def plain(chunk):
        if re.search(r'<!-- num-(?:code|fence):', chunk):
            raise ValueError(f'{path}: number bindings need an adjacent code span or fence')
        return marked(chunk, path, facts, render=render)

    def outside(chunk):
        parts, offset = [], 0
        for span in INLINE_NUMBERS.finditer(chunk):
            if span['binding'] is None and not MARKER_PREFIX.search(span['body']):
                continue
            parts.append(plain(chunk[offset:span.start()]))
            offset = span.end()
            body = span['body']
            if span['binding'] is not None:
                try:
                    template = json.loads(span['binding'])
                except json.JSONDecodeError as error:
                    raise ValueError(f'{path}: invalid inline code number binding') from error
                if not isinstance(template, str) or not any(m[1] for m in MARKER.finditer(template)):
                    raise ValueError(f'{path}: invalid inline code number binding')
                if render and MARKER_PREFIX.search(body):
                    template = body
            elif render:
                template = body
            else:
                marked(body, path, facts)
                raise ValueError(f'{path}: number bindings need an adjacent code span')
            expected = marked(template, path, facts, render=True, inline=False)
            if not render and body != expected:
                raise ValueError(f'{path}: inline code number or template differs from its source')
            snippet = expected if render else MARKER.sub('~marked~', template)
            parts.extend((span['ticks'], snippet, span['ticks']))
            if render:
                data = json.dumps(template, ensure_ascii=True).replace('>', r'\u003e')
                parts.append(f'<!-- num-code:{data} -->')
        parts.append(plain(chunk[offset:]))
        return ''.join(parts)

    chunks, cursor = [], 0
    for match in FENCED_NUMBERS.finditer(text):
        chunks.append(outside(text[cursor:match.start()]))
        cursor = match.end()
        if re.match(r'^\s*rust(?:\b|,)', match['info']):
            chunks.append(outside(match[0]))
            continue
        try:
            bindings = json.loads(match['bindings']) if match['bindings'] is not None else {}
        except json.JSONDecodeError as error:
            raise ValueError(f'{path}: invalid code fence number bindings') from error
        lines = match['body'].splitlines(keepends=True)
        indices = {str(i) for i in range(1, len(lines) + 1)}
        if not isinstance(bindings, dict) or any(
                line not in indices
                or not isinstance(template, str) or '\n' in template
                or not any(m[1] for m in MARKER.finditer(template))
                for line, template in bindings.items()):
            raise ValueError(f'{path}: invalid code fence number bindings')
        updated = {}
        for i, line in enumerate(lines, 1):
            key = str(i)
            template = bindings.get(key)
            if render and MARKER_PREFIX.search(line):
                template = line.rstrip('\n')
            if template is None:
                marked(line, path, facts, render=False)
                continue
            expected = marked(template, path, facts, render=True, inline=False) + '\n'
            if not render and line != expected:
                location = text.count('\n', 0, match.start('body')) + i
                raise ValueError(f'{path}:{location}: code fence number or template differs from its source')
            lines[i - 1] = expected if render else MARKER.sub('~marked~', template) + '\n'
            updated[key] = template
        chunks.extend((match['open'], ''.join(lines), match['close']))
        if render and updated:
            data = json.dumps(updated, ensure_ascii=True).replace('>', r'\u003e')
            chunks.append(f'\n<!-- num-fence:{data} -->')
        elif match['bindings'] is not None:
            chunks.append('\n')
    chunks.append(outside(text[cursor:]))
    return ''.join(chunks)


CODE_NAME = r'[a-z][a-z0-9_-]*'
CODE_MARKER = re.compile(
    rf'^\{{\{{code:({CODE_NAME})\}}\}}[ \t]*$|'
    rf'^<!-- code:({CODE_NAME}) -->\n```rust\n(.*?)^```$', re.M | re.S)
CODE_PREFIX = re.compile(r'\{\{\s*code:|<!--\s*code:')


def code_excerpts(source, rows=None):
    registry = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'scripts/docs-excerpts.py'))
    return registry['registered_excerpts'](source, rows)


def marked_code(text, path, excerpts, *, render=False):
    def replace(match):
        name = match[1] or match[2]
        location = f'{path}:{text.count(chr(10), 0, match.start()) + 1}'
        if name not in excerpts:
            raise ValueError(f'{location}: unknown excerpt {name}')
        if render:
            return f'<!-- code:{name} -->\n```rust\n{excerpts[name]}\n```'
        if match[1]:
            raise ValueError(f'{location}: unrendered code marker {name}')
        if match[3] != excerpts[name] + '\n':
            raise ValueError(f'{location}: excerpt {name} differs from its source')
        return match[0]

    for match in CODE_MARKER.finditer(text):
        if CODE_PREFIX.search(match[3] or ''):
            raise ValueError(f'{path}: nested code marker')
    leftovers = CODE_MARKER.sub(lambda match: '\n' * match[0].count('\n'), text)
    if match := CODE_PREFIX.search(leftovers):
        raise ValueError(f'{path}:{leftovers.count(chr(10), 0, match.start()) + 1}: malformed code marker')
    return CODE_MARKER.sub(replace, text)


def render_documents(source, facts, destination, *, marker_paths=()):
    changes = {}
    excerpts = None
    for name in sorted(set(source.documents()) | set(map(str, marker_paths))):
        candidate = destination / name
        original = candidate.read_text() if candidate.is_file() else source.read(name)
        updated = original
        if CODE_PREFIX.search(updated):
            if excerpts is None:
                excerpts = code_excerpts(source)
            updated = marked_code(updated, name, excerpts, render=True)
        updated = marked_numbers(updated, name, facts, render=True)
        if updated != original:
            changes[name] = updated
    for name, updated in changes.items():
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(updated)
    return sorted(changes)


def content_lines(text):
    text = COMMENT.sub(lambda match: '\n' * match[0].count('\n'), text)
    return [LINK.sub(']', line) for line in text.splitlines()]


def skip_generated(lines, markers):
    for marker in markers:
        opening = f'<!-- {marker}:start -->'
        closing = f'<!-- {marker}:end -->'
        begin, end = lines.index(opening), lines.index(closing)
        lines[begin:end + 1] = [''] * (end + 1 - begin)
    return '\n'.join(lines)


def quantities(line):
    spans = set()
    for pattern in (PAIRED_FIGURES, FIGURE_AFTER_UNIT, COMPARISON, ARCH_PAIR):
        for match in pattern.finditer(line):
            start = match.start('amounts')
            spans.update((start + amount.start(), amount[0])
                         for amount in QUANTITY.finditer(match['amounts'])
                         if amount[0] != '~marked~')
    return sorted(spans)


def cells(line):
    return re.split(r'(?<!\\)\|', line.strip().strip('|'))


def number_findings(text, path, facts, generated=()):
    checked = marked_numbers(text, path, facts)
    lines = content_lines(skip_generated(checked.splitlines(), generated))
    originals = text.splitlines()
    in_fence = None
    for index, line in enumerate(lines):
        if match := FENCE.match(line):
            token = match[1]
            if in_fence is None:
                in_fence = token
            elif token.startswith(in_fence):
                in_fence = None
            lines[index] = ''
        elif in_fence:
            # Check stated measurements in code comments as well as prose.
            comment = re.match(r'^\s*(?://[/!]?[ \t]*|/\*+|\* )(.+)', line)
            lines[index] = comment[1] if comment else ''
    visible = [line.replace('`', '').replace('**', '') for line in lines]
    findings = defaultdict(dict)
    metric_columns = set()
    for index, line in enumerate(visible):
        following = visible[index + 1] if index + 1 < len(visible) else ''
        for offset, figure in quantities(line):
            findings[index][offset] = figure
        if line and following and '|' not in line + following:
            for offset, figure in quantities(line + ' ' + following):
                if offset < len(line):
                    findings[index][offset] = figure
                else:
                    findings[index + 1][offset - len(line) - 1] = figure
        if '|' not in line:
            metric_columns = set()
            continue
        parts, separator = cells(line), cells(following)
        if len(parts) == len(separator) and all(TABLE_RULE.fullmatch(cell) for cell in separator):
            metric_columns = {column for column, cell in enumerate(parts) if METRIC_HEADER.search(cell)}
        elif not all(TABLE_RULE.fullmatch(cell) for cell in parts):
            column_offset = len(line) - len(line.lstrip()) + line.lstrip().startswith('|')
            for column, part in enumerate(parts):
                if column in metric_columns:
                    for amount in QUANTITY.finditer(part):
                        if amount[0] != '~marked~':
                            findings[index][column_offset + amount.start()] = amount[0]
                column_offset += len(part) + 1
    for index, entries in sorted(findings.items()):
        text = MARKER.sub(lambda match: '{{num:' + (match[1] or match[3]) + '}}',
                          originals[index].strip())
        for _, figure in sorted(entries.items()):
            yield index + 1, figure, text


def term_findings(text):
    headings = []
    fence = None
    for line, content in enumerate(content_lines(text), 1):
        if match := FENCE.match(content):
            token = match[1]
            if fence is None:
                fence = token
            elif token.startswith(fence):
                fence = None
        if not fence and (heading := HEADING.match(content)):
            depth = len(heading[1])
            while headings and headings[-1][0] >= depth:
                headings.pop()
            headings.append((depth, bool(EVIDENCE.search(heading[2]))))
        terms = [*TERM.findall(content), *STEP.findall(content)]
        if not any(evidence for _, evidence in headings):
            terms.extend(RECEIPT.findall(content))
        for term in dict.fromkeys(terms):
            yield line, term


def check(source, facts, allowed, verified_sections):
    errors = []
    allowed = Counter((row['path'], row['figure'], row['text']) for row in allowed)
    numbers_found, terms_found = defaultdict(list), defaultdict(list)
    for path in source.documents():
        text = source.read(path)
        marked_sections = [marker for name, marker in verified_sections if name == path]
        for line, amount, sentence in number_findings(text, path, facts, marked_sections):
            finding = (path, amount, sentence)
            if allowed[finding]:
                numbers_found[path].append(f'{line} ({amount})')
                allowed[finding] -= 1
            else:
                errors.append(f'{path}:{line}: add a number marker for {amount}')
        if path == 'README.md' or path.startswith('docs/'):
            terms_found[path] = [f'{line} ({term})' for line, term in term_findings(text)]
    for name, hits in (('Existing unmarked figures', numbers_found), ('Internal terms (report only)', terms_found)):
        print(f'{name}: {sum(map(len, hits.values()))}')
        for path, matches in hits.items():
            if matches:
                print(f'  {path}: ' + ', '.join(matches))
    if errors:
        raise ValueError('\n'.join(errors))
