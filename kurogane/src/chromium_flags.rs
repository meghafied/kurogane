//! Chromium command-line construction.
//!
//! This module provides a normalized intermediate representation for
//! Chromium command-line switches.

use cef::*;
use std::collections::BTreeMap;

/// User supplied Chromium standalone switches and switches with values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChromiumFlag {
    Present(String),
    WithValue(String, String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SwitchValue {
    Present,
    Value(String),
}

/// Switches whose values are comma-separated feature lists. They accumulate
/// instead of replacing each other.
const FEATURE_LISTS: [&str; 2] = ["disable-features", "enable-features"];

/// Merges comma-separated feature lists: `a,b` and `b,c` give `a,b,c`, in
/// first-seen order.
pub(crate) fn merge_feature_lists(lists: &[&str]) -> String {
    let mut seen = Vec::<&str>::new();
    for list in lists {
        for feature in list.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            if !seen.contains(&feature) {
                seen.push(feature);
            }
        }
    }
    seen.join(",")
}

/// The value to write for `name`, given what the command line already holds.
///
/// CEF puts its own `--disable-features` on the command line before this
/// runs; appending ours would replace that list and turn back on features
/// CEF must keep off.
fn value_for(name: &str, existing: Option<&str>, value: &str) -> String {
    match existing {
        Some(existing) if FEATURE_LISTS.contains(&name) => merge_feature_lists(&[existing, value]),
        _ => value.to_owned(),
    }
}

/// Chromium switch plan with last-write-wins precedence model.
#[derive(Default, Debug)]
pub(crate) struct ChromiumFlags {
    switches: BTreeMap<String, SwitchValue>,
}

impl ChromiumFlags {
    /// Insert a standalone switch.
    pub(crate) fn set(&mut self, name: impl Into<String>) {
        self.switches.insert(name.into(), SwitchValue::Present);
    }

    /// Insert a switch with a value.
    ///
    /// `disable-features` and `enable-features` accumulate: a second value
    /// adds its features to the first instead of replacing them.
    pub(crate) fn set_with_value(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let mut value = value.into();
        if let Some(SwitchValue::Value(existing)) = self.switches.get(&name) {
            value = value_for(&name, Some(existing), &value);
        }
        self.switches.insert(name, SwitchValue::Value(value));
    }

    /// Returns whether a switch is present, with or without a value.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.switches.contains_key(name)
    }

    /// Apply user-supplied Chromium flags.
    ///
    /// User flags are appended after runtime policies and therefore
    /// override runtime defaults when the same switch name is used.
    pub(crate) fn extend_user_flags(&mut self, user_flags: &[ChromiumFlag]) {
        for flag in user_flags {
            match flag {
                ChromiumFlag::Present(name) => self.set(name.clone()),
                ChromiumFlag::WithValue(name, value) => {
                    self.set_with_value(name.clone(), value.clone());
                }
            }
        }
    }

    /// Emit the finalized switch set into CEF.
    ///
    /// This is the only place where ChromiumFlags interacts with
    /// CommandLine directly.
    pub(crate) fn apply(self, cmd: &mut CommandLine) {
        for (name, value) in self.switches {
            let name = CefString::from(name.as_str());

            match value {
                SwitchValue::Present => {
                    cmd.append_switch(Some(&name));
                }
                SwitchValue::Value(value) => {
                    let existing = (cmd.has_switch(Some(&name)) != 0)
                        .then(|| CefString::from(&cmd.switch_value(Some(&name))).to_string());
                    let value = value_for(&name.to_string(), existing.as_deref(), &value);
                    cmd.append_switch_with_value(
                        Some(&name),
                        Some(&CefString::from(value.as_str())),
                    );
                }
            }
        }
    }
}

impl std::fmt::Display for ChromiumFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, value) in &self.switches {
            match value {
                SwitchValue::Present => {
                    writeln!(f, "--{name}")?;
                }

                SwitchValue::Value(v) => {
                    writeln!(f, "--{name}={v}")?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Normalization and precedence tests

    #[test]
    fn duplicate_set_keeps_single_effective_switch() {
        let mut flags = ChromiumFlags::default();

        flags.set("disable-gpu");
        flags.set("disable-gpu");

        assert_eq!(
            flags.switches.get("disable-gpu"),
            Some(&SwitchValue::Present)
        );
    }

    #[test]
    fn last_assignment_wins() {
        let mut flags = ChromiumFlags::default();

        flags.set_with_value("use-gl", "angle");
        flags.set_with_value("use-gl", "egl");

        assert_eq!(
            flags.switches.get("use-gl"),
            Some(&SwitchValue::Value("egl".into()))
        );
    }

    #[test]
    fn feature_lists_merge_without_duplicates() {
        assert_eq!(merge_feature_lists(&["a,b", "b, c", ""]), "a,b,c");
    }

    #[test]
    fn feature_lists_merge_with_the_command_line() {
        assert_eq!(
            value_for(
                "disable-features",
                Some("LensOverlay,GlicActorUi"),
                "AimEnabled"
            ),
            "LensOverlay,GlicActorUi,AimEnabled"
        );
        assert_eq!(value_for("use-gl", Some("angle"), "egl"), "egl");
        assert_eq!(
            value_for("disable-features", None, "AimEnabled"),
            "AimEnabled"
        );
    }

    #[test]
    fn feature_lists_accumulate_across_runtime_and_user_flags() {
        let mut flags = ChromiumFlags::default();
        flags.set_with_value("disable-features", "A,B");
        flags.extend_user_flags(&[ChromiumFlag::WithValue(
            "disable-features".into(),
            "B,C".into(),
        )]);
        assert_eq!(
            flags.switches.get("disable-features"),
            Some(&SwitchValue::Value("A,B,C".into()))
        );
    }

    #[test]
    fn value_replaces_flag() {
        let mut flags = ChromiumFlags::default();

        flags.set("disable-gpu");

        flags.set_with_value("disable-gpu", "ignored");

        assert_eq!(
            flags.switches.get("disable-gpu"),
            Some(&SwitchValue::Value("ignored".into()))
        );
    }

    #[test]
    fn flag_replaces_existing_value() {
        let mut flags = ChromiumFlags::default();

        flags.set_with_value("foo", "bar");
        flags.set("foo");

        assert_eq!(flags.switches.get("foo"), Some(&SwitchValue::Present));
    }

    #[test]
    fn contains_reports_switches_with_and_without_values() {
        let mut flags = ChromiumFlags::default();

        flags.set("no-sandbox");
        flags.set_with_value("use-gl", "egl");

        assert!(flags.contains("no-sandbox"));
        assert!(flags.contains("use-gl"));
        assert!(!flags.contains("disable-gpu"));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn last_write_wins(
            key in "[a-z0-9\\-]{1,32}",
            first in ".*",
            second in ".*",
        ) {
            let mut flags = ChromiumFlags::default();

            flags.set_with_value(
                key.clone(),
                first,
            );

            flags.set_with_value(
                key.clone(),
                second.clone(),
            );

            prop_assert_eq!(
                flags.switches.get(&key),
                Some(&SwitchValue::Value(second))
            );
        }
    }

    proptest! {
        #[test]
        fn user_flags_always_override_runtime_values(
            key in "[a-z0-9\\-]{1,32}",
            runtime in ".*",
            user in ".*",
        ) {
            let mut flags = ChromiumFlags::default();

            flags.set_with_value(key.clone(), runtime);

            flags.extend_user_flags(&[
                ChromiumFlag::WithValue(
                    key.clone(),
                    user.clone(),
                )
            ]);

            prop_assert_eq!(
                flags.switches.get(&key),
                Some(&SwitchValue::Value(user))
            );
        }
    }

    proptest! {
        #[test]
        fn intermediate_assignments_do_not_affect_final_state(
            key in "[a-z0-9\\-]{1,32}",
            a in ".*",
            b in ".*",
            c in ".*",
        ) {
            let mut flags = ChromiumFlags::default();

            flags.set_with_value(key.clone(), a);
            flags.set_with_value(key.clone(), b);
            flags.set_with_value(key.clone(), c.clone());

            prop_assert_eq!(
                flags.switches.get(&key),
                Some(&SwitchValue::Value(c))
            );
        }
    }

    proptest! {
        #[test]
        fn number_of_switches_equals_number_of_unique_keys(
            keys in prop::collection::vec(
                "[a-z0-9\\-]{1,16}",
                0..50
            )
        ) {
            let mut flags = ChromiumFlags::default();

            for key in &keys {
                flags.set(key.clone());
            }

            let unique: std::collections::HashSet<_> =
                keys.iter().collect();

            prop_assert_eq!(
                flags.switches.len(),
                unique.len()
            );
        }
    }
}
