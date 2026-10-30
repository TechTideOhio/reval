use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use reval::{audit_repository, HygieneFindingKind};

struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            env::temp_dir().join(format!("reval-test-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn repo_hygiene_scanner_detects_violations_and_verifies_clean() {
    let temp = ScopedTempDir::new("hygiene");
    let root = temp.path();
    let skills_dir = temp.path().join("skills-root");
    fs::create_dir_all(skills_dir.join("installed-skill")).expect("mkdir skill");
    fs::write(
        skills_dir.join("installed-skill").join("SKILL.md"),
        "# Installed\n",
    )
    .expect("write skill");

    // Inject all hygiene violations
    fs::write(root.join(".env"), "SECRET=1\n").expect("write .env");
    fs::create_dir_all(root.join(".venv")).expect("mkdir .venv");
    fs::write(root.join("bun.lock"), "").expect("write bun.lock");
    fs::write(root.join("package-lock.json"), "{}").expect("write package-lock");
    fs::write(
        root.join("README.md"),
        "Welcome to your Lovable project\ngit clone <YOUR_GIT_URL>\n",
    )
    .expect("write README");
    fs::create_dir_all(root.join("rules").join("rust")).expect("mkdir rules");
    fs::write(
        root.join("rules").join("rust").join("style.md"),
        "See skill: `missing-rust-skill`, `installed-skill`\n",
    )
    .expect("write rule");

    let bad_report = audit_repository(root, std::slice::from_ref(&skills_dir)).expect("audit runs");
    assert!(!bad_report.passed);
    let kinds: Vec<_> = bad_report.findings.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&HygieneFindingKind::CommittedEnvFile));
    assert!(kinds.contains(&HygieneFindingKind::CommittedVenvOrStateDir));
    assert!(kinds.contains(&HygieneFindingKind::ConflictingLockfiles));
    assert!(kinds.contains(&HygieneFindingKind::PlaceholderReadme));
    assert!(kinds.contains(&HygieneFindingKind::DanglingSkillReference));

    // Clean up violations
    fs::remove_file(root.join(".env")).expect("rm .env");
    fs::remove_dir_all(root.join(".venv")).expect("rm .venv");
    fs::remove_file(root.join("bun.lock")).expect("rm bun.lock");
    fs::write(root.join("README.md"), "# Clean Flagship Repo\n").expect("clean README");
    fs::write(
        root.join("rules").join("rust").join("style.md"),
        "See skill: `installed-skill`\n",
    )
    .expect("fix rule");

    let clean_report = audit_repository(root, &[skills_dir]).expect("audit runs");
    assert!(clean_report.passed);
    assert_eq!(clean_report.high_severity_count, 0);
    assert_eq!(clean_report.medium_severity_count, 0);
}
