//! Downloads::Prompt: every download opens the Save panel.

use kurogane::{App, Downloads};

fn main() {
    App::new("scenarios/downloads/frontend")
        // Unsigned builds would otherwise prompt for Keychain access each run
        .credential_storage(kurogane::CredentialStorage::Basic)
        .downloads(Downloads::Prompt)
        .run_or_exit();
}
