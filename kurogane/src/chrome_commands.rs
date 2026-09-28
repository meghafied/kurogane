//! CEF command filtering for Kurogane.
//!
//! A Kurogane window omits standard browser UI (tab strips, toolbars, app menus).
//! However, the underlying CEF framework still processes default Chrome shortcuts
//! (e.g., Ctrl+N, Ctrl+T) and context menus.
//!
//! To prevent CEF from spawning unmanaged native windows or tabs, this module intercepts
//! the command pipeline. Kurogane uses a strict allowlist: commands execute ONLY if they
//! operate on the current page context. All other commands are intentionally swallowed,
//! except the quit command (⌘Q on macOS), which closes the application in order.

use std::ffi::{CStr, c_char, c_int};
use std::sync::{Arc, OnceLock};

use cef::*;

use crate::debug;
use crate::runtime::RuntimeServices;

/// Strict allowlist of page-local commands, by their names in `cef_command_ids.h`.
const ALLOWED: &[&CStr] = &[
    // Navigation within the page
    c"IDC_BACK",
    c"IDC_FORWARD",
    c"IDC_RELOAD",
    c"IDC_RELOAD_BYPASSING_CACHE",
    c"IDC_RELOAD_CLEARING_CACHE",
    c"IDC_STOP",
    // Zoom, fullscreen, find and print
    c"IDC_ZOOM_PLUS",
    c"IDC_ZOOM_NORMAL",
    c"IDC_ZOOM_MINUS",
    c"IDC_FULLSCREEN",
    c"IDC_FIND",
    c"IDC_FIND_NEXT",
    c"IDC_FIND_PREVIOUS",
    c"IDC_PRINT",
    // Editing, from shortcuts and from the context menu
    c"IDC_CUT",
    c"IDC_COPY",
    c"IDC_PASTE",
    c"IDC_CONTENT_CONTEXT_CUT",
    c"IDC_CONTENT_CONTEXT_COPY",
    c"IDC_CONTENT_CONTEXT_PASTE",
    c"IDC_CONTENT_CONTEXT_PASTE_AND_MATCH_STYLE",
    c"IDC_CONTENT_CONTEXT_DELETE",
    c"IDC_CONTENT_CONTEXT_SELECTALL",
    c"IDC_CONTENT_CONTEXT_UNDO",
    c"IDC_CONTENT_CONTEXT_REDO",
    // Closing the window the command came from
    c"IDC_CLOSE_TAB",
    c"IDC_CLOSE_WINDOW",
    // Developer tools, which open as Kurogane popups
    c"IDC_DEV_TOOLS",
    c"IDC_DEV_TOOLS_CONSOLE",
    c"IDC_DEV_TOOLS_INSPECT",
    c"IDC_DEV_TOOLS_TOGGLE",
    c"IDC_CONTENT_CONTEXT_INSPECTELEMENT",
];

unsafe extern "C" {
    /// Resolves a string command name to its dynamic CEF command ID.
    /// Returns `-1` if the command is absent in the current CEF build.
    fn cef_id_for_command_id_name(name: *const c_char) -> c_int;
}

/// Resolves a stable command name to its dynamic runtime ID.
fn command_id(name: &CStr) -> Option<c_int> {
    // SAFETY: `name` is a valid, null-terminated C-string. The FFI boundary
    // guarantees read-only access and the backing memory outlives the call.
    let id = unsafe { cef_id_for_command_id_name(name.as_ptr()) };
    (id >= 0).then_some(id)
}

/// Translates [`ALLOWED`] command names to runtime IDs.
/// Cached via [`OnceLock`] because CEF command IDs are unstable across Chromium
/// versions, whereas string names provide a stable resolution ABI.
fn allowed_ids() -> &'static [c_int] {
    static IDS: OnceLock<Vec<c_int>> = OnceLock::new();
    IDS.get_or_init(|| ALLOWED.iter().filter_map(|name| command_id(name)).collect())
}

/// Resolves the dynamic ID range reserved for application-defined context menu items.
fn custom_context_ids() -> Option<(c_int, c_int)> {
    static RANGE: OnceLock<Option<(c_int, c_int)>> = OnceLock::new();
    *RANGE.get_or_init(|| {
        Some((
            command_id(c"IDC_CONTENT_CONTEXT_CUSTOM_FIRST")?,
            command_id(c"IDC_CONTENT_CONTEXT_CUSTOM_LAST")?,
        ))
    })
}

/// The runtime ID of Chrome's quit command, `IDC_EXIT`.
fn exit_id() -> Option<c_int> {
    static ID: OnceLock<Option<c_int>> = OnceLock::new();
    *ID.get_or_init(|| command_id(c"IDC_EXIT"))
}

/// What a Kurogane window does with a Chrome command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Let CEF run it.
    Run,
    /// Close the application in order, as its `terminate:` does.
    Quit,
    /// Swallow it.
    Refuse,
}

/// Decides for command `id`, given the resolved IDs of the allowlist, the
/// custom context menu range and the quit command.
fn verdict(
    id: c_int,
    disposition: WindowOpenDisposition,
    allowed: &[c_int],
    custom: Option<(c_int, c_int)>,
    exit: Option<c_int>,
) -> Verdict {
    if exit == Some(id) {
        return Verdict::Quit;
    }
    let page_local =
        allowed.contains(&id) || custom.is_some_and(|(first, last)| (first..=last).contains(&id));
    if disposition == WindowOpenDisposition::CURRENT_TAB && page_local {
        Verdict::Run
    } else {
        Verdict::Refuse
    }
}

wrap_command_handler! {
    pub struct KuroganeCommandHandler {
        services: Arc<RuntimeServices>,
    }

    impl CommandHandler {
        fn on_chrome_command(
            &self,
            _browser: Option<&mut Browser>,
            command_id: c_int,
            disposition: WindowOpenDisposition,
        ) -> c_int {
            match verdict(command_id, disposition, allowed_ids(), custom_context_ids(), exit_id()) {
                // False. Unhandled by wrapper, proceed with default CEF execution.
                Verdict::Run => 0,
                Verdict::Quit => {
                    // Chrome's own exit path would bypass Kurogane's shutdown
                    debug!("[Commands] quit");
                    crate::runtime::close_all_browsers_and_windows(
                        &self.services.browser_registry,
                        &self.services.window_registry,
                    );
                    1
                }
                Verdict::Refuse => {
                    debug!("[Commands] refused Chrome command {command_id} ({disposition:?})");
                    // True. Handled by wrapper. Swallows the command so CEF drops it.
                    1
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPY: c_int = 10;
    const NEW_TAB: c_int = 11;
    const EXIT: c_int = 12;

    fn decide(id: c_int, disposition: WindowOpenDisposition) -> Verdict {
        verdict(id, disposition, &[COPY], Some((100, 199)), Some(EXIT))
    }

    #[test]
    fn page_local_commands_run() {
        assert_eq!(
            decide(COPY, WindowOpenDisposition::CURRENT_TAB),
            Verdict::Run
        );
        assert_eq!(
            decide(150, WindowOpenDisposition::CURRENT_TAB),
            Verdict::Run
        );
    }

    #[test]
    fn everything_else_is_refused() {
        assert_eq!(
            decide(NEW_TAB, WindowOpenDisposition::CURRENT_TAB),
            Verdict::Refuse
        );
        assert_eq!(
            decide(COPY, WindowOpenDisposition::NEW_FOREGROUND_TAB),
            Verdict::Refuse
        );
    }

    #[test]
    fn quit_closes_the_application() {
        assert_eq!(
            decide(EXIT, WindowOpenDisposition::CURRENT_TAB),
            Verdict::Quit
        );
    }
}
