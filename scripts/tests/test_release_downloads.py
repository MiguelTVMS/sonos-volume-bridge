"""Regression checks for permanent release download aliases."""
import subprocess
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "prepare-release-downloads.sh"
SUFFIXES = ("macos.zip", "macos.dmg", "windows-x64-unsigned.exe", "windows-arm64-unsigned.exe", "linux-amd64.deb", "linux-arm64.deb")


class ReleaseDownloadsTests(unittest.TestCase):
    def test_site_links_to_published_installers(self):
        links = []

        class Links(HTMLParser):
            def handle_starttag(self, tag, attrs):
                if tag == "a":
                    links.append(dict(attrs).get("href", ""))

        Links().feed((SCRIPT.parent.parent / "pages" / "index.html").read_text())
        for suffix in ("macos.dmg", "windows-x64-unsigned.exe", "windows-arm64-unsigned.exe", "linux-amd64.deb", "linux-arm64.deb"):
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
            self.assertEqual((root / "sonos-volume-bridge-windows-unsigned.exe").read_bytes(), b"windows-x64-unsigned.exe")
            for suffix in SUFFIXES:
                self.assertEqual((root / f"sonos-volume-bridge-{suffix}").read_bytes(), suffix.encode())
                self.assertEqual((root / f"sonos-volume-bridge-v1.2.3-{suffix}").read_bytes(), suffix.encode())

    def test_missing_or_empty_installer_creates_no_aliases(self):
        for missing in SUFFIXES:
            for empty in (False, True):
                with self.subTest(missing=missing, empty=empty), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    for suffix in SUFFIXES:
                        if suffix != missing:
                            (root / f"sonos-volume-bridge-v1.2.3-{suffix}").write_bytes(b"installer")
                    if empty:
                        (root / f"sonos-volume-bridge-v1.2.3-{missing}").touch()
                    result = subprocess.run(["bash", str(SCRIPT), directory, "v1.2.3"], capture_output=True)
                    self.assertNotEqual(result.returncode, 0)
                    for suffix in SUFFIXES:
                        self.assertFalse((root / f"sonos-volume-bridge-{suffix}").exists())



if __name__ == "__main__":
    unittest.main()
