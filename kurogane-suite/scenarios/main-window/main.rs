//! The main window opens centered at 1100×700, cannot shrink below 800×500,
//! and keeps its title while the page keeps changing `document.title`.

use kurogane::{App, MainWindow};

fn main() {
    App::new("scenarios/main-window/frontend")
        // Unsigned builds would otherwise prompt for Keychain access each run
        .credential_storage(kurogane::CredentialStorage::Basic)
        .main_window(
            MainWindow::new()
                .title("Main window scenario")
                .size(1100, 700)
                .min_size(800, 500),
        )
        .run_or_exit();
}
