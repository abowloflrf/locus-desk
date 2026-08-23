use std::{env, fs, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=LOCUS_GIT_COMMIT");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    if let Ok(head) = fs::read_to_string(".git/HEAD")
        && let Some(reference) = head.trim().strip_prefix("ref: refs/")
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        println!("cargo:rerun-if-changed=.git/refs/{reference}");
    }

    let commit = env::var("LOCUS_GIT_COMMIT")
        .ok()
        .filter(|value| is_safe_version_value(value))
        .or_else(git_commit)
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=LOCUS_GIT_COMMIT={commit}");
}

fn git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    is_safe_version_value(&value).then_some(value)
}

fn is_safe_version_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
