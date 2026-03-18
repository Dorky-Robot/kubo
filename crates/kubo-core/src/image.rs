//! Embedded Docker image context.
//!
//! All files needed to build the kubo Docker image are compiled into the binary.
//! When the image needs building, they're extracted to a temp dir and `docker build` runs.

use std::fs;
use std::process::{Command, Stdio};

use crate::KuboError;

const IMAGE_TAG: &str = "kubo:latest";

/// Version label baked into the image. Derived from cargo version + hash of all image files.
/// Changes automatically whenever any image file is modified.
pub const IMAGE_VERSION: &str = env!("KUBO_IMAGE_HASH");

// Embed all image context files at compile time
const DOCKERFILE: &str = include_str!("../../../image/Dockerfile");
const ENTRYPOINT: &str = include_str!("../../../image/entrypoint.sh");
const ZSHRC: &str = include_str!("../../../image/zshrc");
const ZSH_THEME: &str = include_str!("../../../image/kubo.zsh-theme");
const TMUX_CONF: &str = include_str!("../../../image/tmux.conf");
const VIMRC: &str = include_str!("../../../image/vimrc");
const WELCOME: &str = include_str!("../../../image/welcome.sh");
const PBPASTE: &str = include_str!("../../../image/pbpaste");
const PBCOPY: &str = include_str!("../../../image/pbcopy");
const CLIP: &str = include_str!("../../../image/clip");

/// Check if the kubo image exists and matches the current version.
pub fn image_up_to_date() -> Result<bool, KuboError> {
    let output = Command::new("docker")
        .args([
            "image",
            "inspect",
            "-f",
            "{{index .Config.Labels \"kubo.image-version\"}}",
            IMAGE_TAG,
        ])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let label = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(label == IMAGE_VERSION)
}

/// Build the kubo image from embedded files.
/// If `no_cache` is true, Docker layer cache is bypassed (useful for
/// picking up new versions of tools installed via remote scripts).
pub fn build_image(no_cache: bool) -> Result<(), KuboError> {
    let tmp = tempfile::tempdir().map_err(KuboError::Io)?;
    let dir = tmp.path();

    // Write all image files
    let dockerfile_with_label = format!(
        "{}\nLABEL kubo.image-version=\"{}\"\n",
        DOCKERFILE, IMAGE_VERSION
    );
    fs::write(dir.join("Dockerfile"), dockerfile_with_label)?;
    fs::write(dir.join("entrypoint.sh"), ENTRYPOINT)?;
    fs::write(dir.join("zshrc"), ZSHRC)?;
    fs::write(dir.join("kubo.zsh-theme"), ZSH_THEME)?;
    fs::write(dir.join("tmux.conf"), TMUX_CONF)?;
    fs::write(dir.join("vimrc"), VIMRC)?;
    fs::write(dir.join("welcome.sh"), WELCOME)?;
    fs::write(dir.join("pbpaste"), PBPASTE)?;
    fs::write(dir.join("pbcopy"), PBCOPY)?;
    fs::write(dir.join("clip"), CLIP)?;

    let mut args = vec!["build", "-t", IMAGE_TAG];
    if no_cache {
        args.push("--no-cache");
    }
    args.push(".");

    let status = Command::new("docker")
        .args(&args)
        .current_dir(dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        return Err(KuboError::Container("failed to build kubo image".into()));
    }
    Ok(())
}

/// Ensure the kubo image exists and is up to date. Builds if needed.
pub fn ensure_image() -> Result<(), KuboError> {
    if image_up_to_date()? {
        return Ok(());
    }
    eprintln!("Building kubo image (v{IMAGE_VERSION})...");
    build_image(false)?;
    eprintln!("Image ready.");
    Ok(())
}

/// Return the image tag.
pub fn image_tag() -> &'static str {
    IMAGE_TAG
}

/// Return the image version baked into this binary.
pub fn version() -> &'static str {
    IMAGE_VERSION
}
