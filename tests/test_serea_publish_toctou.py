"""Focused TOCTOU tests for verified artifact publication.

These tests exercise the descriptor-bound publication helper directly. They use
only test-owned temporary files under /private/tmp and never touch signing
assets, mounted Vaults, or pre-existing artifacts.
"""
from __future__ import annotations

import hashlib
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import serea_signing as cli


PRIVATE_TMP = Path("/private/tmp")


class PublishVerifiedArtifactTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="serea-toctou-", dir=PRIVATE_TMP)
        self.base = Path(self._tmp.name)
        os.chmod(self.base, 0o700)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _write(self, name: str, data: bytes, mode: int = 0o400) -> Path:
        path = self.base / name
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(path, mode)
        return path

    @staticmethod
    def _hash(data: bytes) -> str:
        return hashlib.sha256(data).hexdigest()

    def test_observed_hash_rejects_symlink(self) -> None:
        target = self._write("target.bin", b"hello")
        link = self.base / "link.bin"
        os.symlink(target, link)
        self.assertIsNone(cli._observed_file_sha256(link))

    def test_observed_hash_matches_fd_bytes(self) -> None:
        path = self._write("ok.bin", b"abc")
        self.assertEqual(cli._observed_file_sha256(path), self._hash(b"abc"))

    def test_publish_copies_verified_bytes_to_fresh_output(self) -> None:
        source = self._write("signed.apk", b"APK-CONTENT")
        destination = self.base / "out.apk"
        published = cli._publish_verified_artifact(
            source, destination, self._hash(b"APK-CONTENT"), label="final APK", file_mode=0o400)
        self.assertEqual(published, self._hash(b"APK-CONTENT"))
        self.assertEqual(destination.read_bytes(), b"APK-CONTENT")
        self.assertEqual(stat.S_IMODE(destination.lstat().st_mode), 0o400)

    def test_publish_refuses_existing_conflicting_output(self) -> None:
        source = self._write("signed.apk", b"NEW-BYTES")
        destination = self._write("out.apk", b"OLD-BYTES")
        original_inode = destination.lstat().st_ino
        with self.assertRaises(cli.SigningError):
            cli._publish_verified_artifact(
                source, destination, self._hash(b"NEW-BYTES"), label="final APK", file_mode=0o400)
        self.assertEqual(destination.read_bytes(), b"OLD-BYTES")
        self.assertEqual(destination.lstat().st_ino, original_inode)

    def test_publish_is_idempotent_for_matching_output(self) -> None:
        source = self._write("signed.apk", b"SAME")
        destination = self._write("out.apk", b"SAME")
        original_inode = destination.lstat().st_ino
        cli._publish_verified_artifact(
            source, destination, self._hash(b"SAME"), label="final APK", file_mode=0o400)
        self.assertEqual(destination.lstat().st_ino, original_inode)

    def test_publish_rejects_source_symlink(self) -> None:
        target = self._write("signed.apk", b"APK")
        link = self.base / "signed-link.apk"
        os.symlink(target, link)
        destination = self.base / "out.apk"
        with self.assertRaises(cli.SigningError):
            cli._publish_verified_artifact(
                link, destination, self._hash(b"APK"), label="final APK", file_mode=0o400)
        self.assertFalse(destination.exists())

    def test_publish_detects_wrong_expected_hash(self) -> None:
        source = self._write("signed.apk", b"ACTUAL")
        destination = self.base / "out.apk"
        with self.assertRaisesRegex(cli.SigningError, "changed before publication"):
            cli._publish_verified_artifact(
                source, destination, self._hash(b"EXPECTED"), label="final APK", file_mode=0o400)
        self.assertFalse(destination.exists())

    def test_publish_detects_source_byte_replacement(self) -> None:
        source = self._write("signed.apk", b"VERIFIED")
        destination = self.base / "out.apk"
        real_read = os.read
        state = {"tampered": False}

        def tampered_read(fd: int, size: int) -> bytes:
            if not state["tampered"]:
                state["tampered"] = True
                return b"TAMPERED-BYTES"
            return b""

        with mock.patch.object(cli.os, "read", side_effect=tampered_read):
            with self.assertRaisesRegex(cli.SigningError, "changed before publication"):
                cli._publish_verified_artifact(
                    source, destination, self._hash(b"VERIFIED"), label="final APK", file_mode=0o400)
        self.assertFalse(destination.exists())

    def test_publish_detects_link_redirection_to_unverified_bytes(self) -> None:
        source = self._write("signed.apk", b"VERIFIED")
        attacker = self._write("attacker.apk", b"ATTACKER")
        destination = self.base / "out.apk"

        def hostile_link(src: os.PathLike[str], dst: os.PathLike[str], *args: object, **kwargs: object) -> None:
            # Simulate a same-UID writer redirecting the final link to other bytes.
            with open(attacker, "rb") as incoming, open(dst, "wb") as outgoing:
                outgoing.write(incoming.read())

        with mock.patch.object(cli.os, "link", side_effect=hostile_link):
            with self.assertRaises(cli.SigningError):
                cli._publish_verified_artifact(
                    source, destination, self._hash(b"VERIFIED"), label="final APK", file_mode=0o400)
        self.assertEqual(destination.read_bytes(), b"ATTACKER")


if __name__ == "__main__":
    unittest.main()
