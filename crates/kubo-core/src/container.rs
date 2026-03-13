use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::KuboError;

const IMAGE: &str = "kubo:latest";
const LABEL: &str = "managed-by=kubo";

/// Read a value from the host's git config.
fn git_config_get(key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()?;
    if output.status.success() {
        let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if val.is_empty() { None } else { Some(val) }
    } else {
        None
    }
}

/// A Docker container that mounts a host directory for isolated development.
pub struct Container {
    pub name: String,
    pub host_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerStatus {
    pub name: String,
    pub status: String,
    pub host_path: String,
}

impl Container {
    /// Create a Container handle from a host directory path.
    /// The container name is derived from the directory name.
    pub fn from_path(path: &Path) -> Result<Self, KuboError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", path.display())))?;

        if !canonical.is_dir() {
            return Err(KuboError::InvalidPath(format!(
                "{} is not a directory",
                canonical.display()
            )));
        }

        let dir_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| KuboError::InvalidPath("cannot determine directory name".into()))?;

        let name = format!("kubo-{dir_name}");

        Ok(Self {
            name,
            host_path: canonical,
        })
    }

    /// Check if the kubo image exists.
    pub fn image_exists() -> Result<bool, KuboError> {
        let output = Command::new("docker")
            .args(["image", "inspect", IMAGE])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        Ok(output.success())
    }

    /// Build the kubo image from the bundled Dockerfile context.
    pub fn build_image(context_dir: &Path) -> Result<(), KuboError> {
        let status = Command::new("docker")
            .args(["build", "-t", IMAGE, "."])
            .current_dir(context_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container("failed to build kubo image".into()));
        }
        Ok(())
    }

    /// Check if Docker is available.
    pub fn check_docker() -> Result<(), KuboError> {
        let output = Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match output {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(KuboError::DockerNotFound(
                "docker daemon is not running".into(),
            )),
            Err(e) => Err(KuboError::DockerNotFound(format!(
                "docker command not found: {e}"
            ))),
        }
    }

    /// Returns true if this container already exists (running or stopped).
    pub fn exists(&self) -> Result<bool, KuboError> {
        let output = Command::new("docker")
            .args(["container", "inspect", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        Ok(output.success())
    }

    /// Returns true if this container is currently running.
    pub fn is_running(&self) -> Result<bool, KuboError> {
        let output = Command::new("docker")
            .args([
                "container",
                "inspect",
                "-f",
                "{{.State.Running}}",
                &self.name,
            ])
            .output()?;

        Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true")
    }

    /// Create and start the container, or start it if it already exists but is stopped.
    /// Returns Ok(true) if a new container was created, Ok(false) if an existing one was started.
    pub fn ensure_running(&self) -> Result<bool, KuboError> {
        if self.is_running()? {
            return Ok(false);
        }

        if self.exists()? {
            let status = Command::new("docker")
                .args(["start", &self.name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;

            if !status.success() {
                return Err(KuboError::Container(format!(
                    "failed to start existing container {}",
                    self.name
                )));
            }
            return Ok(false);
        }

        // Create new container
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            self.name.clone(),
            "--label".to_string(),
            LABEL.to_string(),
            "--label".to_string(),
            format!("kubo.host-path={}", self.host_path.display()),
            "-v".to_string(),
            format!("{}:/work", self.host_path.display()),
            "-w".to_string(),
            "/work".to_string(),
            "-u".to_string(),
            "dev".to_string(),
        ];

        // Mount host credentials (read-only)
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(&home);

            let mounts: &[(&str, &str)] = &[
                (".config/gh", "/home/dev/.config/gh"),
                (".ssh", "/home/dev/.ssh"),
            ];

            for (src, dest) in mounts {
                let host_path = home.join(src);
                if host_path.exists() {
                    args.extend([
                        "-v".to_string(),
                        format!("{}:{dest}:ro", host_path.display()),
                    ]);
                }
            }
        }

        // Pass git identity from host (avoids mounting gitconfig with macOS-specific paths)
        for (key, env) in &[
            ("user.name", "GIT_AUTHOR_NAME"),
            ("user.email", "GIT_AUTHOR_EMAIL"),
        ] {
            if let Some(val) = git_config_get(key) {
                args.extend(["-e".to_string(), format!("{env}={val}")]);
                // Set committer too
                let committer_env = env.replace("AUTHOR", "COMMITTER");
                args.extend(["-e".to_string(), format!("{committer_env}={val}")]);
            }
        }

        // Pass signing key path if configured
        if let Some(key) = git_config_get("user.signingkey") {
            // Remap host path to container path
            let container_key = if key.starts_with('~') {
                key.replacen('~', "/home/dev", 1)
            } else if key.contains("/.ssh/") {
                format!("/home/dev/.ssh/{}", key.rsplit('/').next().unwrap_or(&key))
            } else {
                key
            };
            args.extend([
                "-e".to_string(),
                format!("KUBO_GIT_SIGNING_KEY={container_key}"),
            ]);
        }

        args.extend([IMAGE.to_string()]);

        let status = Command::new("docker")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to create container {}",
                self.name
            )));
        }

        Ok(true)
    }

    /// Exec into the container with an interactive shell.
    /// This replaces the current process's stdin/stdout/stderr.
    pub fn exec_shell(&self) -> Result<std::process::ExitStatus, KuboError> {
        let status = Command::new("docker")
            .args(["exec", "-it", "-u", "dev", "-w", "/work", &self.name, "zsh"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        Ok(status)
    }

    /// Stop the container.
    pub fn stop(&self) -> Result<(), KuboError> {
        let status = Command::new("docker")
            .args(["stop", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to stop container {}",
                self.name
            )));
        }
        Ok(())
    }

    /// Remove the container (must be stopped first).
    pub fn remove(&self) -> Result<(), KuboError> {
        let status = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to remove container {}",
                self.name
            )));
        }
        Ok(())
    }

    /// List all kubo-managed containers.
    pub fn list_all() -> Result<Vec<ContainerStatus>, KuboError> {
        let output = Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                &format!("label={LABEL}"),
                "--format",
                "{{.Names}}\t{{.Status}}\t{{.Label \"kubo.host-path\"}}",
            ])
            .output()?;

        if !output.status.success() {
            return Err(KuboError::Container("failed to list containers".into()));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let containers = text
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.len() >= 2 {
                    Some(ContainerStatus {
                        name: parts[0].to_string(),
                        status: parts[1].to_string(),
                        host_path: parts.get(2).unwrap_or(&"").to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(containers)
    }
}
