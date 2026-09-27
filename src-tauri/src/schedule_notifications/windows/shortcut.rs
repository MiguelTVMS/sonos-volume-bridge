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
        UI::Shell::{IShellLinkW, PropertiesSystem::IPropertyStore, ShellLink},
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

pub(super) fn ensure(path: &Path, executable: &Path, id: &str) -> windows::core::Result<()> {
    let _apartment = apartment()?;
    // SAFETY: all interfaces and property variants outlive synchronous COM calls.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let file: IPersistFile = link.cast()?;
        let properties: IPropertyStore = link.cast()?;
        let path = HSTRING::from(path.as_os_str());
        if Path::new(&path.to_os_string()).exists() {
            file.Load(&path, STGM_READWRITE)?;
        } else {
            link.SetPath(&HSTRING::from(executable.as_os_str()))?;
        }
        let app_id = PROPVARIANT::from(id);
        let clsid = super::activation::class_id(id);
        let activator = InitPropVariantFromCLSID(&raw const clsid)?;
        properties.SetValue(std::ptr::from_ref(&APP_ID), &raw const app_id)?;
        properties.SetValue(std::ptr::from_ref(&ACTIVATOR), &raw const activator)?;
        properties.Commit()?;
        file.Save(&path, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_shortcut_contains_sender_and_activator_and_preserves_existing_target() {
        let path = std::env::temp_dir().join(format!("svb-toast-{}.lnk", std::process::id()));
        let exe = std::env::current_exe().unwrap();
        ensure(&path, &exe, "normal.app").unwrap();
        let before = std::fs::read(&path).unwrap();
        ensure(
            &path,
            Path::new("C:\\not-the-installed-target.exe"),
            "normal.app",
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
