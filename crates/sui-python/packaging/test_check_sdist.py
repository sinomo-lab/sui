"""Exercise rejection of the broken 0.5.0 layout without loading the extension."""

import io
import tarfile
import tempfile
import unittest
from pathlib import Path

from check_sdist import check_sdist


class SdistChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.license = self.directory / "LICENSE"
        self.license.write_text("MIT License\n", encoding="utf-8")

    def archive(self, *, license_path="LICENSE", metadata_license="LICENSE", version="0.5.0"):
        archive = self.directory / "sinomo_ui-0.5.0.tar.gz"
        metadata = (
            "Metadata-Version: 2.4\nName: sinomo-ui\n"
            f"Version: {version}\nLicense-Expression: MIT\n"
            f"License-File: {metadata_license}\n\n"
        )
        files = {"PKG-INFO": metadata.encode(), "pyproject.toml": b"[build-system]\n"}
        files[license_path] = self.license.read_bytes()
        with tarfile.open(archive, "w:gz") as sdist:
            for path, data in files.items():
                member = tarfile.TarInfo(f"sinomo_ui-0.5.0/{path}")
                member.size = len(data)
                sdist.addfile(member, io.BytesIO(data))
        return archive

    def test_valid_sdist(self):
        check_sdist(self.archive(), "0.5.0", self.license)

    def test_crate_license_does_not_satisfy_root_metadata(self):
        with self.assertRaisesRegex(ValueError, "Missing regular file.*LICENSE"):
            check_sdist(
                self.archive(license_path="crates/sui-python/LICENSE"), "0.5.0", self.license
            )

    def test_metadata_version_must_match_filename(self):
        with self.assertRaisesRegex(ValueError, "name/version"):
            check_sdist(self.archive(version="0.6.0"), "0.5.0", self.license)

    def test_every_declared_license_must_exist(self):
        with self.assertRaisesRegex(ValueError, "Missing regular file.*NOTICE"):
            check_sdist(
                self.archive(metadata_license="LICENSE\nLicense-File: NOTICE"),
                "0.5.0",
                self.license,
            )

    def test_license_copy_must_match_workspace(self):
        archive = self.archive()
        self.license.write_text("Different license\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "differs"):
            check_sdist(archive, "0.5.0", self.license)


if __name__ == "__main__":
    unittest.main()
