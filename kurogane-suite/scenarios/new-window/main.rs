//! New-window policy: https links with a click go to the system browser,
//! app:// windows open in the app, everything else is refused. Scripted
//! opens without user activation never reach the policy: Chromium's popup
//! blocker stops them first.

use kurogane::{App, NewWindowAction};

fn main() {
    App::new("scenarios/new-window/frontend")
        // Unsigned builds would otherwise prompt for Keychain access each run
        .credential_storage(kurogane::CredentialStorage::Basic)
        .on_new_window(|request, _app| {
            let action = if request.url.starts_with("app://") {
                NewWindowAction::Allow
            } else if request.url.starts_with("https://") && request.user_gesture {
                NewWindowAction::OpenExternal
            } else {
                NewWindowAction::Deny
            };
            println!(
                "[new-window] {:?} gesture={} {} -> {action:?}",
                request.disposition, request.user_gesture, request.url
            );
            action
        })
        .run_or_exit();
}
