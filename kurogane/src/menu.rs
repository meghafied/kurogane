//! The default application menu, as data. `platform::macos` turns it into
//! an `NSMenu`; the table itself is testable on every platform.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

/// Modifier keys of a key equivalent. Command is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Modifiers {
    pub shift: bool,
    pub option: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Action {
        title: String,
        /// Objective-C selector sent along the responder chain.
        selector: &'static str,
        /// Key pressed with ⌘, or empty for none.
        key: &'static str,
        modifiers: Modifiers,
    },
    Separator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Menu {
    pub title: String,
    pub items: Vec<Item>,
    /// macOS lists the open windows in this menu.
    pub is_window_menu: bool,
}

/// The App, Edit and Window menus for an application called `app_name`.
///
/// Page shortcuts (⌘S, ⌘N, ⌘O, ⌘T, ⌘P and the like) are deliberately not
/// claimed, so they keep reaching the page.
pub(crate) fn default_menus(app_name: &str) -> Vec<Menu> {
    let action = |title: &str, selector: &'static str, key: &'static str, modifiers: Modifiers| {
        Item::Action {
            title: title.to_owned(),
            selector,
            key,
            modifiers,
        }
    };
    let none = Modifiers::default();
    let shift = Modifiers {
        shift: true,
        option: false,
    };
    let option = Modifiers {
        shift: false,
        option: true,
    };
    let option_shift = Modifiers {
        shift: true,
        option: true,
    };

    vec![
        Menu {
            title: app_name.to_owned(),
            items: vec![
                action(
                    &format!("About {app_name}"),
                    "orderFrontStandardAboutPanel:",
                    "",
                    none,
                ),
                Item::Separator,
                action(&format!("Hide {app_name}"), "hide:", "h", none),
                action("Hide Others", "hideOtherApplications:", "h", option),
                action("Show All", "unhideAllApplications:", "", none),
                Item::Separator,
                action(&format!("Quit {app_name}"), "terminate:", "q", none),
            ],
            is_window_menu: false,
        },
        Menu {
            title: "Edit".to_owned(),
            items: vec![
                action("Undo", "undo:", "z", none),
                action("Redo", "redo:", "z", shift),
                Item::Separator,
                action("Cut", "cut:", "x", none),
                action("Copy", "copy:", "c", none),
                action("Paste", "paste:", "v", none),
                action(
                    "Paste and Match Style",
                    "pasteAndMatchStyle:",
                    "v",
                    option_shift,
                ),
                action("Delete", "delete:", "", none),
                action("Select All", "selectAll:", "a", none),
            ],
            is_window_menu: false,
        },
        Menu {
            title: "Window".to_owned(),
            items: vec![
                action("Minimize", "performMiniaturize:", "m", none),
                action("Zoom", "performZoom:", "", none),
                Item::Separator,
                action("Close", "performClose:", "w", none),
                action("Bring All to Front", "arrangeInFront:", "", none),
            ],
            is_window_menu: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(menus: &[Menu]) -> Vec<(&str, &str, Modifiers)> {
        menus
            .iter()
            .flat_map(|menu| &menu.items)
            .filter_map(|item| match item {
                Item::Action {
                    selector,
                    key,
                    modifiers,
                    ..
                } => Some((*selector, *key, *modifiers)),
                Item::Separator => None,
            })
            .collect()
    }

    #[test]
    fn quit_is_command_q_through_terminate() {
        let menus = default_menus("Notes");
        assert!(actions(&menus).contains(&("terminate:", "q", Modifiers::default())));
    }

    #[test]
    fn edit_shortcuts_reach_the_first_responder() {
        let menus = default_menus("Notes");
        let all = actions(&menus);
        for (selector, key) in [
            ("undo:", "z"),
            ("cut:", "x"),
            ("copy:", "c"),
            ("paste:", "v"),
            ("selectAll:", "a"),
        ] {
            assert!(
                all.contains(&(selector, key, Modifiers::default())),
                "{selector}"
            );
        }
        let shift = Modifiers {
            shift: true,
            option: false,
        };
        assert!(all.contains(&("redo:", "z", shift)));
    }

    #[test]
    fn page_shortcuts_stay_with_the_page() {
        let menus = default_menus("Notes");
        for (_, key, modifiers) in actions(&menus) {
            if modifiers == Modifiers::default() {
                assert!(
                    !["s", "n", "o", "t", "p", "r", "f", "l"].contains(&key),
                    "⌘{key} is claimed"
                );
            }
        }
    }

    #[test]
    fn the_app_name_titles_the_app_menu_items() {
        let menus = default_menus("Notes");
        let titles: Vec<&str> = menus[0]
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Action { title, .. } => Some(title.as_str()),
                Item::Separator => None,
            })
            .collect();
        assert!(titles.contains(&"About Notes"));
        assert!(titles.contains(&"Hide Notes"));
        assert!(titles.contains(&"Quit Notes"));
    }

    #[test]
    fn the_last_menu_is_the_window_menu() {
        let menus = default_menus("Notes");
        assert_eq!(menus.iter().filter(|menu| menu.is_window_menu).count(), 1);
        assert!(menus.last().unwrap().is_window_menu);
    }
}
