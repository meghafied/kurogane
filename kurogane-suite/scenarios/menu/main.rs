//! The default macOS menu: ⌘Q quits, Edit shortcuts work in text fields,
//! page shortcuts such as ⌘S still reach the page.

fn main() {
    kurogane::App::new("scenarios/menu/frontend")
        // Unsigned builds would otherwise prompt for Keychain access each run
        .credential_storage(kurogane::CredentialStorage::Basic)
        .run_or_exit();
    println!("[menu] run() returned; the app quit in order");
}
