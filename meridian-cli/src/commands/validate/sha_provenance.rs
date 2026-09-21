//! Port of `scripts/kernel-validate.mjs`'s `sha-provenance` check. Every
//! vendored skill under `skills/` carries a `PIN.yaml` next to it. The pin
//! is verified by recomputing the digest here; a pin that is merely
//! recorded proves nothing. File discovery and reading stay in this crate
//! (`meridian-cli`); the pure digest type
//! (`meridian_core::types::ContentDigest`) and the strict YAML adapter
//! (`meridian_app::source_format::parse_yaml`) are reused, not
//! reimplemented.
//!
//! The `requires_instance_context` half of a pin is checked separately by
//! `instance_context::run` — unchanged by this module.

use std::fs;
use std::path::Path;

use meridian_app::source_format::parse_yaml;
use meridian_core::types::ContentDigest;
use serde_json::Value;

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    let skills_dir = kernel_root.join("skills");
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failures.push(format!(
                        "sha-provenance: could not read a directory entry under {}: {error}",
                        skills_dir.display()
                    ));
                    continue;
                }
            };
            // `file_type()` reports the entry's own type as the directory
            // lists it, without following a symlink — unlike
            // `entry.path().is_dir()`, which stats through the link and
            // would silently pin a skill's provenance to whatever a symlink
            // happens to point at right now.
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    failures.push(format!(
                        "sha-provenance: could not read the type of {}: {error}",
                        entry.path().display()
                    ));
                    continue;
                }
            };
            if file_type.is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();

    if names.is_empty() {
        warnings.push("sha-provenance: no vendored skills found".to_string());
        return Outcome { failures, warnings };
    }

    for name in names {
        let dir = skills_dir.join(&name);
        let pin_path = dir.join("PIN.yaml");
        let Ok(pin_raw) = fs::read_to_string(&pin_path) else {
            failures.push(format!(
                "sha-provenance: {name} has no PIN.yaml; its provenance is unverifiable"
            ));
            continue;
        };
        let pin = match parse_yaml(&pin_raw) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("sha-provenance: {name} PIN.yaml: {error}"));
                continue;
            }
        };

        let artifact_name = pin.get("artifact").and_then(Value::as_str).unwrap_or("");
        let artifact_path = dir.join(artifact_name);
        let Ok(artifact_text) = fs::read_to_string(&artifact_path) else {
            failures.push(format!(
                "sha-provenance: {name} pins \"{artifact_name}\", which does not exist"
            ));
            continue;
        };
        let actual = ContentDigest::of_str(&artifact_text);
        let pinned_sha256 = pin.get("sha256").and_then(Value::as_str);
        match pinned_sha256 {
            None => failures.push(format!("sha-provenance: {name} PIN.yaml records no sha256")),
            Some(pinned) if pinned != actual.value() => failures.push(format!(
                "sha-provenance: {name} artifact {} != pinned {}",
                short(actual.value()),
                short(pinned)
            )),
            Some(_) => {}
        }

        if let Some(archive) = pin.get("source_archive") {
            let arch_path_rel = archive.get("path").and_then(Value::as_str);
            let arch_sha256 = archive.get("sha256").and_then(Value::as_str);
            if let (Some(arch_path_rel), Some(arch_sha256)) = (arch_path_rel, arch_sha256) {
                let arch_path = dir.join(arch_path_rel);
                match fs::read(&arch_path) {
                    Err(_) => failures.push(format!(
                        "sha-provenance: {name} source archive {arch_path_rel} is missing"
                    )),
                    Ok(bytes) => {
                        let arch_actual = ContentDigest::of_bytes(&bytes);
                        if arch_actual.value() != arch_sha256 {
                            failures.push(format!(
                                "sha-provenance: {name} source archive {} != pinned {}",
                                short(arch_actual.value()),
                                short(arch_sha256)
                            ));
                        }
                    }
                }
            }
        }
    }

    Outcome { failures, warnings }
}

fn short(digest: &str) -> String {
    digest.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn no_skills_directory_at_all_is_a_warning_not_a_failure() {
        let temp =
            std::env::temp_dir().join(format!("sha-provenance-no-skills-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        let outcome = run(&temp);
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("no vendored skills found"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_skill_whose_artifact_matches_its_pin_is_clean() {
        let temp =
            std::env::temp_dir().join(format!("sha-provenance-clean-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        write(&temp, "skills/demo/SKILL.md", "hello world\n");
        let digest = ContentDigest::of_str("hello world\n");
        write(
            &temp,
            "skills/demo/PIN.yaml",
            &format!("artifact: SKILL.md\nsha256: {}\n", digest.value()),
        );
        let outcome = run(&temp);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_mismatched_pin_is_a_failure() {
        let temp =
            std::env::temp_dir().join(format!("sha-provenance-mismatch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        write(&temp, "skills/demo/SKILL.md", "hello world\n");
        write(
            &temp,
            "skills/demo/PIN.yaml",
            "artifact: SKILL.md\nsha256: dead000000000000000000000000000000000000000000000000000000000000\n",
        );
        let outcome = run(&temp);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("!= pinned"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_missing_pin_yaml_is_a_failure() {
        let temp =
            std::env::temp_dir().join(format!("sha-provenance-nopin-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        write(&temp, "skills/demo/SKILL.md", "hello world\n");
        let outcome = run(&temp);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("has no PIN.yaml"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_pinned_artifact_that_does_not_exist_is_a_failure() {
        let temp = std::env::temp_dir().join(format!(
            "sha-provenance-missing-artifact-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        write(
            &temp,
            "skills/demo/PIN.yaml",
            "artifact: SKILL.md\nsha256: abc\n",
        );
        let outcome = run(&temp);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("which does not exist"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_mismatched_source_archive_is_a_separate_failure() {
        let temp = std::env::temp_dir().join(format!(
            "sha-provenance-archive-mismatch-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        write(&temp, "skills/demo/SKILL.md", "hello world\n");
        let digest = ContentDigest::of_str("hello world\n");
        write(&temp, "skills/demo/source/archive.zip", "not really a zip");
        write(
            &temp,
            "skills/demo/PIN.yaml",
            &format!(
                "artifact: SKILL.md\nsha256: {}\nsource_archive:\n  path: source/archive.zip\n  sha256: dead000000000000000000000000000000000000000000000000000000000000\n",
                digest.value()
            ),
        );
        let outcome = run(&temp);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("source archive"));
        let _ = fs::remove_dir_all(&temp);
    }

    /// RAII guard so a panicking assertion still removes the temp tree it
    /// created, instead of leaking it — used by the new tests below.
    struct RaiiTemp(std::path::PathBuf);
    impl RaiiTemp {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "sha-provenance-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            RaiiTemp(dir)
        }
    }
    impl Drop for RaiiTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_to_a_directory_under_skills_is_never_followed_at_discovery() {
        use std::os::unix::fs::symlink;

        let temp = RaiiTemp::new("symlink-skip");
        write(&temp.0, "skills/real-skill/SKILL.md", "hello world\n");
        let digest = ContentDigest::of_str("hello world\n");
        write(
            &temp.0,
            "skills/real-skill/PIN.yaml",
            &format!("artifact: SKILL.md\nsha256: {}\n", digest.value()),
        );
        // A symlink sitting inside skills/ that happens to point at a real
        // directory: is_dir() (which follows symlinks) would enrol it as a
        // second "skill" with no PIN.yaml of its own and fail closed on a
        // name that is not a real vendored skill at all. file_type() (which
        // does not follow it) must not enrol it.
        symlink(
            temp.0.join("skills/real-skill"),
            temp.0.join("skills/linked-skill"),
        )
        .expect("create symlink");

        let outcome = run(&temp.0);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }
}
