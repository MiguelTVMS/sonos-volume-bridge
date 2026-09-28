"""Check installer identities against the established upgrade contract."""
import json
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

class RebrandIdentityTests(unittest.TestCase):
    def test_store_and_settings_identities_are_preserved(self):
        config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
        self.assertEqual(config['identifier'], 'ms.miguel.sonosvolumebridge.desktop')
        self.assertEqual(config['productName'], 'Speaker Volume Bridge')
        manifest = ET.fromstring((ROOT / 'packaging/windows-msix/AppxManifest.xml.template').read_text())
        ns = {'p': 'http://schemas.microsoft.com/appx/manifest/foundation/windows10',
              'desktop': 'http://schemas.microsoft.com/appx/manifest/desktop/windows10'}
        self.assertEqual(manifest.find('p:Identity', ns).attrib['Name'], 'Miguel.MS.SonosVolumeBridge')
        app = manifest.find('p:Applications/p:Application', ns)
        self.assertEqual(app.attrib['Id'], 'SonosVolumeBridge')
        self.assertEqual(app.attrib['Executable'], 'speaker-volume-bridge.exe')
        self.assertEqual(manifest.find('.//desktop:StartupTask', ns).attrib['TaskId'], 'SonosVolumeBridgeStartup')

    def test_debian_replaces_old_package_and_retains_command_alias(self):
        deb = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['bundle']['linux']['deb']
        for field in ('conflicts', 'replaces', 'provides'):
            self.assertIn('sonos-volume-bridge', deb[field])
        self.assertEqual(deb['postInstallScript'], '../packaging/linux/postinst')
        self.assertIn('ln -sfn speaker-volume-bridge /usr/bin/sonos-volume-bridge', (ROOT / 'packaging/linux/postinst').read_text())

    def test_installer_registry_identity_is_independent_of_visible_name(self):
        script = (ROOT / 'src-tauri/windows/installer.nsi').read_text()
        self.assertIn('!define LEGACYPRODUCTNAME "Sonos Volume Bridge"', script)
        self.assertIn('Uninstall\\${LEGACYPRODUCTNAME}', script)
        self.assertIn('!define MANUPRODUCTKEY "${MANUKEY}\\${LEGACYPRODUCTNAME}"', script)

if __name__ == '__main__':
    unittest.main()
