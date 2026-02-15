use std::env;
use std::process::Command;

fn main() {
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let profile = env::var("PROFILE").unwrap();

    let full_version = if profile == "debug" {
        let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
        let dirty = !is_ci_environment() && is_git_dirty();
        if dirty {
            format!("{}-dev+{}.dirty", version, git_hash)
        } else {
            format!("{}-dev+{}", version, git_hash)
        }
    } else {
        version
    };

    println!("cargo:rustc-env=AGENTREE_VERSION={}", full_version);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
    println!("cargo:rerun-if-changed=.git/index");
}

fn get_git_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn is_git_dirty() -> bool {
    let unstaged = Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(false);

    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(false);

    unstaged || staged
}

fn is_ci_environment() -> bool {
    env::var("CI").is_ok() || env::var("GITHUB_ACTIONS").is_ok() || env::var("GITLAB_CI").is_ok()
}
