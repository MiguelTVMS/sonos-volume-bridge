"""Exercise the release collector with real Debian package metadata."""
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "collect-linux-installer.sh"


class LinuxInstallerTests(unittest.TestCase):
    def package(self, root, arch):
        control = root / "package" / "DEBIAN"
        control.mkdir(parents=True)
        (control / "control").write_text(
            "Package: test-installer\nVersion: 1.2.3\n"
            f"Architecture: {arch}\nMaintainer: Test <test@example.invalid>\n"
            "Description: Package fixture\n"
        )
        bundle = root / "bundle"
        bundle.mkdir()
        package = bundle / "installer.deb"
        subprocess.run(["dpkg-deb", "--build", "--root-owner-group",
                        str(control.parent), str(package)], check=True, capture_output=True)
        return bundle, package

    def collect(self, root, bundle, arch):
        return subprocess.run(["bash", str(SCRIPT), str(bundle), "v1.2.3", arch],
                              cwd=root, capture_output=True)

    def test_native_packages_keep_distinct_architecture_and_contents(self):
        for arch in ("amd64", "arm64"):
            with self.subTest(arch=arch), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                bundle, package = self.package(root, arch)
                result = self.collect(root, bundle, arch)
                self.assertEqual(result.returncode, 0, result.stderr)
                output = root / f"sonos-volume-bridge-v1.2.3-linux-{arch}.deb"
                self.assertEqual(output.read_bytes(), package.read_bytes())

    def test_rejects_wrong_architecture_missing_empty_and_ambiguous_packages(self):
        for case in ("wrong", "missing", "empty", "multiple"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                bundle, package = self.package(root, "amd64" if case == "wrong" else "arm64")
                if case == "missing":
                    package.unlink()
                elif case == "empty":
                    package.write_bytes(b"")
                elif case == "multiple":
                    (bundle / "other.deb").write_bytes(package.read_bytes())
                self.assertNotEqual(self.collect(root, bundle, "arm64").returncode, 0)
                self.assertFalse(list(root.glob("sonos-volume-bridge-*.deb")))


if __name__ == "__main__":
    unittest.main()
