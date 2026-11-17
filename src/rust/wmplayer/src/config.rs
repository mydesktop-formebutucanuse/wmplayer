//! wmplayer's settings were Audacious's: the first start copies them over.

use std::fs;
use std::io;
use std::path::Path;

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let kind = e.file_type()?;
        if kind.is_dir() {
            copy_dir(&src, &dst)?;
        } else if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&src)?, &dst)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Copies the directory `old` to `new` if `new` is not there yet. True
/// if it did.
pub fn migrate(old: &Path, new: &Path) -> io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    let name = new.file_name().unwrap_or_default().to_string_lossy();
    let tmp = new.with_file_name(format!(".{name}.copying"));
    let _ = fs::remove_dir_all(&tmp);
    copy_dir(old, &tmp)?;
    fs::rename(&tmp, new)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_once() {
        let base = std::env::temp_dir().join(format!("wmplayer-migrate-{}", std::process::id()));
        let (old, new) = (base.join("audacious"), base.join("wmplayer"));
        fs::create_dir_all(old.join("playlists")).unwrap();
        fs::write(old.join("config"), "[audacious]\n").unwrap();
        fs::write(old.join("playlists/1000.audpl"), "x").unwrap();
        assert!(migrate(&old, &new).unwrap());
        assert_eq!(fs::read_to_string(new.join("playlists/1000.audpl")).unwrap(), "x");
        fs::write(new.join("config"), "changed").unwrap();
        assert!(!migrate(&old, &new).unwrap());
        assert_eq!(fs::read_to_string(new.join("config")).unwrap(), "changed");
        fs::remove_dir_all(&base).unwrap();
    }
}
