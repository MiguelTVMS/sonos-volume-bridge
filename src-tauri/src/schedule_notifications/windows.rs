//! Keep Windows permission checks and delivery on the same notification identity.

pub(super) trait Transport {
    type Error;
    fn setting(&self) -> Permission;
    fn prepare(&self) -> Result<(), Self::Error>;
    fn send(&self, title: &str, body: &str) -> Result<(), Self::Error>;
}

pub(super) enum Permission {
    Known(bool),
    NotRegistered,
    Unavailable,
}

pub(super) struct Notifier<T>(T);

impl<T: Transport> Notifier<T> {
    fn open<E>(
        identifier: &str,
        register: impl FnOnce(&str) -> Result<(), E>,
        create: impl FnOnce(&str) -> Result<T, E>,
    ) -> Result<Self, E> {
        // Registration must precede the first permission query, not just delivery.
        register(identifier)?;
        create(identifier).map(Self)
    }

    pub(super) fn permitted(&self) -> bool {
        match self.0.setting() {
            Permission::Known(enabled) => enabled,
            Permission::NotRegistered => {
                self.0.prepare().is_ok() && matches!(self.0.setting(), Permission::Known(true))
            }
            Permission::Unavailable => false,
        }
    }

    pub(super) fn send(&self, title: &str, body: &str) -> Result<(), T::Error> {
        self.0.send(title, body)
    }
}

#[cfg(windows)]
pub(super) struct Native(windows::UI::Notifications::ToastNotifier, String);

#[cfg(windows)]
impl Transport for Native {
    type Error = windows::core::Error;

    fn setting(&self) -> Permission {
        match self.0.Setting() {
            Ok(setting) => Permission::Known(
                setting == windows::UI::Notifications::NotificationSetting::Enabled,
            ),
            Err(error) if error.code() == windows::core::HRESULT::from_win32(1168) => {
                Permission::NotRegistered
            }
            Err(_) => Permission::Unavailable,
        }
    }

    fn prepare(&self) -> Result<(), Self::Error> {
        use windows::{
            Data::Xml::Dom::XmlDocument,
            Foundation::DateTime,
            UI::Notifications::{ToastNotification, ToastNotificationManager},
            core::{HSTRING, Interface},
        };
        // Windows cannot read Setting until an unpackaged app has sent its first toast.
        // Follow the Windows Community Toolkit's silent preregistration sequence.
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from("<toast><visual><binding template=\"ToastGeneric\"><text>Sonos Volume Bridge</text></binding></visual><audio silent=\"true\"/></toast>"))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        let tag = HSTRING::from("svb-register");
        toast.SetSuppressPopup(true)?;
        toast.SetTag(&tag)?;
        let expires = DateTime {
            UniversalTime: (jiff::Timestamp::now().as_second() + 11_644_473_600 + 15) * 10_000_000,
        };
        let expiration = windows::Foundation::PropertyValue::CreateDateTime(expires)?
            .cast::<windows::Foundation::IReference<DateTime>>()?;
        toast.SetExpirationTime(&expiration)?;
        self.0.Show(&toast)?;
        // The broker registers the sender asynchronously. Keep the first Save from
        // losing its notification while that initial registration is completing.
        for _ in 0..10 {
            if !matches!(self.setting(), Permission::NotRegistered) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.RemoveGroupedTagWithId(&tag, &HSTRING::new(), &HSTRING::from(&self.1));
        }
        Ok(())
    }

    fn send(&self, title: &str, body: &str) -> Result<(), Self::Error> {
        use windows::{
            Data::Xml::Dom::XmlDocument, UI::Notifications::ToastNotification, core::HSTRING,
        };
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from("<toast><visual><binding template=\"ToastGeneric\"><text/><text/></binding></visual></toast>"))?;
        let nodes = xml.GetElementsByTagName(&HSTRING::from("text"))?;
        for (index, value) in [(0, title), (1, body)] {
            // Text nodes safely preserve speaker names containing XML characters.
            nodes
                .Item(index)?
                .AppendChild(&xml.CreateTextNode(&HSTRING::from(value))?)?;
        }
        self.0
            .Show(&ToastNotification::CreateToastNotification(&xml)?)
    }
}

#[cfg(windows)]
pub(super) fn notifier(
    identifier: &str,
    name: &str,
) -> Result<Notifier<Native>, windows::core::Error> {
    use windows::{UI::Notifications::ToastNotificationManager, core::HSTRING};
    if windows::ApplicationModel::Package::Current().is_ok() {
        return ToastNotificationManager::CreateToastNotifier()
            .map(|native| Notifier(Native(native, String::new())));
    }
    Notifier::open(
        identifier,
        |id| {
            // Unpackaged/local runs have no installer to register their notification identity.
            // Register presentation metadata only; never change the user's notification settings.
            let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                .create_subkey(format!("Software\\Classes\\AppUserModelId\\{id}"))?;
            if key.get_value::<String, _>("DisplayName").ok().as_deref() != Some(name) {
                key.set_value("DisplayName", &name)?;
            }
            Ok(())
        },
        |id| {
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))
                .map(|native| Native(native, id.into()))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    #[cfg(windows)]
    #[test]
    #[ignore = "Displays a native Windows toast; run manually on an interactive desktop"]
    fn native_development_notification_smoke() {
        let notifier =
            notifier("ms.miguel.sonosvolumebridge.desktop", "Sonos Volume Bridge").unwrap();
        assert!(
            notifier.permitted(),
            "Windows notifications must be enabled for the development host: {:?}",
            notifier.0.0.Setting()
        );
        notifier
            .send(
                "Sonos Volume Bridge",
                "Windows night schedule notification test.",
            )
            .unwrap();
    }

    struct Fake {
        enabled: bool,
        initialized: Cell<bool>,
        sent: Rc<RefCell<Vec<String>>>,
    }
    impl Transport for Fake {
        type Error = ();
        fn setting(&self) -> Permission {
            if self.initialized.get() {
                Permission::Known(self.enabled)
            } else {
                Permission::NotRegistered
            }
        }
        fn prepare(&self) -> Result<(), ()> {
            self.initialized.set(true);
            Ok(())
        }
        fn send(&self, _: &str, body: &str) -> Result<(), ()> {
            self.sent.borrow_mut().push(body.into());
            Ok(())
        }
    }

    #[test]
    fn local_run_registers_before_permission_and_save_and_boundary_notifications() {
        for identifier in ["normal.app", "normal.app.ui-demo"] {
            let registered = RefCell::new(None);
            let sent = Rc::new(RefCell::new(Vec::new()));
            let notifier = Notifier::open(
                identifier,
                |id| {
                    *registered.borrow_mut() = Some(id.to_owned());
                    Ok(())
                },
                |id| {
                    Ok::<_, ()>(Fake {
                        initialized: Cell::new(false),
                        enabled: registered.borrow().as_deref() == Some(id),
                        sent: sent.clone(),
                    })
                },
            )
            .unwrap();
            // Selecting notifications checks permission before any first delivery.
            assert!(
                notifier.permitted(),
                "local startup must register the same identity before checking permission"
            );
            for notice in ["saved", "scheduled start", "scheduled end"] {
                if notifier.permitted() {
                    notifier.send("Night schedule", notice).unwrap();
                }
            }
            assert_eq!(
                *sent.borrow(),
                ["saved", "scheduled start", "scheduled end"]
            );
        }
    }

    #[test]
    fn installed_app_keeps_its_identity_and_respects_disabled_notifications() {
        for enabled in [false, true] {
            let sent = Rc::new(RefCell::new(Vec::new()));
            let notifier = Notifier::open(
                "installed.app",
                |_| Ok(()),
                |id| {
                    assert_eq!(id, "installed.app");
                    Ok::<_, ()>(Fake {
                        initialized: Cell::new(true),
                        enabled,
                        sent: sent.clone(),
                    })
                },
            )
            .unwrap();
            assert_eq!(notifier.permitted(), enabled);
            if notifier.permitted() {
                notifier.send("Night schedule", "saved").unwrap();
            }
            assert_eq!(sent.borrow().len(), usize::from(enabled));
        }
    }

    #[test]
    fn unavailable_permission_does_not_prime_or_send() {
        struct Unavailable;
        impl Transport for Unavailable {
            type Error = ();
            fn setting(&self) -> Permission {
                Permission::Unavailable
            }
            fn prepare(&self) -> Result<(), ()> {
                panic!("must not prime unrelated failures")
            }
            fn send(&self, _: &str, _: &str) -> Result<(), ()> {
                panic!("must not send")
            }
        }
        let notifier = Notifier::open("app", |_| Ok::<_, ()>(()), |_| Ok(Unavailable)).unwrap();
        assert!(!notifier.permitted());
    }
}
