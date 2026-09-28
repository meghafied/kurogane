//! Opening URLs in the user's default browser.

use std::fmt::{Display, Formatter};

/// Why [`open_external`] refused or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenExternalError {
    /// Only `http`, `https` and `mailto` URLs are handed to the system.
    UnsupportedUrl(String),
    /// The system could not open the URL.
    Failed(String),
}

impl Display for OpenExternalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedUrl(url) => write!(f, "not an http, https or mailto URL: {url}"),
            Self::Failed(reason) => write!(f, "could not open the URL: {reason}"),
        }
    }
}

impl std::error::Error for OpenExternalError {}

/// Checks that `url` is one the system browser should receive.
pub(crate) fn validate(url: &str) -> Result<url::Url, OpenExternalError> {
    match url::Url::parse(url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https" | "mailto") => Ok(parsed),
        _ => Err(OpenExternalError::UnsupportedUrl(url.to_owned())),
    }
}

/// Opens `url` in the system's default browser (or mail client for
/// `mailto`). Only `http`, `https` and `mailto` URLs are accepted.
pub fn open_external(url: &str) -> Result<(), OpenExternalError> {
    let url = validate(url)?;
    platform::open(url.as_str()).map_err(OpenExternalError::Failed)
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    pub(super) fn open(url: &str) -> Result<(), String> {
        let string = NSString::from_str(url);
        let url = NSURL::URLWithString(&string).ok_or_else(|| "NSURL rejected it".to_owned())?;
        if NSWorkspace::sharedWorkspace().openURL(&url) {
            Ok(())
        } else {
            Err("NSWorkspace declined it".to_owned())
        }
    }
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    pub(super) fn open(url: &str) -> Result<(), String> {
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (verb, target) = (wide("open"), wide(url));
        // SAFETY: both strings are NUL-terminated and outlive the call
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // Values above 32 mean success
        if result as isize > 32 {
            Ok(())
        } else {
            Err(format!("ShellExecuteW returned {}", result as isize))
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    pub(super) fn open(url: &str) -> Result<(), String> {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_urls_are_accepted() {
        for url in [
            "https://example.com/a?b=1",
            "http://example.com",
            "mailto:hi@example.com",
        ] {
            assert!(validate(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn everything_else_is_refused() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "app://app/index.html",
            "myapp://open",
            "not a url",
            "",
        ] {
            assert!(
                matches!(validate(url), Err(OpenExternalError::UnsupportedUrl(_))),
                "{url}"
            );
        }
    }
}
