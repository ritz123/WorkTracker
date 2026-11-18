//! Sets `WORK_TRACKER_VERSION` for the compile so the app reports the git tag.
//!
//! Order: `WORK_TRACKER_VERSION` in the environment (the release workflow passes
//! the pushed tag), then the tag on HEAD, then the nearest ancestor tag, then
//! `version` in `Cargo.toml` when this checkout has no tag. A leading `v` is
//! removed. A tag that is not `MAJOR.MINOR.PATCH` is ignored so the update
//! check always sees three numeric parts.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WORK_TRACKER_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        if let Some(reference) = head.trim().strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=.git/{reference}");
        }
    }

    let version = version_from_env()
        .or_else(version_from_git)
        .unwrap_or_else(package_version);
    assert!(
        is_semver(&version),
        "resolved version '{version}' is not MAJOR.MINOR.PATCH"
    );
    println!("cargo:rustc-env=WORK_TRACKER_VERSION={version}");
}

/// `WORK_TRACKER_VERSION` from the environment, used by the release workflow.
fn version_from_env() -> Option<String> {
    std::env::var("WORK_TRACKER_VERSION")
        .ok()
        .and_then(|raw| semver_tag(&raw))
}

/// Tag pointing at HEAD, or the nearest ancestor tag when HEAD is untagged.
///
/// An untagged commit keeps the last released number rather than inventing a
/// `1.2.3-3-gabcdef` suffix, which the in-app update check cannot compare.
fn version_from_git() -> Option<String> {
    git_describe(&["describe", "--tags", "--exact-match"])
        .or_else(|| git_describe(&["describe", "--tags", "--abbrev=0"]))
}

fn git_describe(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    semver_tag(&String::from_utf8_lossy(&output.stdout))
}

fn package_version() -> String {
    std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by cargo")
}

/// Accepts `v1.2.3` or `1.2.3` and returns `1.2.3`.
fn semver_tag(raw: &str) -> Option<String> {
    let stripped = raw.trim().trim_start_matches(['v', 'V']);
    is_semver(stripped).then(|| stripped.to_string())
}

fn is_semver(value: &str) -> bool {
    let mut parts = value.split('.');
    let three = parts.next().zip(parts.next()).zip(parts.next());
    matches!(three, Some(((_, _), _)))
        && parts.next().is_none()
        && value.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}
