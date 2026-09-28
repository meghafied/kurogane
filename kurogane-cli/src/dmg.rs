//! Creates macOS DMG disk images.
//!
//! The image holds the app and an `Applications` link, the familiar
//! drag-to-install pair.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::tui;

/// Returns the DMG path for an application name.
fn dmg_path(output_dir: &Path, name: &str) -> PathBuf {
    output_dir.join(format!("{name}.dmg"))
}

/// Returns the folder the image contents are staged in, beside the DMG.
fn staging_dir(output_dir: &Path, name: &str) -> PathBuf {
    output_dir.join(format!(".{name}-dmg"))
}

/// Stages the image contents: a copy of the app and an `Applications` link.
fn stage(app_dir: &Path, staging: &Path) -> Result<()> {
    if staging.exists() {
        fs::remove_dir_all(staging)
            .with_context(|| format!("failed to remove {}", staging.display()))?;
    }
    fs::create_dir_all(staging)
        .with_context(|| format!("failed to create {}", staging.display()))?;

    let app_name = app_dir
        .file_name()
        .with_context(|| format!("{} has no file name", app_dir.display()))?;
    let copy = staging.join(app_name);
    // On APFS the copy is a clone and takes no space; elsewhere it is a copy
    let copied = |flags: &str| -> Result<bool> {
        Ok(Command::new("cp")
            .arg(flags)
            .arg(app_dir)
            .arg(&copy)
            .status()
            .context("failed to run cp")?
            .success())
    };
    if !copied("-Rc")? {
        let _ = fs::remove_dir_all(&copy);
        if !copied("-R")? {
            bail!(
                "failed to copy {} into the DMG staging folder",
                app_dir.display()
            );
        }
    }

    std::os::unix::fs::symlink("/Applications", staging.join("Applications"))
        .context("failed to create the Applications link")?;
    Ok(())
}

/// Creates a compressed DMG containing the application and an
/// `Applications` link.
pub fn build(app_dir: &Path, output_dir: &Path, name: &str) -> Result<PathBuf> {
    let dmg_path = dmg_path(output_dir, name);

    if dmg_path.exists() {
        fs::remove_file(&dmg_path)
            .with_context(|| format!("failed to remove {}", dmg_path.display()))?;
    }

    let staging = staging_dir(output_dir, name);
    stage(app_dir, &staging)?;

    let status = Command::new("hdiutil")
        .arg("create")
        .arg("-volname")
        .arg(name)
        .arg("-srcfolder")
        .arg(&staging)
        .arg("-ov")
        .arg("-format")
        .arg("UDZO")
        .arg(&dmg_path)
        .status();
    let _ = fs::remove_dir_all(&staging);

    if !status?.success() {
        bail!("hdiutil create failed; macOS tools are required to build a DMG");
    }

    tui::field("dmg", tui::format_path(&dmg_path));

    Ok(dmg_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmg_is_named_after_the_app_and_written_beside_it() {
        assert_eq!(
            dmg_path(Path::new("/proj/dist"), "MyApp"),
            Path::new("/proj/dist/MyApp.dmg")
        );
        assert_eq!(
            staging_dir(Path::new("/proj/dist"), "MyApp"),
            Path::new("/proj/dist/.MyApp-dmg")
        );
    }

    #[test]
    fn staging_holds_the_app_and_an_applications_link() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("MyApp.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::write(app.join("Contents/MacOS/myapp"), b"exe").unwrap();
        let staging = dir.path().join(".MyApp-dmg");
        fs::create_dir_all(staging.join("leftover")).unwrap();

        stage(&app, &staging).unwrap();

        assert_eq!(
            fs::read(staging.join("MyApp.app/Contents/MacOS/myapp")).unwrap(),
            b"exe"
        );
        let link = staging.join("Applications");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), Path::new("/Applications"));
        assert!(!staging.join("leftover").exists());
    }
}
