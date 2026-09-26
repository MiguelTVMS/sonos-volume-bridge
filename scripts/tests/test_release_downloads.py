"""Regression checks for permanent release download aliases."""
import subprocess
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "prepare-release-downloads.sh"
SUFFIXES = ("macos.zip", "windows-unsigned.exe", "linux-amd64.deb", "linux-arm64.deb")


class ReleaseDownloadsTests(unittest.TestCase):
    def test_site_links_to_both_published_linux_architectures(self):
        links = []

        class Links(HTMLParser):
            def handle_starttag(self, tag, attrs):
                if tag == "a":
                    links.append(dict(attrs).get("href", ""))

        Links().feed((SCRIPT.parent.parent / "pages" / "index.html").read_text())
        for suffix in ("linux-amd64.deb", "linux-arm64.deb"):
            self.assertIn(suffix, SUFFIXES)
            self.assertTrue(any(link.endswith(
                f"/releases/latest/download/sonos-volume-bridge-{suffix}"
            ) for link in links))

    def test_copies_preserve_source_content(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for suffix in SUFFIXES:
                (root / f"sonos-volume-bridge-v1.2.3-{suffix}").write_bytes(suffix.encode())
            for _ in range(2):
                subprocess.run(["bash", str(SCRIPT), directory, "v1.2.3"], check=True)
            for suffix in SUFFIXES:
                self.assertEqual((root / f"sonos-volume-bridge-{suffix}").read_bytes(), suffix.encode())
                self.assertEqual((root / f"sonos-volume-bridge-v1.2.3-{suffix}").read_bytes(), suffix.encode())

    def test_missing_or_empty_installer_creates_no_aliases(self):
        for empty in (False, True):
            with self.subTest(empty=empty), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                for suffix in SUFFIXES[:-1]:
                    (root / f"sonos-volume-bridge-v1.2.3-{suffix}").write_bytes(b"installer")
                if empty:
                    (root / f"sonos-volume-bridge-v1.2.3-{SUFFIXES[-1]}").touch()
                result = subprocess.run(["bash", str(SCRIPT), directory, "v1.2.3"], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                for suffix in SUFFIXES:
                    self.assertFalse((root / f"sonos-volume-bridge-{suffix}").exists())


if __name__ == "__main__":
    unittest.main()
