//! Complete the Windows shell identity for unpackaged runs.
#![allow(unsafe_code)] // Synchronous COM calls with owned strings and balanced initialization.
use std::path::Path;
use windows::{
    Win32::{
        Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile, STGM_READWRITE,
            StructuredStorage::{InitPropVariantFromCLSID, PROPVARIANT},
        },
        UI::Shell::{
            IShellLinkW, PropertiesSystem::IPropertyStore, SHCNE_UPDATEITEM, SHCNF_PATHW,
            SHChangeNotify, ShellLink,
        },
    },
    core::{GUID, HSTRING, Interface},
};
const APP_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};
const ACTIVATOR: PROPERTYKEY = PROPERTYKEY { pid: 26, ..APP_ID };

struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: balance only successful initialization on this thread.
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
fn apartment() -> windows::core::Result<Apartment> {
    // SAFETY: no reserved pointer; an existing STA is usable for these calls.
    let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if result != RPC_E_CHANGED_MODE {
        result.ok()?;
    }
    Ok(Apartment(result.is_ok()))
}

pub(super) fn ensure(
    path: &Path,
    executable: &Path,
    id: &str,
    icon: &Path,
) -> windows::core::Result<()> {
    let installed = installed_executable(id);
    ensure_target(path, executable, id, icon, installed.as_deref())
}

fn installed_executable(id: &str) -> Option<std::path::PathBuf> {
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY},
    };
    if id != "ms.miguel.sonosvolumebridge.desktop" {
        return None;
    }
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(|hive| {
            let key = RegKey::predef(hive)
                .open_subkey_with_flags(
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Sonos Volume Bridge",
                    KEY_READ | KEY_WOW64_64KEY,
                )
                .ok()?;
            let directory: String = key.get_value("InstallLocation").ok()?;
            let executable = Path::new(directory.trim_matches('"')).join("sonos-volume-bridge.exe");
            executable
                .is_file()
                .then(|| dunce::canonicalize(executable).ok())
                .flatten()
        })
}

fn ensure_target(
    path: &Path,
    executable: &Path,
    id: &str,
    icon: &Path,
    installed: Option<&Path>,
) -> windows::core::Result<()> {
    let _apartment = apartment()?;
    // SAFETY: all interfaces and property variants outlive synchronous COM calls.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let file: IPersistFile = link.cast()?;
        let properties: IPropertyStore = link.cast()?;
        let path = HSTRING::from(path.as_os_str());
        if Path::new(&path.to_os_string()).exists() {
            file.Load(&path, STGM_READWRITE)?;
        }
        // A registered installation owns the shortcut even when a development
        // build starts later. Otherwise repair it to this standalone executable.
        link.SetPath(&HSTRING::from(installed.unwrap_or(executable).as_os_str()))?;
        let app_id = PROPVARIANT::from(id);
        link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)?;
        let clsid = super::activation::class_id(id);
        let activator = InitPropVariantFromCLSID(&raw const clsid)?;
        properties.SetValue(std::ptr::from_ref(&APP_ID), &raw const app_id)?;
        properties.SetValue(std::ptr::from_ref(&ACTIVATOR), &raw const activator)?;
        properties.Commit()?;
        file.Save(&path, true)?;
        SHChangeNotify(
            SHCNE_UPDATEITEM,
            SHCNF_PATHW,
            Some(path.as_ptr().cast()),
            None,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_repairs_development_shortcut_and_keeps_installation_owner() {
        let path = std::env::temp_dir().join(format!("svb-upgrade-{}.lnk", std::process::id()));
        let development = Path::new("C:\\development\\sonos-volume-bridge.exe");
        let installed = std::env::current_exe().unwrap();
        ensure_target(&path, development, "normal.app", &installed, None).unwrap();
        ensure_target(
            &path,
            &installed,
            "normal.app",
            &installed,
            Some(&installed),
        )
        .unwrap();
        ensure_target(
            &path,
            development,
            "normal.app",
            &installed,
            Some(&installed),
        )
        .unwrap();
        let _apartment = apartment().unwrap();
        // SAFETY: load the real persisted shell link into owned buffers.
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let file: IPersistFile = link.cast().unwrap();
            file.Load(
                &HSTRING::from(path.as_os_str()),
                windows::Win32::System::Com::STGM_READ,
            )
            .unwrap();
            let mut target = vec![0_u16; 32768];
            link.GetPath(&mut target, std::ptr::null_mut(), 0).unwrap();
            let end = target.iter().position(|unit| *unit == 0).unwrap();
            assert_eq!(
                String::from_utf16(&target[..end]).unwrap(),
                installed.to_string_lossy()
            );
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn startup_shortcut_contains_sender_and_activator_and_preserves_existing_target() {
        let path = std::env::temp_dir().join(format!("svb-toast-{}.lnk", std::process::id()));
        let exe = std::env::current_exe().unwrap();
        ensure_target(&path, &exe, "normal.app", &exe, Some(&exe)).unwrap();
        let before = std::fs::read(&path).unwrap();
        ensure_target(
            &path,
            Path::new("C:\\not-the-installed-target.exe"),
            "normal.app",
            &exe,
            Some(&exe),
        )
        .unwrap();
        let _apartment = apartment().unwrap();
        // SAFETY: read back the persisted shortcut through the shell's property store.
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let file: IPersistFile = link.cast().unwrap();
            file.Load(
                &HSTRING::from(path.as_os_str()),
                windows::Win32::System::Com::STGM_READ,
            )
            .unwrap();
            let properties: IPropertyStore = link.cast().unwrap();
            let mut icon_path = vec![0_u16; 32768];
            let mut icon_index = -1;
            link.GetIconLocation(&mut icon_path, &raw mut icon_index)
                .unwrap();
            let end = icon_path.iter().position(|unit| *unit == 0).unwrap();
            assert_eq!(
                String::from_utf16(&icon_path[..end]).unwrap(),
                exe.to_string_lossy()
            );
            assert_eq!(icon_index, 0);
            assert_eq!(
                properties.GetValue(std::ptr::from_ref(&APP_ID)).unwrap(),
                PROPVARIANT::from("normal.app")
            );
            let clsid = super::super::activation::class_id("normal.app");
            assert_eq!(
                properties.GetValue(std::ptr::from_ref(&ACTIVATOR)).unwrap(),
                InitPropVariantFromCLSID(&raw const clsid).unwrap()
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
}
