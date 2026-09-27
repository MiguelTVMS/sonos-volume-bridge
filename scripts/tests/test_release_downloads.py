"""Regression checks for unique permanent release downloads."""
import subprocess
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "prepare-release-downloads.sh"
SUFFIXES = ("macos.dmg", "windows-x64-unsigned.exe", "windows-arm64-unsigned.exe", "linux-amd64.deb", "linux-arm64.deb")
DESTINATIONS = tuple("linux-x64.deb" if suffix == "linux-amd64.deb" else suffix for suffix in SUFFIXES)


class ReleaseDownloadsTests(unittest.TestCase):
    def test_site_links_to_published_installers(self):
        links = []

        class Links(HTMLParser):
            def handle_starttag(self, tag, attrs):
                if tag == "a":
                    links.append(dict(attrs).get("href", ""))

        Links().feed((SCRIPT.parent.parent / "pages" / "index.html").read_text())
        for suffix in ("macos.dmg", "windows-x64-unsigned.exe", "windows-arm64-unsigned.exe", "linux-x64.deb", "linux-arm64.deb"):
            self.assertIn(suffix, DESTINATIONS)
            self.assertTrue(any(link.endswith(
                f"/releases/latest/download/sonos-volume-bridge-{suffix}"
            ) for link in links))

    def test_publication_contains_one_file_per_package_and_can_be_repeated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for suffix in SUFFIXES:
                (root / f"sonos-volume-bridge-v1.2.3-{suffix}").write_bytes(suffix.encode())
            for _ in range(2):
                subprocess.run(["bash", str(SCRIPT), directory, "v1.2.3"], check=True)
                self.assertEqual(
                    {path.name for path in root.iterdir()},
                    {f"sonos-volume-bridge-{suffix}" for suffix in DESTINATIONS},
                )
                for source, destination in zip(SUFFIXES, DESTINATIONS):
                    self.assertEqual((root / f"sonos-volume-bridge-{destination}").read_bytes(), source.encode())

    def test_missing_or_empty_installer_leaves_all_inputs_untouched(self):
        for missing in SUFFIXES:
            for empty in (False, True):
                with self.subTest(missing=missing, empty=empty), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    for suffix in SUFFIXES:
                        if suffix != missing:
                            (root / f"sonos-volume-bridge-v1.2.3-{suffix}").write_bytes(b"installer")
                    if empty:
                        (root / f"sonos-volume-bridge-v1.2.3-{missing}").touch()
                    before = {p.name: p.read_bytes() for p in root.iterdir()}
                    result = subprocess.run(["bash", str(SCRIPT), directory, "v1.2.3"], capture_output=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(before, {p.name: p.read_bytes() for p in root.iterdir()})



if __name__ == "__main__":
    unittest.main()
