#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import tarfile
import zipfile
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def binary_name(name: str, windows: bool) -> str:
    return f"{name}.exe" if windows else name


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Package Awo release binaries and top-level docs into one archive.",
    )
    parser.add_argument("--profile", default="release", choices=("debug", "release"))
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--version-label", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--output-dir", type=Path, default=Path("dist"))
    parser.add_argument(
        "--format",
        choices=("auto", "zip", "tar.gz"),
        default="auto",
        help="Archive format. Defaults to zip on Windows and tar.gz elsewhere.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = repo_root()
    binary_dir = (args.binary_dir or (root / "target" / args.profile)).resolve()
    windows = "windows" in args.target_name.lower() or os.name == "nt"
    archive_format = args.format
    if archive_format == "auto":
        archive_format = "zip" if windows else "tar.gz"

    staging_name = f"awo-{args.version_label}-{args.target_name}"
    output_dir = (root / args.output_dir).resolve()
    staging_dir = output_dir / staging_name
    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    staging_dir.mkdir(parents=True, exist_ok=True)

    for filename in (
        binary_name("awo", windows),
        binary_name("awod", windows),
        binary_name("awo-mcp", windows),
        "README.md",
        "LICENSE",
        "docs/release-process.md",
    ):
        source = (binary_dir / filename) if filename.startswith("awo") else (root / filename)
        if not source.exists():
            raise FileNotFoundError(f"missing required release asset: {source}")
        target = staging_dir / filename
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            raise RuntimeError(f"release asset must be a file, not a directory: {source}")
        target.write_bytes(source.read_bytes())

    if archive_format == "zip":
        archive_path = output_dir / f"{staging_name}.zip"
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for file_path in staging_dir.rglob("*"):
                archive.write(file_path, file_path.relative_to(output_dir))
    else:
        archive_path = output_dir / f"{staging_name}.tar.gz"
        with tarfile.open(archive_path, "w:gz") as archive:
            archive.add(staging_dir, arcname=staging_dir.name)

    print(archive_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
