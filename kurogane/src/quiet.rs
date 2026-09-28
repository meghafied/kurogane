//! Chromium settings that stop background calls to Google.
//!
//! Chrome-style CEF keeps several browser services alive that talk to
//! Google without any page asking: AI-mode eligibility checks,
//! search-engine preconnects, network time queries, component update
//! checks, Safe Browsing list updates and account reconciliation.
//!
//! Safe Browsing is turned off entirely: Chromium 150 has no switch that
//! stops only its list updates. That removes its warnings for pages loaded
//! from the web, so it suits apps that show their own content.
//!
//! Account reconciliation (`accounts.google.com/ListAccounts`) is not
//! covered: turning it off means disallowing sign-in (`signin.allowed`),
//! and a profile saved with sign-in disallowed crashes CEF 150's
//! Chrome-style browser creation on the next launch.

use crate::chromium_flags::ChromiumFlags;

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

/// Boolean preferences switched off on the global request context.
pub(crate) const DISABLED_PREFERENCES: &[&str] = &[
    // Safe Browsing list updates to safebrowsing.googleapis.com, a few
    // minutes after launch
    "safebrowsing.enabled",
];

/// Adds the quiet settings to `flags`.
pub(crate) fn apply(flags: &mut ChromiumFlags) {
    flags.set_with_value("disable-features", DISABLED_FEATURES.join(","));
    // Component update checks to update.googleapis.com, first about 60 s
    // after launch
    flags.set("disable-component-update");
}

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
        assert!(DISABLED_PREFERENCES.contains(&"safebrowsing.enabled"));
    }

    #[test]
    fn quiet_flags_also_stop_component_updates() {
        let mut flags = ChromiumFlags::default();
        apply(&mut flags);
        assert!(flags.contains("disable-features"));
        assert!(flags.contains("disable-component-update"));
    }
}
