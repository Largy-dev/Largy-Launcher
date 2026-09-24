//! Desktop shortcuts that start the launcher straight into an instance:
//! `LargyLauncher.exe --launch <instance id>`.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

pub const LAUNCH_FLAG: &str = "--launch";

/// Instance id passed with `--launch <id>` (or `--launch=<id>`), if any.
pub fn launch_arg<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        let value = match arg.strip_prefix(LAUNCH_FLAG) {
            Some("") => args.next().map(|v| v.as_ref().to_string()),
            Some(rest) => rest.strip_prefix('=').map(str::to_string),
            None => None,
        };
        if let Some(id) = value.filter(|id| crate::instances::validate_id(id).is_ok()) {
            return Some(id);
        }
    }
    None
}

/// `name` made safe as a Windows file name (reserved characters replaced,
/// no trailing dot or space).
pub fn shortcut_file_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    format!("{}.lnk", if cleaned.is_empty() { "Minecraft" } else { &cleaned })
}

/// Creates (or replaces) `<dir>/<instance name>.lnk` pointing at the running
/// launcher executable with `--launch <id>`.
pub fn create_shortcut(dir: &Path, instance_id: &str, instance_name: &str) -> AppResult<PathBuf> {
    crate::instances::validate_id(instance_id)?;
    let exe = std::env::current_exe()?;
    let path = dir.join(shortcut_file_name(instance_name));
    write_link(&exe, &format!("{LAUNCH_FLAG} {instance_id}"), &format!("Jouer à {instance_name}"), &path)?;
    Ok(path)
}

#[cfg(windows)]
fn write_link(exe: &Path, arguments: &str, description: &str, path: &Path) -> AppResult<()> {
    let mut link = mslnk::ShellLink::new(exe).map_err(|e| AppError::Other(format!("raccourci impossible : {e}")))?;
    link.set_arguments(Some(arguments.to_string()));
    link.set_name(Some(description.to_string()));
    link.set_icon_location(Some(exe.display().to_string()));
    if let Some(dir) = exe.parent() {
        link.set_working_dir(Some(dir.display().to_string()));
    }
    link.create_lnk(path).map_err(|e| AppError::Other(format!("raccourci impossible : {e}")))
}

#[cfg(not(windows))]
fn write_link(_exe: &Path, _arguments: &str, _description: &str, _path: &Path) -> AppResult<()> {
    Err(AppError::Other("les raccourcis ne sont disponibles que sur Windows".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_launch_flag_in_both_forms() {
        assert_eq!(launch_arg(["app.exe", "--launch", "my-pack"]), Some("my-pack".to_string()));
        assert_eq!(launch_arg(["app.exe", "--launch=my-pack"]), Some("my-pack".to_string()));
        assert_eq!(launch_arg(["app.exe"]), None);
        assert_eq!(launch_arg(["app.exe", "--launch"]), None);
    }

    #[test]
    fn rejects_ids_that_could_escape_the_instances_folder() {
        assert_eq!(launch_arg(["app.exe", "--launch", "../evil"]), None);
        assert_eq!(launch_arg(["app.exe", "--launcher", "x"]), None);
    }

    #[test]
    fn shortcut_names_are_valid_windows_file_names() {
        assert_eq!(shortcut_file_name("All the Mods 9"), "All the Mods 9.lnk");
        assert_eq!(shortcut_file_name("a/b:c?"), "a_b_c_.lnk");
        assert_eq!(shortcut_file_name("pack. "), "pack.lnk");
        assert_eq!(shortcut_file_name("  "), "Minecraft.lnk");
    }
}
