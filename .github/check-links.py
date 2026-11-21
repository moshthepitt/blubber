#!/usr/bin/env python3
"""Check local links, published receipts and Rust fences."""

from collections import Counter
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
RECEIPTS = ROOT / "verification/results"
REPO_URL = "https://github.com/moshthepitt/blubber/blob/v0.1.0/"
INLINE = re.compile(r'\]\(<?([^\s)<>]+)>?(?:\s+"[^"]*")?\)')
REFERENCE = re.compile(r'^\s*\[[^\]\n]+\]:\s+<?([^\s<>]+)>?', re.M)
CODE = re.compile(r'(?<!`)(`+)(?!`)(.+?)(?<!`)\1(?!`)', re.S)
REPO_PATH = re.compile(
    r'(?<![\w./:@-])(?:/|(?:\.\.?/)*)'
    r'(?:scripts|verification|benchmarks|programs|crates|tests|fuzz|docs|\.github|\.cargo)/'
    r'[\w.%+@~-]+(?:/[\w.%+@~-]+)*/?(?:#[\w.%-]+)?'
)
FENCES = re.compile(
    r'^[ \t]*((`|~)\2{2,})(?!\2)([^\n]*)\n(.*?)^[ \t]*\1\2*[ \t]*$',
    re.M | re.S,
)
COMMENTS = re.compile(r"<!--.*?-->", re.S)


def docs(root):
    for path in sorted(root.rglob("*.md")):
        if any(parent in {".git", "target", ".tmp"} for parent in path.relative_to(root).parts):
            continue
        yield path, path.read_text()
    for base in ("crates", "programs", "tests", "benchmarks", "verification", "fuzz"):
        for path in sorted((root / base).rglob("*.rs")):
            lines = re.findall(r"^\s*//!(?: |$)([^\n]*)", path.read_text(), re.M)
            if lines:
                yield path, "\n".join(lines)


def anchors(path):
    text = FENCES.sub("", COMMENTS.sub("", path.read_text()))
    counts = Counter()
    result = set(re.findall(r'\bid=["\']([^"\']+)["\']', text))
    for line in re.findall(r"^#{1,6} (.+?)\s*#*\s*$", text, re.M):
        line = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", line)
        line = re.sub(r"[^\w\s-]", "", line.lower()).replace(" ", "-")
        result.add(line if not counts[line] else f"{line}-{counts[line]}")
        counts[line] += 1
    return result


def rustdoc_scaffolding(text, path):
    # Remove the Markdown block quote prefix while preserving source line numbers.
    text = re.sub(r"^(?:[ \t]*>[ \t]?)+", "", text, flags=re.M)
    for fence in FENCES.finditer(text):
        tags = re.split(r"[,\s]+", fence[3].strip())
        if tags[0] not in {"rust", "rs"}:
            continue
        first = text.count("\n", 0, fence.start(4)) + 1
        for line_number, line in enumerate(fence[4].splitlines(), first):
            if re.match(r"^\s*#(?:\s|$)", line):
                yield f"{path}:{line_number}: rustdoc-only line in a public Rust fence"


def check(root=ROOT):
    errors = []
    receipts = set()
    for doc, text in docs(root):
        if doc.suffix == ".md":
            errors.extend(rustdoc_scaffolding(text, doc.relative_to(root)))
        text = COMMENTS.sub("", text)
        prose = FENCES.sub("", text)
        links = [(target, False) for target in INLINE.findall(prose) + REFERENCE.findall(prose)]
        snippets = [snippet for _, snippet in CODE.findall(prose)]
        snippets.extend(fence[4] for fence in FENCES.finditer(text))
        links.extend((target, True) for target in dict.fromkeys(
            target for snippet in snippets for target in REPO_PATH.findall(snippet)))
        for raw, code in links:
            match = urlsplit(raw)
            if raw.startswith(REPO_URL):
                target = root / unquote(match.path.split("/v0.1.0/", 1)[1])
            elif match.scheme or match.netloc:
                continue
            elif not match.path:
                target = doc
            else:
                path = unquote(match.path)
                base = doc.parent
                if path.startswith("/") or (code and not path.startswith("../")):
                    base, path = root, path.lstrip("/")
                target = base / path
            target = target.resolve()
            name = doc.relative_to(root)
            if not target.is_relative_to(root) or not target.exists():
                errors.append(f"{name}: missing local path {raw}")
            elif match.fragment and target.suffix == ".md" and unquote(match.fragment) not in anchors(target):
                errors.append(f"{name}: missing fragment {raw}")
            elif not code and target.is_relative_to(root / "verification/results"):
                if target.is_dir():
                    receipts.update(p for p in target.rglob("*") if p.is_file())
                else:
                    receipts.add(target)
    shipped = {p for p in (root / "verification/results").rglob("*") if p.is_file()}
    for path in sorted(shipped - receipts):
        errors.append(f"Unlinked receipt: {path.relative_to(root)}")
    return errors


if __name__ == "__main__":
    errors = check()
    if errors:
        sys.exit("\n".join(errors))
    print("Local documentation links and receipt inventory passed")
