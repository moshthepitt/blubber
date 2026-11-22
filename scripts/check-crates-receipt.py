#!/usr/bin/env python3
"""Verify packaged sources and look up exact versions on crates.io."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
RECEIPT = Path("verification/results/release/crates.json")
CRATES = ("blubber-core", "blubber-derive", "blubber")
VERSION = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
RUN_ID = r"[0-9]{8}T[0-9]{6}Z-[0-9a-f]{12}"


def cargo(root, *args):
    return subprocess.check_output(["cargo", "+1.97.1", *args], cwd=root, text=True)


def package_sources(root, crate):
    listed = cargo(root, "package", "--list", "--locked", "--offline", "--allow-dirty", "-p", crate)
    directory = Path("crates") / crate
    # Cargo generates the normalized manifest, lockfile and VCS metadata. The
    # original manifest is archived as Cargo.toml.orig. Include the workspace
    # manifest too, since Cargo inherits package and dependency settings from it.
    sources = {Path("Cargo.toml")}
    for name in listed.splitlines():
        if name in {"Cargo.toml", "Cargo.lock", ".cargo_vcs_info.json"}:
            continue
        path = directory / ("Cargo.toml" if name == "Cargo.toml.orig" else name)
        if path.is_absolute() or ".." in path.parts or path in sources:
            raise ValueError(f"Invalid package file for {crate}: {name!r}")
        sources.add(path)
    if {directory / "Cargo.toml", directory / "src/lib.rs"} - sources:
        raise ValueError(f"Missing package sources for {crate}")
    return sorted(sources)


def package_state(root, staged_docs=None):
    root = root.resolve()
    metadata = json.loads(cargo(root, "metadata", "--no-deps", "--format-version", "1", "--locked", "--offline"))
    packages = {Path(pkg["manifest_path"]): pkg for pkg in metadata["packages"]}
    versions, digests = {}, {}
    for crate in CRATES:
        package = packages.get(root / "crates" / crate / "Cargo.toml")
        if package is None or package["name"] != crate:
            raise ValueError(f"Missing workspace package {crate}")
        versions[crate] = package["version"]
        digest = hashlib.sha256()
        for path in package_sources(root, crate):
            source = root / path
            if not source.is_file() or source.is_symlink() or not source.resolve().is_relative_to(root):
                raise ValueError(f"Expected a regular package source at {path}")
            data = source.read_bytes()
            if staged_docs is not None:
                staged = staged_docs / path
                if staged.is_file() and staged.read_bytes() != data:
                    raise ValueError(f"Staged documentation changes packaged source {path}. Run release checks on the updated source.")
            digest.update(path.as_posix().encode() + b"\0" + hashlib.sha256(data).digest())
        digests[crate] = digest.hexdigest()
    return versions, digests


def release_version(tag):
    if re.fullmatch("v" + VERSION, tag) is None:
        raise ValueError(f"Expected a vMAJOR.MINOR.PATCH tag, got {tag!r}")
    return tag[1:]


def verify(root, receipt, tag):
    version = release_version(tag)
    evidence = json.loads(receipt.read_text())
    if not isinstance(evidence, dict) or evidence.get("version") != version:
        raise ValueError(f"Release receipt version differs from {tag}")
    run_id, lanes = evidence.get("run_id"), evidence.get("lanes_passed")
    if not isinstance(run_id, str) or not re.fullmatch(RUN_ID, run_id) or type(lanes) is not int or lanes < 1:
        raise ValueError("Invalid release run id or passing lane count")
    expected = evidence.get("sha256")
    if not isinstance(expected, dict) or set(expected) != set(CRATES):
        raise ValueError("Release receipt has an invalid crate inventory")
    versions, digests = package_state(root)
    for crate in CRATES:
        if versions[crate] != version:
            raise ValueError(f"{crate} version {versions[crate]} differs from {tag}")
        if digests[crate] != expected[crate]:
            raise ValueError(f"{crate} packaged source digest differs from the release receipt")
    print(f"{tag}: {lanes} passing lanes in {run_id}. All three crate digests match.")


def version_exists(crate, version):
    request = Request(f"https://index.crates.io/{crate[:2]}/{crate[2:4]}/{crate}",
                      headers={"Cache-Control": "no-cache"})
    try:
        with urlopen(request, timeout=30) as response:
            rows = [json.loads(line) for line in response if line.strip()]
    except HTTPError as error:
        if error.code == 404:
            return False
        raise
    if not rows or any(not isinstance(row, dict) or row.get("name") != crate
                       or not isinstance(row.get("vers"), str) for row in rows):
        raise ValueError(f"Invalid crates.io index entry for {crate}")
    return any(row["vers"] == version for row in rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("verify")
    check.add_argument("tag")
    check.add_argument("--receipt", type=Path, default=ROOT / RECEIPT)
    commands.add_parser("digests", help="Print the three packaged source digests")
    lookup = commands.add_parser("exists")
    lookup.add_argument("crate", choices=CRATES)
    lookup.add_argument("tag")
    args = parser.parse_args()
    try:
        if args.command == "exists":
            return 0 if version_exists(args.crate, release_version(args.tag)) else 1
        if args.command == "digests":
            print(json.dumps(package_state(ROOT)[1], indent=2))
        else:
            verify(ROOT, args.receipt, args.tag)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
