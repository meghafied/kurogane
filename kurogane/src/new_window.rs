//! What happens when a page opens a new window: `target="_blank"`,
//! `window.open`, or a link opened with a modifier key.

use std::sync::Arc;

use cef::WindowOpenDisposition;

/// A page's request to open a new window, passed to the closure given to
/// [`App::on_new_window`](crate::App::on_new_window).
///
/// `Default` lets applications build requests in their own policy tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct NewWindowRequest {
    /// The URL the new window would load.
    pub url: String,
    /// The URL of the frame that asked.
    pub opener_url: String,
    /// How the page asked for it.
    pub disposition: NewWindowDisposition,
    /// Whether a click or key press started it.
    pub user_gesture: bool,
}

/// How a page asked for a new window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum NewWindowDisposition {
    ForegroundTab,
    BackgroundTab,
    Popup,
    Window,
    #[default]
    Other,
}

/// What to do with a [`NewWindowRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewWindowAction {
    /// Open an application window, as without a policy.
    Allow,
    /// Do nothing.
    Deny,
    /// Hand the URL to the system's default browser.
    OpenExternal,
}

/// What [`App::on_new_window`](crate::App::on_new_window) stores; the closure,
/// with the handle bound.
pub(crate) type NewWindowHandler = Arc<dyn Fn(&NewWindowRequest) -> NewWindowAction + Send + Sync>;

impl From<WindowOpenDisposition> for NewWindowDisposition {
    fn from(disposition: WindowOpenDisposition) -> Self {
        match disposition {
            WindowOpenDisposition::NEW_FOREGROUND_TAB => Self::ForegroundTab,
            WindowOpenDisposition::NEW_BACKGROUND_TAB => Self::BackgroundTab,
            WindowOpenDisposition::NEW_POPUP => Self::Popup,
            WindowOpenDisposition::NEW_WINDOW => Self::Window,
            _ => Self::Other,
        }
    }
}

/// Whether CEF should cancel the popup, and whether its URL goes to the
/// system browser.
pub(crate) fn popup_outcome(action: NewWindowAction) -> (bool, bool) {
    match action {
        NewWindowAction::Allow => (false, false),
        NewWindowAction::Deny => (true, false),
        NewWindowAction::OpenExternal => (true, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_keeps_the_popup() {
        assert_eq!(popup_outcome(NewWindowAction::Allow), (false, false));
    }

    #[test]
    fn deny_cancels_the_popup() {
        assert_eq!(popup_outcome(NewWindowAction::Deny), (true, false));
    }

    #[test]
    fn open_external_cancels_and_hands_off() {
        assert_eq!(popup_outcome(NewWindowAction::OpenExternal), (true, true));
    }

    #[test]
    fn dispositions_map_to_the_public_kinds() {
        use NewWindowDisposition as D;
        assert_eq!(
            D::from(WindowOpenDisposition::NEW_FOREGROUND_TAB),
            D::ForegroundTab
        );
        assert_eq!(
            D::from(WindowOpenDisposition::NEW_BACKGROUND_TAB),
            D::BackgroundTab
        );
        assert_eq!(D::from(WindowOpenDisposition::NEW_POPUP), D::Popup);
        assert_eq!(D::from(WindowOpenDisposition::NEW_WINDOW), D::Window);
        assert_eq!(D::from(WindowOpenDisposition::SAVE_TO_DISK), D::Other);
    }
}
