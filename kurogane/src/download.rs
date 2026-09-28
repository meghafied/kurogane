//! What happens to downloads: `<a download>`, `Content-Disposition:
//! attachment`, or a navigation Chromium cannot display.

use cef::*;

/// Download behaviour, set with [`App::downloads`](crate::App::downloads).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Downloads {
    /// Chromium decides. Chrome-style windows save to the Downloads folder
    /// without asking.
    #[default]
    Chromium,
    /// Refuse every download.
    Deny,
    /// Ask where to save each download with the system's Save panel.
    Prompt,
}

/// `(may start, show Save As)` for a policy.
pub(crate) fn verdict(policy: Downloads) -> (bool, bool) {
    match policy {
        Downloads::Deny => (false, false),
        Downloads::Prompt => (true, true),
        Downloads::Chromium => (true, false),
    }
}

wrap_download_handler! {
    pub struct KuroganeDownloadHandler {
        policy: Downloads,
    }

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            // cef-rs defaults to 0, which would cancel every download
            verdict(self.policy).0 as ::std::os::raw::c_int
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            _download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            let (_, prompt) = verdict(self.policy);
            let Some(callback) = callback.filter(|_| prompt) else {
                return 0;
            };
            crate::debug!(
                "[Download] save as {:?}",
                suggested_name.map(|name| name.to_string())
            );
            // An empty path suggests the page's file name in the Save panel
            callback.cont(Some(&CefString::from("")), 1);
            1
        }
    }
}

/// The handler a client installs, or none to leave downloads to Chromium.
pub(crate) fn handler(policy: Downloads) -> Option<DownloadHandler> {
    (policy != Downloads::Chromium).then(|| KuroganeDownloadHandler::new(policy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_allows_and_asks() {
        assert_eq!(verdict(Downloads::Prompt), (true, true));
    }

    #[test]
    fn deny_refuses() {
        assert_eq!(verdict(Downloads::Deny), (false, false));
    }

    #[test]
    fn chromium_installs_no_handler() {
        assert!(handler(Downloads::Chromium).is_none());
        assert!(handler(Downloads::Prompt).is_some());
    }
}
