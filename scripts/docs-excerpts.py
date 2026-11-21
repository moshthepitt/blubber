"""Read the source regions shown in the user docs and check their test links."""
import csv
import io
from pathlib import Path
import re
from textwrap import dedent

REGISTRY = Path("docs/facts/excerpts.tsv")
NAME = r"[a-z][a-z0-9_-]*"
REGION = re.compile(rf"^[ \t]*(//! )?// docs:(start|end) ({NAME})[ \t]*$", re.M)
DOC_FENCE = re.compile(r"^//! ```(rust)?\n(?:^//!.*\n)+?^//! ```[ \t]*$", re.M)


def regions(text, path):
    markers = list(REGION.finditer(text))
    fences = list(DOC_FENCE.finditer(text))
    if len(markers) % 2:
        raise ValueError(f"{path}: unmatched excerpt markers")
    excerpts = {}
    for start, end in zip(markers[::2], markers[1::2]):
        prefix, edge, name = start.groups()
        if edge != "start" or end.groups() != (prefix, "end", name):
            raise ValueError(f"{path}: unmatched excerpt {name}")
        body = text[start.end() + 1:end.start()].rstrip("\n")
        if prefix:
            body = "\n".join(line.removeprefix("//! ").removeprefix("//!")
                             for line in body.splitlines())
        body = dedent(body).strip("\n")
        if not body.strip():
            raise ValueError(f"{path}: empty excerpt {name}")
        in_doctest = any(fence.start() < start.start() < end.end() < fence.end() for fence in fences)
        excerpts.setdefault(name, []).append((body, bool(prefix and in_doctest)))
    return excerpts


def registry_rows(source):
    rows = csv.DictReader(io.StringIO(source.read(REGISTRY)), delimiter="\t", strict=True)
    if rows.fieldnames != ["name", "source", "proof_source", "proof_test", "parts"]:
        raise ValueError("Expected name, source, proof_source, proof_test and parts in excerpt registry")
    return list(rows)


def registered_excerpts(source, rows=None):
    if rows is None:
        rows = registry_rows(source)
        if not rows:
            raise ValueError("Empty excerpt registry")
    files, excerpts, defining_file = {}, {}, {}
    for row in rows:
        name, file, proof, test, parts = (row[key] for key in ("name", "source", "proof_source", "proof_test", "parts"))
        if None in row or not re.fullmatch(NAME, name or "") or name in excerpts:
            raise ValueError(f"Invalid or duplicate excerpt registration: {name}")
        if not file or not proof or not test:
            raise ValueError(f"{name}: excerpt needs a proving test")
        for location in (file, proof):
            path = Path(location)
            if path.is_absolute() or ".." in path.parts or path.suffix != ".rs" or path.as_posix() != location:
                raise ValueError(f"{name}: invalid excerpt source {location}")
        if file not in files:
            files[file] = regions(source.read(Path(file)), file)
        regions_for_name = files[file].get(name, [])
        if not regions_for_name or str(len(regions_for_name)) != parts:
            raise ValueError(f"{file}: expected {parts} regions for excerpt {name}")
        if test == "rustdoc":
            valid = file == proof and all(in_doctest for _, in_doctest in regions_for_name)
        else:
            pattern = rf"^((?:[ \t]*#\[[^\n]*\][ \t]*\n)+)[ \t]*fn {re.escape(test)}\("
            attributes = re.findall(pattern, source.read(Path(proof)), re.M)
            valid = (bool(re.fullmatch(r"[a-zA-Z_]\w*", test)) and len(attributes) == 1
                     and "#[test]" in attributes[0]
                     and not re.search(r"\b(?:ignore|should_panic)\b", attributes[0]))
        if not valid:
            raise ValueError(f"{name}: missing runnable proving test {proof} / {test}")
        excerpts[name] = "\n\n".join(body for body, _ in regions_for_name)
        defining_file[name] = file
    for file, marked in files.items():
        for name in marked:
            if defining_file.get(name) != file:
                raise ValueError(f"{file}: excerpt {name} needs a proving test")
    return excerpts
