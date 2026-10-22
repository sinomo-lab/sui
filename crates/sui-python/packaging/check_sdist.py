"""Check the actual sdist, including the license paths PyPI validates."""

import argparse
import tarfile
from pathlib import Path, PurePosixPath

from packaging.metadata import Metadata
from packaging.utils import canonicalize_name, parse_sdist_filename
from packaging.version import Version


def check_sdist(archive: Path, version: str, license_file: Path) -> None:
    name, archive_version = parse_sdist_filename(archive.name)
    if name != "sinomo-ui" or archive_version != Version(version):
        raise ValueError(f"Unexpected distribution: {archive.name}")
    root = archive.name.removesuffix(".tar.gz")
    with tarfile.open(archive, "r:gz") as sdist:
        members = {member.name: member for member in sdist.getmembers()}

        def read(relative: str) -> bytes:
            member = members.get(f"{root}/{relative}")
            if member is None or not member.isfile():
                raise ValueError(f"Missing regular file in sdist: {relative}")
            return sdist.extractfile(member).read()

        metadata = Metadata.from_email(read("PKG-INFO"), validate=True)
        if canonicalize_name(metadata.name) != name or metadata.version != archive_version:
            raise ValueError("PKG-INFO name/version does not match the archive")
        if metadata.license_expression != "MIT":
            raise ValueError("Expected License-Expression: MIT")
        if not metadata.license_files or "LICENSE" not in metadata.license_files:
            raise ValueError("Expected License-File: LICENSE")
        read("pyproject.toml")
        for declared in metadata.license_files:
            path = PurePosixPath(declared)
            if path.is_absolute() or ".." in path.parts or "\\" in declared:
                raise ValueError(f"Invalid License-File path: {declared}")
            if not read(declared).decode("utf-8").strip():
                raise ValueError(f"Empty license file: {declared}")
        # The Python copy must stay in sync with the workspace license.
        if read("LICENSE").decode("utf-8").splitlines() != license_file.read_text(
            encoding="utf-8"
        ).splitlines():
            raise ValueError("Packaged LICENSE differs from the workspace LICENSE")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--license", type=Path, required=True, dest="license_file")
    args = parser.parse_args()
    check_sdist(args.archive, args.version, args.license_file)
    print(f"Validated {args.archive}: metadata and all declared license files")
