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

/// A Docker container that mounts one or more host directories for isolated development.
pub struct Container {
    pub name: String,
    pub mounts: Vec<Mount>,
}

/// A directory mounted into the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub host_path: PathBuf,
    pub container_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerStatus {
    pub name: String,
    pub status: String,
    pub mounts: Vec<String>,
}

impl Container {
    /// Create a container from a name and a list of host directory paths.
    pub fn new(name: &str, dirs: &[PathBuf]) -> Result<Self, KuboError> {
        let mut mounts = Vec::new();
        for dir in dirs {
            let canonical = dir
                .canonicalize()
                .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", dir.display())))?;

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

            let container_path = format!("/work/{dir_name}");

            mounts.push(Mount {
                host_path: canonical,
                container_path,
            });
        }

        let container_name = format!("kubo-{name}");
        Ok(Self {
            name: container_name,
            mounts,
        })
    }

    /// Create a container from a single directory path (legacy convenience).
    /// The container name is derived from the directory name.
    pub fn from_path(path: &Path) -> Result<Self, KuboError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", path.display())))?;

        let dir_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| KuboError::InvalidPath("cannot determine directory name".into()))?
            .to_string();

        Self::new(&dir_name, &[canonical])
    }

    /// Load an existing container's mounts from its Docker labels.
    pub fn load(name: &str) -> Result<Self, KuboError> {
        let container_name = if name.starts_with("kubo-") {
            name.to_string()
        } else {
            format!("kubo-{name}")
        };

        let output = Command::new("docker")
            .args([
                "container",
                "inspect",
                "-f",
                "{{index .Config.Labels \"kubo.mounts\"}}",
                &container_name,
            ])
            .output()?;

        if !output.status.success() {
            return Err(KuboError::Container(format!(
                "container {container_name} not found"
            )));
        }

        let mounts_json = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mounts: Vec<Mount> = if mounts_json.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<MountSerde>>(&mounts_json)
                .map_err(|e| KuboError::Container(format!("bad mount label: {e}")))?
                .into_iter()
                .map(|m| Mount {
                    host_path: PathBuf::from(m.host),
                    container_path: m.container,
                })
                .collect()
        };

        Ok(Self {
            name: container_name,
            mounts,
        })
    }

    /// Add a directory to this container's mount list.
    /// The container must be recreated for this to take effect.
    pub fn add_mount(&mut self, dir: &Path) -> Result<(), KuboError> {
        let canonical = dir
            .canonicalize()
            .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", dir.display())))?;

        if !canonical.is_dir() {
            return Err(KuboError::InvalidPath(format!(
                "{} is not a directory",
                canonical.display()
            )));
        }

        let dir_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| KuboError::InvalidPath("cannot determine directory name".into()))?
            .to_string();

        // Skip if already mounted
        if self.mounts.iter().any(|m| m.host_path == canonical) {
            return Ok(());
        }

        // If we had a single mount at /work, migrate to /work/<name> layout
        if self.mounts.len() == 1 && self.mounts[0].container_path == "/work" {
            let old_name = self.mounts[0]
                .host_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_string();
            self.mounts[0].container_path = format!("/work/{old_name}");
        }

        self.mounts.push(Mount {
            host_path: canonical,
            container_path: format!("/work/{dir_name}"),
        });

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

    /// Get the image ID of the current kubo:latest image.
    fn current_image_id() -> Result<Option<String>, KuboError> {
        let output = Command::new("docker")
            .args(["image", "inspect", "-f", "{{.Id}}", IMAGE])
            .output()?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    /// Get the image ID that this container was created from.
    fn container_image_id(&self) -> Result<Option<String>, KuboError> {
        let output = Command::new("docker")
            .args(["container", "inspect", "-f", "{{.Image}}", &self.name])
            .output()?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    /// Check if the container's image is outdated compared to kubo:latest.
    fn is_outdated(&self) -> Result<bool, KuboError> {
        let current = Self::current_image_id()?;
        let container = self.container_image_id()?;
        match (current, container) {
            (Some(cur), Some(con)) => Ok(cur != con),
            _ => Ok(false),
        }
    }

    /// Force remove this container.
    fn force_remove(&self) -> Result<(), KuboError> {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        Ok(())
    }

    /// Serialize mounts to JSON for the label.
    fn mounts_label(&self) -> String {
        let serde_mounts: Vec<MountSerde> = self
            .mounts
            .iter()
            .map(|m| MountSerde {
                host: m.host_path.display().to_string(),
                container: m.container_path.clone(),
            })
            .collect();
        serde_json::to_string(&serde_mounts).unwrap_or_default()
    }

    /// Create and start the container, or start it if it already exists but is stopped.
    /// If the container exists but uses an outdated image, it is recreated.
    /// Returns Ok(true) if a new container was created, Ok(false) if an existing one was started.
    pub fn ensure_running(&self) -> Result<bool, KuboError> {
        if self.exists()? {
            if self.is_outdated()? {
                eprintln!("Image updated, recreating container {}...", self.name);
                self.force_remove()?;
            } else if self.is_running()? {
                return Ok(false);
            } else {
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
        }

        self.create()?;
        Ok(true)
    }

    /// Create the container (must not already exist).
    pub fn create(&self) -> Result<(), KuboError> {
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            self.name.clone(),
            "--label".to_string(),
            LABEL.to_string(),
            "--label".to_string(),
            format!("kubo.mounts={}", self.mounts_label()),
            "-u".to_string(),
            "dev".to_string(),
        ];

        // Host networking — container shares the host network stack.
        // Any port a dev app binds to is immediately accessible on the host.
        args.extend(["--network".to_string(), "host".to_string()]);

        // Working directory
        args.extend(["-w".to_string(), "/work".to_string()]);

        // Mount project directories
        for mount in &self.mounts {
            args.extend([
                "-v".to_string(),
                format!("{}:{}", mount.host_path.display(), mount.container_path),
            ]);
        }

        // Mount host credentials (read-only)
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(&home);

            let cred_mounts: &[(&str, &str)] = &[
                (".config/gh", "/home/dev/.config/gh"),
                (".ssh", "/home/dev/.ssh"),
                (".katulong/uploads", "/home/dev/.katulong/uploads"),
            ];

            for (src, dest) in cred_mounts {
                let host_path = home.join(src);
                if host_path.exists() {
                    args.extend([
                        "-v".to_string(),
                        format!("{}:{dest}:ro", host_path.display()),
                    ]);
                }
            }
        }

        // Pass git identity
        for (key, env) in &[
            ("user.name", "GIT_AUTHOR_NAME"),
            ("user.email", "GIT_AUTHOR_EMAIL"),
        ] {
            if let Some(val) = git_config_get(key) {
                args.extend(["-e".to_string(), format!("{env}={val}")]);
                let committer_env = env.replace("AUTHOR", "COMMITTER");
                args.extend(["-e".to_string(), format!("{committer_env}={val}")]);
            }
        }

        // Pass signing key path if configured
        if let Some(key) = git_config_get("user.signingkey") {
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

        args.push(IMAGE.to_string());

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

        Ok(())
    }

    /// Recreate this container (preserves name and mounts, fresh image).
    pub fn recreate(&self) -> Result<(), KuboError> {
        self.force_remove()?;
        self.create()
    }

    /// Exec into the container with an interactive shell.
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

    /// Remove the container.
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

    /// Check if a container with the given name exists.
    pub fn name_exists(name: &str) -> Result<bool, KuboError> {
        let container_name = if name.starts_with("kubo-") {
            name.to_string()
        } else {
            format!("kubo-{name}")
        };
        let output = Command::new("docker")
            .args(["container", "inspect", &container_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        Ok(output.success())
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
                "{{.Names}}\t{{.Status}}\t{{.Label \"kubo.mounts\"}}",
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
                    let mounts_json = parts.get(2).unwrap_or(&"");
                    let mount_paths: Vec<String> =
                        serde_json::from_str::<Vec<MountSerde>>(mounts_json)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|m| m.host)
                            .collect();
                    Some(ContainerStatus {
                        name: parts[0].to_string(),
                        status: parts[1].to_string(),
                        mounts: mount_paths,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(containers)
    }
}

/// Serialization helper for mount labels.
#[derive(serde::Serialize, serde::Deserialize)]
struct MountSerde {
    host: String,
    container: String,
}
