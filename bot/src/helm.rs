use anyhow::{bail, Result};
use std::path::Path;
use tokio::process::Command;

const NAMESPACE: &str = "games";

pub async fn upgrade_install(name: &str, chart: &Path, values: &Path) -> Result<()> {
    let out = Command::new("helm")
        .args(["upgrade", "--install", name])
        .arg(chart)
        .arg("-f")
        .arg(values)
        .args(["-n", NAMESPACE])
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "helm upgrade failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

pub async fn uninstall(name: &str) -> Result<()> {
    let out = Command::new("helm")
        .args(["uninstall", name, "-n", NAMESPACE])
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "helm uninstall failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

pub async fn release_exists(name: &str) -> Result<bool> {
    Ok(Command::new("helm")
        .args(["status", name, "-n", NAMESPACE])
        .output()
        .await?
        .status
        .success())
}
