use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::KuboError;

use std::io::Write;

const IMAGE: &str = "kubo:latest";
const LABEL: &str = "managed-by=kubo";
/// File inside the container (on the persistent home volume) where deferred
/// mount changes are stored when active sessions prevent an immediate recreate.
const PENDING_MOUNTS_PATH: &str = "/home/dev/.kubo/pending-mounts.json";

/// Manifest stored inside a .kubo export archive.
#[derive(serde::Serialize, serde::Deserialize)]
struct ExportManifest {
    /// Original container name (e.g. "kubo-myproject").
    name: String,
    /// Mount configuration from the original container.
    mounts: Vec<MountSerde>,
    /// kubo image version that created this export.
    image_version: String,
}

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

/// Append host credential mount args to a Docker `run` argument vector.
///
/// Mounts SSH keys, tool configs, and katulong uploads from the host home directory.
/// Used by both `create` and `import` to ensure consistent credential passthrough.
fn append_host_credential_args(args: &mut Vec<String>) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let home = PathBuf::from(&home);

    // Read-write mounts (credentials that tools may update)
    let rw_mounts: &[(&str, &str)] = &[(".config/gh", "/home/dev/.config/gh")];

    for (src, dest) in rw_mounts {
        let host_path = home.join(src);
        if host_path.exists() {
            args.extend(["-v".to_string(), format!("{}:{dest}", host_path.display())]);
        }
    }

    // Read-only mounts — auto-detected host configs.
    // Only mounted if they exist, so kubo works on machines
    // that don't have all the Dorky Robot tools installed.
    let ro_mounts: &[(&str, &str)] = &[
        (".ssh", "/home/dev/.ssh"),
        // Dorky Robot tool configs
        (".config/tunnels", "/home/dev/.config/tunnels"),
        (".config/katulong", "/home/dev/.config/katulong"),
        (".config/yelo", "/home/dev/.config/yelo"),
        // Cloudflared auth cert
        (".cloudflared", "/home/dev/.cloudflared"),
    ];

    // Read-write mounts — katulong uploads need to be writable so
    // the clipboard bridge can share images between host and container.
    // Always create the directory so the mount is guaranteed to exist.
    let katulong_uploads = home.join(".katulong/uploads");
    let _ = std::fs::create_dir_all(&katulong_uploads);
    args.extend([
        "-v".to_string(),
        format!("{}:/home/dev/.katulong/uploads", katulong_uploads.display()),
    ]);

    for (src, dest) in ro_mounts {
        let host_path = home.join(src);
        if host_path.exists() {
            args.extend([
                "-v".to_string(),
                format!("{}:{dest}:ro", host_path.display()),
            ]);
        }
    }
}

/// Append git identity and signing key env vars to a Docker `run` argument vector.
fn append_git_identity_args(args: &mut Vec<String>) {
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
    pub active_sessions: usize,
    pub running: bool,
    pub image_version: String,
}

impl ContainerStatus {
    /// The user-facing name, without the internal "kubo-" prefix.
    pub fn display_name(&self) -> &str {
        self.name.strip_prefix("kubo-").unwrap_or(&self.name)
    }
}

impl Container {
    /// The user-facing name, without the internal "kubo-" prefix.
    pub fn display_name(&self) -> &str {
        self.name.strip_prefix("kubo-").unwrap_or(&self.name)
    }

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

    /// Create a container from a single directory path.
    /// The container name is derived from the directory name.
    /// The directory is mounted directly at /work so files are immediately visible.
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
            .ok_or_else(|| KuboError::InvalidPath("cannot determine directory name".into()))?
            .to_string();

        Ok(Self {
            name: format!("kubo-{dir_name}"),
            mounts: vec![Mount {
                host_path: canonical,
                container_path: "/work".to_string(),
            }],
        })
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
        let status = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to remove container {}",
                self.display_name()
            )));
        }
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
    /// Returns Ok(true) if a new container was created, Ok(false) if an existing one was started.
    /// Does NOT auto-recreate outdated containers — use `kubo update` for that.
    pub fn ensure_running(&self) -> Result<bool, KuboError> {
        if self.exists()? {
            if self.is_outdated()? {
                eprintln!(
                    "Note: {} is using an older image. Run `kubo update {}` to upgrade.",
                    self.display_name(),
                    self.display_name()
                );
            }
            if self.is_running()? {
                return Ok(false);
            }
            let status = Command::new("docker")
                .args(["start", &self.name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;

            if !status.success() {
                return Err(KuboError::Container(format!(
                    "failed to start existing container {}",
                    self.display_name()
                )));
            }
            return Ok(false);
        }

        self.create()?;
        Ok(true)
    }

    /// Count the number of *running* exec sessions attached to this container.
    ///
    /// Docker keeps stale exec IDs around after they exit, so we inspect each
    /// one and only count those that are still running.
    pub fn exec_session_count(&self) -> Result<usize, KuboError> {
        let output = Command::new("docker")
            .args(["inspect", "-f", "{{json .ExecIDs}}", &self.name])
            .output()?;

        if !output.status.success() {
            return Ok(0);
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text == "null" || text == "<no value>" || text.is_empty() {
            return Ok(0);
        }

        let exec_ids: Vec<String> = serde_json::from_str(&text).unwrap_or_default();
        let mut running = 0;
        for id in &exec_ids {
            let inspect = Command::new("docker")
                .args(["inspect", "-f", "{{.Running}}", id])
                .output()?;
            let val = String::from_utf8_lossy(&inspect.stdout).trim().to_string();
            if val == "true" {
                running += 1;
            }
        }
        Ok(running)
    }

    /// Update the container: rebuild the image from scratch and recreate.
    /// This fetches the latest versions of all tools (katulong, claude, gh, etc).
    pub fn update(&self, force: bool) -> Result<(), KuboError> {
        if !self.exists()? {
            return Err(KuboError::Container(format!(
                "container {} not found",
                self.display_name()
            )));
        }

        let sessions = self.exec_session_count()?;
        if sessions > 0 && !force {
            return Err(KuboError::Container(format!(
                "{} has {} active session{}. Stop them first or use --force.",
                self.display_name(),
                sessions,
                if sessions == 1 { "" } else { "s" }
            )));
        }

        if sessions > 0 {
            eprintln!(
                "Warning: {} has {} active session{}, forcing update...",
                self.display_name(),
                sessions,
                if sessions == 1 { "" } else { "s" }
            );
        }

        // Rebuild image from scratch (no cache) to get latest tool versions
        eprintln!("Rebuilding kubo image (fetching latest tools)...");
        crate::image::build_image(true)?;

        eprintln!("Recreating {}...", self.display_name());
        self.recreate()?;
        Ok(())
    }

    /// Docker volume name for this container's persistent home directory.
    fn home_volume(&self) -> String {
        format!("{}-home", self.name)
    }

    /// Docker volume name for this container's persistent work directory.
    fn work_volume(&self) -> String {
        format!("{}-work", self.name)
    }

    /// Remove named Docker volumes associated with this container.
    pub fn remove_volumes(&self) -> Result<(), KuboError> {
        for vol in [self.home_volume(), self.work_volume()] {
            let status = Command::new("docker")
                .args(["volume", "rm", "-f", &vol])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;

            if !status.success() {
                return Err(KuboError::Container(format!(
                    "failed to remove volume {vol}"
                )));
            }
        }
        Ok(())
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
        args.extend(["--network".to_string(), "host".to_string()]);

        // Generous memory defaults for dev work (Claude Code needs headroom)
        args.extend([
            "--memory".to_string(),
            "6g".to_string(),
            "--memory-swap".to_string(),
            "12g".to_string(),
        ]);

        // Working directory
        args.extend(["-w".to_string(), "/work".to_string()]);

        // Persistent home volume: preserves ~/.claude, ~/.local, shell history, configs.
        args.extend([
            "-v".to_string(),
            format!("{}:/home/dev", self.home_volume()),
        ]);

        // Work volume: only used for multi-mount containers where bind mounts go to
        // /work/<name> subdirs. Skipped when a bind mount targets /work directly,
        // because Docker Desktop (macOS) doesn't reliably overlay bind mounts nested
        // inside a named volume.
        let has_direct_work_mount = self.mounts.iter().any(|m| m.container_path == "/work");
        if !has_direct_work_mount {
            args.extend(["-v".to_string(), format!("{}:/work", self.work_volume())]);
        }

        // Mount project directories
        for mount in &self.mounts {
            args.extend([
                "-v".to_string(),
                format!("{}:{}", mount.host_path.display(), mount.container_path),
            ]);
        }

        // Mount host credentials and pass git identity
        append_host_credential_args(&mut args);
        append_git_identity_args(&mut args);

        // Pass kubo name so the prompt can show it
        args.extend([
            "-e".to_string(),
            format!("KUBO_NAME={}", self.display_name()),
        ]);

        args.push(IMAGE.to_string());

        let status = Command::new("docker")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to create container {}",
                self.display_name()
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
        let kubo_name_env = format!("KUBO_NAME={}", self.display_name());
        let status = Command::new("docker")
            .args([
                "exec",
                "-it",
                "-u",
                "dev",
                "-e",
                "DISPLAY=:99",
                "-e",
                &kubo_name_env,
                "-w",
                "/work",
                &self.name,
                "zsh",
            ])
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
                self.display_name()
            )));
        }
        Ok(())
    }

    /// Remove the container. If `volumes` is true, also remove persistent volumes.
    pub fn remove(&self, volumes: bool) -> Result<(), KuboError> {
        let status = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to remove container {}",
                self.display_name()
            )));
        }

        if volumes {
            self.remove_volumes()?;
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

    /// Export this container to a portable .kubo archive.
    ///
    /// The archive contains:
    /// - `manifest.json` — name, mounts, image version
    /// - `filesystem.tar` — full container filesystem via `docker export`
    pub fn export(&self, output: &Path) -> Result<(), KuboError> {
        if !self.exists()? {
            return Err(KuboError::Container(format!(
                "container {} not found",
                self.display_name()
            )));
        }

        let tmp = tempfile::tempdir()?;

        // Write manifest
        let manifest = ExportManifest {
            name: self.name.clone(),
            mounts: self
                .mounts
                .iter()
                .map(|m| MountSerde {
                    host: m.host_path.display().to_string(),
                    container: m.container_path.clone(),
                })
                .collect(),
            image_version: crate::image::version().to_string(),
        };
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| KuboError::Container(format!("failed to serialize manifest: {e}")))?;
        std::fs::write(tmp.path().join("manifest.json"), &manifest_json)?;

        // Export container filesystem
        let fs_tar = tmp.path().join("filesystem.tar");
        let fs_file = std::fs::File::create(&fs_tar)?;
        let status = Command::new("docker")
            .args(["export", &self.name])
            .stdout(Stdio::from(fs_file))
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(format!(
                "failed to export container {}",
                self.display_name()
            )));
        }

        // Bundle manifest + filesystem into the .kubo archive
        let output_file = std::fs::File::create(output)?;
        let mut archive = tar::Builder::new(output_file);
        archive.append_path_with_name(tmp.path().join("manifest.json"), "manifest.json")?;
        archive.append_path_with_name(&fs_tar, "filesystem.tar")?;
        archive.finish()?;

        Ok(())
    }

    /// Import a container from a .kubo archive.
    ///
    /// - `archive_path` — path to the .kubo file
    /// - `name` — optional name override (defaults to original name from manifest)
    /// - `dirs` — host directories to mount (remaps the original mount paths)
    pub fn import(
        archive_path: &Path,
        name: Option<&str>,
        dirs: &[PathBuf],
    ) -> Result<Self, KuboError> {
        let tmp = tempfile::tempdir()?;

        // Extract the .kubo archive
        let archive_file = std::fs::File::open(archive_path)
            .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", archive_path.display())))?;
        let mut archive = tar::Archive::new(archive_file);
        archive
            .unpack(tmp.path())
            .map_err(|e| KuboError::Container(format!("failed to extract archive: {e}")))?;

        // Read manifest
        let manifest_path = tmp.path().join("manifest.json");
        let manifest_str = std::fs::read_to_string(&manifest_path)
            .map_err(|_| KuboError::Container("archive missing manifest.json".into()))?;
        let manifest: ExportManifest = serde_json::from_str(&manifest_str)
            .map_err(|e| KuboError::Container(format!("invalid manifest: {e}")))?;

        // Determine container name
        let container_name = match name {
            Some(n) => {
                if n.starts_with("kubo-") {
                    n.to_string()
                } else {
                    format!("kubo-{n}")
                }
            }
            None => manifest.name.clone(),
        };

        // Check name isn't taken
        if Self::name_exists(&container_name)? {
            return Err(KuboError::Container(format!(
                "container {container_name} already exists — use a different name or remove it first"
            )));
        }

        // Import the filesystem tar as a Docker image
        let imported_image = format!("kubo-imported:{}", container_name);
        let fs_tar = tmp.path().join("filesystem.tar");
        let fs_file = std::fs::File::open(&fs_tar)
            .map_err(|_| KuboError::Container("archive missing filesystem.tar".into()))?;
        let output = Command::new("docker")
            .args(["import", "-", &imported_image])
            .stdin(Stdio::from(fs_file))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(KuboError::Container(format!(
                "docker import failed: {stderr}"
            )));
        }

        // Build mount list: use provided dirs, or fall back to manifest container paths
        let mounts: Vec<Mount> = if dirs.is_empty() {
            // No dirs provided — keep container paths from manifest but warn
            manifest
                .mounts
                .iter()
                .map(|m| Mount {
                    host_path: PathBuf::from(&m.host),
                    container_path: m.container.clone(),
                })
                .collect()
        } else {
            // Map provided dirs to container paths from manifest, or generate new ones
            let mut mounts = Vec::new();
            for (i, dir) in dirs.iter().enumerate() {
                let canonical = dir
                    .canonicalize()
                    .map_err(|e| KuboError::InvalidPath(format!("{}: {e}", dir.display())))?;
                if !canonical.is_dir() {
                    return Err(KuboError::InvalidPath(format!(
                        "{} is not a directory",
                        canonical.display()
                    )));
                }
                let container_path = if i < manifest.mounts.len() {
                    manifest.mounts[i].container.clone()
                } else {
                    let dir_name = canonical
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project");
                    format!("/work/{dir_name}")
                };
                mounts.push(Mount {
                    host_path: canonical,
                    container_path,
                });
            }
            mounts
        };

        // Create the container from the imported image
        let mounts_label: Vec<MountSerde> = mounts
            .iter()
            .map(|m| MountSerde {
                host: m.host_path.display().to_string(),
                container: m.container_path.clone(),
            })
            .collect();
        let mounts_json = serde_json::to_string(&mounts_label).unwrap_or_default();

        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            container_name.clone(),
            "--label".to_string(),
            LABEL.to_string(),
            "--label".to_string(),
            format!("kubo.mounts={mounts_json}"),
            "--label".to_string(),
            format!("kubo.imported-from={}", archive_path.display()),
            "-u".to_string(),
            "dev".to_string(),
            "--network".to_string(),
            "host".to_string(),
            "--memory".to_string(),
            "6g".to_string(),
            "--memory-swap".to_string(),
            "12g".to_string(),
            "-w".to_string(),
            "/work".to_string(),
            // The imported image has no CMD/ENTRYPOINT, so we need to set one
            "--entrypoint".to_string(),
            "/bin/sh".to_string(),
        ];

        // Mount directories
        for mount in &mounts {
            args.extend([
                "-v".to_string(),
                format!("{}:{}", mount.host_path.display(), mount.container_path),
            ]);
        }

        // Mount host credentials and pass git identity
        append_host_credential_args(&mut args);
        append_git_identity_args(&mut args);

        args.push(imported_image.clone());
        // Keep the container alive
        args.extend(["-c".to_string(), "sleep infinity".to_string()]);

        let status = Command::new("docker")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            return Err(KuboError::Container(
                "failed to create container from imported image".to_string(),
            ));
        }

        Ok(Self {
            name: container_name,
            mounts,
        })
    }

    /// Save pending mount configuration inside the running container.
    ///
    /// The file is written to the persistent home volume so it survives
    /// container restarts but is accessible from inside the container.
    pub fn save_pending_mounts(&self, mounts: &[Mount]) -> Result<(), KuboError> {
        let serde_mounts: Vec<MountSerde> = mounts
            .iter()
            .map(|m| MountSerde {
                host: m.host_path.display().to_string(),
                container: m.container_path.clone(),
            })
            .collect();
        let json = serde_json::to_string_pretty(&serde_mounts)
            .map_err(|e| KuboError::Container(format!("failed to serialize mounts: {e}")))?;

        let mut child = Command::new("docker")
            .args([
                "exec",
                "-i",
                &self.name,
                "sh",
                "-c",
                &format!("mkdir -p /home/dev/.kubo && cat > {PENDING_MOUNTS_PATH}"),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(json.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(KuboError::Container("failed to save pending mounts".into()));
        }
        Ok(())
    }

    /// Read pending mount configuration from inside the container, if any.
    pub fn read_pending_mounts(&self) -> Result<Option<Vec<Mount>>, KuboError> {
        let output = Command::new("docker")
            .args(["exec", &self.name, "cat", PENDING_MOUNTS_PATH])
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let json = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if json.is_empty() {
            return Ok(None);
        }

        let mounts = serde_json::from_str::<Vec<MountSerde>>(&json)
            .map_err(|e| KuboError::Container(format!("bad pending mounts: {e}")))?
            .into_iter()
            .map(|m| Mount {
                host_path: PathBuf::from(m.host),
                container_path: m.container,
            })
            .collect();

        Ok(Some(mounts))
    }

    /// Remove the pending mounts file from inside the container.
    pub fn clear_pending_mounts(&self) -> Result<(), KuboError> {
        let _ = Command::new("docker")
            .args(["exec", &self.name, "rm", "-f", PENDING_MOUNTS_PATH])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
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
                "{{.Names}}\t{{.Status}}\t{{.Label \"kubo.mounts\"}}\t{{.Label \"kubo.image-version\"}}",
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
                let parts: Vec<&str> = line.splitn(4, '\t').collect();
                if parts.len() >= 2 {
                    let mounts_json = parts.get(2).unwrap_or(&"");
                    let mount_paths: Vec<String> =
                        serde_json::from_str::<Vec<MountSerde>>(mounts_json)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|m| m.host)
                            .collect();
                    let name = parts[0].to_string();
                    let status = parts[1].to_string();
                    let image_version = parts.get(3).unwrap_or(&"").to_string();
                    let is_running = status.starts_with("Up");

                    // Count active exec sessions for running containers
                    let active_sessions = if is_running {
                        let c = Container {
                            name: name.clone(),
                            mounts: Vec::new(),
                        };
                        c.exec_session_count().unwrap_or(0)
                    } else {
                        0
                    };

                    Some(ContainerStatus {
                        name,
                        status,
                        mounts: mount_paths,
                        active_sessions,
                        running: is_running,
                        image_version,
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
