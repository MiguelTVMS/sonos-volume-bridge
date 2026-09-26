//! Unpackaged desktop toast activation, including persistent Notification Center entries.
#![allow(unsafe_code)] // COM callbacks implement the Windows ABI; pointers are checked before use.
#![allow(clippy::ref_as_ptr, clippy::inline_always)] // Generated windows::implement code.
use std::sync::{Arc, OnceLock};
use windows::{
    Win32::{
        Foundation::{CLASS_E_NOAGGREGATION, E_POINTER},
        System::Com::{
            CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, CoInitializeEx, CoRegisterClassObject,
            CoUninitialize, IClassFactory, IClassFactory_Impl, REGCLS_MULTIPLEUSE,
        },
        UI::Notifications::{
            INotificationActivationCallback, INotificationActivationCallback_Impl,
            NOTIFICATION_USER_INPUT_DATA,
        },
    },
    core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, implement},
};

static OPEN_SETTINGS: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
static REGISTRATION: OnceLock<Result<(), HRESULT>> = OnceLock::new();

pub(super) fn set_open_settings(callback: impl Fn() + Send + Sync + 'static) {
    let _ = OPEN_SETTINGS.set(Arc::new(callback));
}

pub(super) fn class_id(identifier: &str) -> GUID {
    #[cfg(test)]
    if identifier.ends_with(".activation-test") {
        return GUID::from_u128(0x3381b991_4b45_47f4_8e06_651caa0d67b4);
    }
    if identifier.ends_with(".ui-demo") {
        GUID::from_u128(0xd9c451c2_f65c_4732_965d_9a7695b2c2ee)
    } else {
        GUID::from_u128(0xa607018c_48b4_45c8_b0b2_46c243fde206)
    }
}

#[implement(INotificationActivationCallback)]
struct Activator;
#[allow(non_snake_case)]
impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        _: &PCWSTR,
        _: &PCWSTR,
        _: *const NOTIFICATION_USER_INPUT_DATA,
        _: u32,
    ) -> windows::core::Result<()> {
        // Toast arguments are deliberately ignored: activation only opens Settings.
        if let Some(open) = OPEN_SETTINGS.get() {
            open();
        }
        Ok(())
    }
}

#[implement(IClassFactory)]
struct Factory;
#[allow(non_snake_case)]
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<'_, IUnknown>,
        iid: *const GUID,
        object: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        if iid.is_null() || object.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: COM supplies writable output storage and a valid requested IID.
        unsafe {
            *object = std::ptr::null_mut();
        }
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let callback: INotificationActivationCallback = Activator.into();
        // SAFETY: QueryInterface transfers one reference to the COM caller.
        unsafe { callback.query(iid, object).ok() }
    }
    fn LockServer(&self, _: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

pub(super) fn register(identifier: &str) -> windows::core::Result<()> {
    REGISTRATION
        .get_or_init(|| {
            let clsid = class_id(identifier);
            let (send, receive) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                // SAFETY: this dedicated MTA owns its COM registration for process lifetime.
                let result = unsafe {
                    CoInitializeEx(None, COINIT_MULTITHREADED)
                        .ok()
                        .and_then(|()| {
                            let factory: IClassFactory = Factory.into();
                            let result = CoRegisterClassObject(
                                &raw const clsid,
                                &factory,
                                CLSCTX_LOCAL_SERVER,
                                REGCLS_MULTIPLEUSE,
                            );
                            if result.is_err() {
                                CoUninitialize();
                            }
                            result
                        })
                };
                let succeeded = result.is_ok();
                let _ = send.send(result.map(|_| ()).map_err(|e| e.code()));
                if succeeded {
                    loop {
                        std::thread::park();
                    }
                }
            });
            receive
                .recv()
                .unwrap_or(Err(windows::Win32::Foundation::E_FAIL))
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn registered_factory_opens_settings_on_native_activation() {
        use windows::Win32::System::Com::CoCreateInstance;
        let opened = Arc::new(AtomicUsize::new(0));
        let observed = opened.clone();
        set_open_settings(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        });
        register("normal.app.activation-test").unwrap();
        register("normal.app.activation-test").unwrap();
        // SAFETY: initialize this test thread, obtain the actual registered COM
        // class, and pass empty activation arguments that production ignores.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            {
                let callback: INotificationActivationCallback = CoCreateInstance(
                    &class_id("normal.app.activation-test"),
                    None,
                    CLSCTX_LOCAL_SERVER,
                )
                .unwrap();
                callback
                    .Activate(PCWSTR::null(), PCWSTR::null(), &[])
                    .unwrap();
            }
            CoUninitialize();
        }
        assert_eq!(opened.load(Ordering::SeqCst), 1);
    }
}
