//! Chromium settings that stop background calls to Google.
//!
//! Chrome-style CEF keeps several browser services alive that talk to
//! Google without any page asking: AI-mode eligibility checks,
//! search-engine preconnects, network time queries and account
//! reconciliation.
//!
//! Account reconciliation (`accounts.google.com/ListAccounts`) is not
//! covered: turning it off means disallowing sign-in (`signin.allowed`),
//! and a profile saved with sign-in disallowed crashes CEF 150's
//! Chrome-style browser creation on the next launch.

/// Features switched off with `--disable-features`.
pub(crate) const DISABLED_FEATURES: &[&str] = &[
    // www.google.com/async/folae eligibility requests
    "AimEnabled",
    "AimServerEligibilityEnabled",
    "AimServerRequestOnStartupEnabled",
    // Preconnects to the default search engine
    "PreconnectToSearch",
    "PreconnectFromKeyedService",
    // Secure time queries to clients2.google.com/time
    "NetworkTimeServiceQuerying",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quiet_features_cover_the_observed_google_calls() {
        for feature in [
            "AimServerEligibilityEnabled",
            "PreconnectToSearch",
            "NetworkTimeServiceQuerying",
        ] {
            assert!(DISABLED_FEATURES.contains(&feature));
        }
    }
}
