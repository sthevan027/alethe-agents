use std::path::{Path, PathBuf};

/// Confirms `candidate` resolves to a location inside `root`, following symlinks on both sides
/// and neutralizing `..` traversal through canonicalization. Returns the canonicalized candidate
/// on success so callers can operate on that resolved path instead of the raw (renderer-supplied)
/// one, avoiding a check/use gap.
///
/// Both `root` and `candidate` must already exist — this is an authorization boundary, not a
/// general-purpose path utility, so an entry that can't be resolved is treated as outside the
/// boundary rather than silently allowed.
pub fn ensure_within(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "unable to resolve the active working directory".to_string())?;
    if !root.is_dir() {
        return Err("the active working directory is not a directory".to_string());
    }

    let candidate = candidate
        .canonicalize()
        .map_err(|_| "entry not found".to_string())?;

    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err("entry is outside the active terminal's working directory".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("alethe-path-guard-{label}-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn allows_a_nested_path() {
        let root = temp_dir("nested-root");
        let file = root.join("child.txt");
        fs::write(&file, b"hi").unwrap();

        let resolved = ensure_within(&root, &file).expect("nested path should be allowed");
        assert_eq!(resolved, file.canonicalize().unwrap());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn allows_a_deeply_nested_path() {
        let root = temp_dir("deep-root");
        let nested = root.join("a").join("b").join("c.txt");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(&nested, b"hi").unwrap();

        assert!(ensure_within(&root, &nested).is_ok());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_dot_dot_traversal_out_of_root() {
        let root = temp_dir("traversal-root");
        let sibling = temp_dir("traversal-sibling");
        let escape_target = sibling.join("secret.txt");
        fs::write(&escape_target, b"secret").unwrap();

        let traversal_path = root.join("..").join(sibling.file_name().unwrap()).join("secret.txt");
        let result = ensure_within(&root, &traversal_path);
        assert!(result.is_err(), "traversal outside root must be rejected");

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&sibling).ok();
    }

    #[test]
    fn rejects_a_disjoint_absolute_path() {
        let root = temp_dir("disjoint-root");
        let other = temp_dir("disjoint-other");
        let outside_file = other.join("outside.txt");
        fs::write(&outside_file, b"hi").unwrap();

        let result = ensure_within(&root, &outside_file);
        assert!(result.is_err());

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&other).ok();
    }

    #[test]
    fn rejects_non_existent_root() {
        let root = std::env::temp_dir().join("alethe-path-guard-does-not-exist");
        let candidate = std::env::temp_dir();
        assert!(ensure_within(&root, &candidate).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_escapes_root() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink-root");
        let outside = temp_dir("symlink-outside");
        let outside_file = outside.join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();

        let link = root.join("escape-link");
        symlink(&outside_file, &link).unwrap();

        let result = ensure_within(&root, &link);
        assert!(result.is_err(), "a symlink escaping root must be rejected");

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&outside).ok();
    }

    #[cfg(unix)]
    #[test]
    fn allows_a_symlink_that_stays_within_root() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink-inside-root");
        let real_file = root.join("real.txt");
        fs::write(&real_file, b"hi").unwrap();

        let link = root.join("link.txt");
        symlink(&real_file, &link).unwrap();

        assert!(ensure_within(&root, &link).is_ok());

        fs::remove_dir_all(&root).ok();
    }
}
