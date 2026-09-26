//! Read the desktop clock preference without changing system settings.
#[cfg(target_os = "macos")]
pub fn hour12() -> Option<bool> {
    use objc2_foundation::{NSDateFormatter, NSLocale, NSString};
    let locale = NSLocale::autoupdatingCurrentLocale();
    let pattern = NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("j"),
        0,
        Some(&locale),
    )?;
    pattern_hour12(&pattern.to_string())
}

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 fills an owned UTF-16 buffer; null locale means user defaults.
pub fn hour12() -> Option<bool> {
    use windows::{
        Win32::Globalization::{GetLocaleInfoEx, LOCALE_STIMEFORMAT},
        core::PCWSTR,
    };
    let mut buffer = [0_u16; 128];
    // SAFETY: the API receives a valid writable slice, including space for its terminator.
    let count = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_STIMEFORMAT, Some(&mut buffer)) };
    if count <= 1 {
        return None;
    }
    pattern_hour12(&String::from_utf16_lossy(
        &buffer[..usize::try_from(count).ok()? - 1],
    ))
}

#[cfg(target_os = "linux")]
pub fn hour12() -> Option<bool> {
    use std::process::Command;
    // GNOME's explicit desktop preference takes precedence over LC_TIME.
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .contains("gnome")
    {
        let preference = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "clock-format"])
            .output()
            .ok();
        if let Some(output) = preference.filter(|output| output.status.success()) {
            match String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
            {
                "12h" => return Some(true),
                "24h" => return Some(false),
                _ => {}
            }
        }
    }
    let output = Command::new("locale")
        .args(["-k", "LC_TIME"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let pattern = text.lines().find_map(|line| line.strip_prefix("t_fmt="))?;
    posix_hour12(pattern)
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
pub fn hour12() -> Option<bool> {
    None
}

#[cfg(any(target_os = "macos", windows, test))]
fn pattern_hour12(pattern: &str) -> Option<bool> {
    let mut quoted = false;
    for ch in pattern.chars() {
        match ch {
            '\'' => quoted = !quoted,
            'H' | 'k' if !quoted => return Some(false),
            'h' | 'K' if !quoted => return Some(true),
            _ => {}
        }
    }
    None
}
#[cfg(any(target_os = "linux", test))]
fn posix_hour12(pattern: &str) -> Option<bool> {
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            continue;
        }
        let mut token = chars.next()?;
        if token == 'E' || token == 'O' {
            token = chars.next()?;
        }
        match token {
            'H' | 'k' | 'R' | 'T' => return Some(false),
            'I' | 'l' | 'r' => return Some(true),
            _ => {}
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_patterns_preserve_clock_overrides() {
        assert_eq!(pattern_hour12("HH:mm:ss"), Some(false));
        assert_eq!(pattern_hour12("h:mm tt"), Some(true));
        assert_eq!(pattern_hour12("'hours' HH:mm"), Some(false));
        assert_eq!(posix_hour12("%r"), Some(true));
        assert_eq!(posix_hour12("%H:%M:%S"), Some(false));
        assert_eq!(posix_hour12("%OI:%M %p"), Some(true));
        assert_eq!(posix_hour12("%%H %I:%M"), Some(true));
    }
}
