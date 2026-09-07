"""Exercise the real installer with local download fixtures; no network needed."""

import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[1] / "install.sh"
ASSET = "stopproof-x86_64-unknown-linux-musl"
NEW_BINARY = b"#!/bin/sh\necho 'stopproof 0.2.0'\n"


@unittest.skipIf(os.name == "nt", "POSIX installer runs on Linux and macOS")
class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="stopproof-installer-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.install = self.root / "install path"
        self.install.mkdir()
        self.destination = self.install / "stopproof"
        self.destination.write_bytes(b"old working binary\n")
        self.destination.chmod(0o755)
        (self.root / "binary").write_bytes(NEW_BINARY)
        digest = hashlib.sha256(NEW_BINARY).hexdigest()
        (self.root / "SHA256SUMS").write_text(f"{digest}  {ASSET}\n")
        self.executable("uname", "#!/bin/sh\ncase \"$1\" in -s) echo Linux ;; -m) echo x86_64 ;; esac\n")
        self.executable("curl", """#!/usr/bin/env python3
import os, pathlib, sys
root = pathlib.Path(os.environ['INSTALLER_FIXTURES'])
url = next(arg for arg in sys.argv if arg.startswith('https://'))
dest = pathlib.Path(sys.argv[sys.argv.index('-o') + 1])
mode = os.environ.get('INSTALLER_TEST_MODE', '')
manifest = url.endswith('/SHA256SUMS')
if mode == 'download-failure' and not manifest:
    dest.write_bytes(b'partial broken download')
    sys.exit(22)
if mode == 'missing-manifest' and manifest:
    sys.exit(22)
dest.write_bytes((root / ('SHA256SUMS' if manifest else 'binary')).read_bytes())
""")

    def executable(self, name, text):
        script = self.bin / name
        script.write_text(text)
        script.chmod(0o755)

    def run_installer(self, mode=""):
        env = os.environ.copy()
        env.update(
            PATH=str(self.bin) + os.pathsep + env.get("PATH", ""),
            STOPPROOF_INSTALL_DIR=str(self.install),
            INSTALLER_FIXTURES=str(self.root),
            INSTALLER_TEST_MODE=mode,
        )
        return subprocess.run(["sh", str(INSTALLER)], env=env, capture_output=True, text=True, timeout=10)

    def assert_preserved(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.destination.read_bytes(), b"old working binary\n")
        self.assertEqual(list(self.install.iterdir()), [self.destination])

    def test_failed_download_preserves_previous_binary(self):
        self.assert_preserved(self.run_installer("download-failure"))

    def test_missing_manifest_preserves_previous_binary(self):
        self.assert_preserved(self.run_installer("missing-manifest"))

    def test_checksum_mismatch_preserves_previous_binary(self):
        (self.root / "SHA256SUMS").write_text(f"{'0' * 64}  {ASSET}\n")
        self.assert_preserved(self.run_installer())

    def test_missing_asset_checksum_preserves_previous_binary(self):
        (self.root / "SHA256SUMS").write_text(f"{'0' * 64}  unrelated-binary\n")
        self.assert_preserved(self.run_installer())

    def test_duplicate_checksum_is_rejected(self):
        checksum = (self.root / "SHA256SUMS").read_text()
        (self.root / "SHA256SUMS").write_text(checksum * 2)
        self.assert_preserved(self.run_installer())

    def test_verified_download_replaces_previous_binary(self):
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.destination.read_bytes(), NEW_BINARY)
        self.assertTrue(os.access(self.destination, os.X_OK))
        self.assertEqual(list(self.install.iterdir()), [self.destination])

    def test_checksum_matched_but_unusable_binary_preserves_previous(self):
        binary = b"#!/bin/sh\nexit 1\n"
        (self.root / "binary").write_bytes(binary)
        digest = hashlib.sha256(binary).hexdigest()
        (self.root / "SHA256SUMS").write_text(f"{digest}  {ASSET}\n")
        self.assert_preserved(self.run_installer())


if __name__ == "__main__":
    unittest.main()
