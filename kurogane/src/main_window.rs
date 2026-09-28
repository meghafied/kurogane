//! The window the application opens at startup.

use cef::Rect;

use crate::error::ConfigError;

/// How the application's first window opens.
///
/// Sizes are in density-independent pixels. Pass to
/// [`App::main_window`](crate::App::main_window).
///
/// ```no_run
/// use kurogane::{App, MainWindow};
///
/// App::new("dist")
///     .main_window(MainWindow::new().title("Notes").size(1280, 860).min_size(960, 640))
///     .run_or_exit();
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MainWindow {
    pub(crate) title: Option<String>,
    pub(crate) size: Option<(i32, i32)>,
    pub(crate) min_size: Option<(i32, i32)>,
}

impl MainWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the window title. The page's `<title>` does not change it.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Opens the window at this size, centered on the primary display's work
    /// area and shrunk to fit it.
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Keeps the user from resizing the window below this size.
    pub fn min_size(mut self, width: i32, height: i32) -> Self {
        self.min_size = Some((width, height));
        self
    }

    /// Builder problems, reported by `App::build`.
    pub(crate) fn problems(&self) -> Vec<ConfigError> {
        let positive = |size: Option<(i32, i32)>| size.is_none_or(|(w, h)| w > 0 && h > 0);
        let mut problems = Vec::new();
        if !positive(self.size) {
            problems.push(ConfigError::InvalidWindowSize("the size must be positive"));
        }
        if !positive(self.min_size) {
            problems.push(ConfigError::InvalidWindowSize(
                "the minimum size must be positive",
            ));
        }
        if let (Some((w, h)), Some((min_w, min_h))) = (self.size, self.min_size)
            && (min_w > w || min_h > h)
        {
            problems.push(ConfigError::InvalidWindowSize(
                "the minimum size is larger than the size",
            ));
        }
        problems
    }
}

/// Centers a `width`×`height` window in `work_area`, shrinking it to fit.
pub(crate) fn centered_bounds(work_area: &Rect, width: i32, height: i32) -> Rect {
    let width = width.min(work_area.width);
    let height = height.min(work_area.height);
    Rect {
        x: work_area.x + (work_area.width - width) / 2,
        y: work_area.y + (work_area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_window_is_centered_in_the_work_area() {
        let bounds = centered_bounds(&area(0, 25, 1920, 1055), 1280, 860);
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (320, 122, 1280, 860)
        );
    }

    #[test]
    fn a_window_larger_than_the_work_area_is_shrunk_to_fit() {
        let bounds = centered_bounds(&area(0, 25, 1024, 743), 1280, 860);
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (0, 25, 1024, 743)
        );
    }

    #[test]
    fn a_secondary_display_origin_is_respected() {
        let bounds = centered_bounds(&area(-1440, 0, 1440, 900), 1000, 600);
        assert_eq!((bounds.x, bounds.y), (-1220, 150));
    }

    #[test]
    fn valid_options_have_no_problems() {
        let window = MainWindow::new()
            .title("A")
            .size(1280, 860)
            .min_size(960, 640);
        assert!(window.problems().is_empty());
    }

    #[test]
    fn a_zero_or_negative_size_is_a_problem() {
        assert_eq!(MainWindow::new().size(0, 600).problems().len(), 1);
        assert_eq!(MainWindow::new().min_size(800, -1).problems().len(), 1);
    }

    #[test]
    fn a_minimum_larger_than_the_size_is_a_problem() {
        let window = MainWindow::new().size(800, 600).min_size(1024, 600);
        assert_eq!(
            window.problems(),
            vec![ConfigError::InvalidWindowSize(
                "the minimum size is larger than the size"
            )]
        );
    }
}
