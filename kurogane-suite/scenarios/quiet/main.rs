//! Quiet network: run with --log-net-log=/tmp/quiet.json, wait at least five
//! minutes (component update checks start about a minute after launch,
//! Safe Browsing updates a few minutes later), quit, then list the hosts
//! Chromium contacted. With quiet_network the page, which
//! makes no requests, should leave no Google hosts in the log.
//! KUROGANE_LOUD=1 runs without it for comparison.

fn main() {
    kurogane::App::new("scenarios/quiet/frontend")
        // Unsigned builds would otherwise prompt for Keychain access each run
        .credential_storage(kurogane::CredentialStorage::Basic)
        .quiet_network(std::env::var_os("KUROGANE_LOUD").is_none())
        .run_or_exit();
}
