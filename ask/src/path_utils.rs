/// Expand the tilde in a path to the user's home directory.
pub fn expand_path(path: &str) -> anyhow::Result<String> {
    if let Some(tail) = path.strip_prefix("~/") {
        if let Some(user_dirs) = directories::UserDirs::new() {
            return Ok(format!("{}/{}", user_dirs.home_dir().display(), tail));
        } else {
            anyhow::bail!("Could not find user directories to expand tilde in path")
        }
    }
    Ok(path.to_string())
}
