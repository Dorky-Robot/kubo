use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use clap::{Parser, Subcommand};
use kubo_core::{Container, ContainerStatus};

#[derive(Parser)]
#[command(
    name = "kubo",
    about = "Isolated dev environments in Docker",
    version,
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Directory or kubo name to open
    target: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Create a named kubo with one or more directories
    New {
        /// Name for the kubo
        name: String,
        /// Directories to mount
        dirs: Vec<PathBuf>,
    },
    /// Add directories to an existing kubo
    Add {
        /// Target kubo name
        name: String,
        /// Directories to add
        dirs: Vec<PathBuf>,
        /// Force recreate even if there are active sessions
        #[arg(long)]
        force: bool,
    },
    /// Remove directories from an existing kubo
    Detach {
        /// Target kubo name
        name: String,
        /// Directories to remove
        dirs: Vec<PathBuf>,
        /// Force recreate even if there are active sessions
        #[arg(long)]
        force: bool,
    },
    /// List all kubo containers
    Ls,
    /// Stop a running kubo container
    Stop {
        /// Container name (e.g. kubo-myproject)
        name: String,
    },
    /// Restart a kubo container
    Restart {
        /// Container name
        name: String,
    },
    /// Remove a kubo container
    Rm {
        /// Container name (e.g. kubo-myproject)
        name: String,
        /// Also remove persistent volumes (home dir, work dir data)
        #[arg(long)]
        volumes: bool,
    },
    /// Update a kubo container to the latest image
    Update {
        /// Container name
        name: String,
        /// Force update even if there are active sessions
        #[arg(long)]
        force: bool,
    },
    /// Export a kubo to a portable .kubo file
    Export {
        /// Container name
        name: String,
        /// Output file path (defaults to <name>.kubo)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Import a kubo from a .kubo file
    Import {
        /// Path to the .kubo archive
        file: PathBuf,
        /// Override the container name
        #[arg(short, long)]
        name: Option<String>,
        /// Directories to mount (replaces original mount paths)
        #[arg(short, long)]
        dir: Vec<PathBuf>,
    },
    /// Rebuild image (no cache) and update all running containers
    Refresh,
    /// Force rebuild the kubo Docker image
    Build {
        /// Skip Docker layer cache (fetch latest remote tools)
        #[arg(long)]
        no_cache: bool,
    },
    /// Upgrade kubo to the latest release
    Upgrade,
    /// Show kubo version and image info
    Version,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::New { name, dirs }) => cmd_new(&name, &dirs),
        Some(Command::Add { name, dirs, force }) => cmd_add(&name, &dirs, force),
        Some(Command::Detach { name, dirs, force }) => cmd_detach(&name, &dirs, force),
        Some(Command::Ls) => cmd_ls(),
        Some(Command::Stop { name }) => cmd_stop(&name),
        Some(Command::Restart { name }) => cmd_restart(&name),
        Some(Command::Rm { name, volumes }) => cmd_rm(&name, volumes),
        Some(Command::Update { name, force }) => cmd_update(&name, force),
        Some(Command::Export { name, output }) => cmd_export(&name, output.as_deref()),
        Some(Command::Import { file, name, dir }) => cmd_import(&file, name.as_deref(), &dir),
        Some(Command::Refresh) => cmd_refresh(),
        Some(Command::Build { no_cache }) => cmd_build(no_cache),
        Some(Command::Upgrade) => cmd_upgrade(),
        Some(Command::Version) => cmd_version(),
        None => match cli.target {
            Some(target) => cmd_open(&target),
            None => {
                eprintln!("Usage: kubo <dir>                    open dir in a container");
                eprintln!("       kubo <name>                   attach to named kubo");
                eprintln!("       kubo new <name> <dirs...>     create named kubo");
                eprintln!("       kubo add <name> <dirs...>     add dirs to existing kubo");
                eprintln!("       kubo detach <name> <dirs...>  remove dirs from a kubo");
                eprintln!("       kubo ls                       list containers");
                eprintln!("       kubo stop/rm <name>           manage containers");
                eprintln!(
                    "       kubo refresh                  rebuild image + update all containers"
                );
                eprintln!("       kubo upgrade                  upgrade kubo to latest release");
                eprintln!("       kubo export <name>            export kubo to portable file");
                eprintln!("       kubo import <file>            import kubo from file");
                eprintln!("\nTry: kubo .");
                std::process::exit(1);
            }
        },
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Smart open: if target is a directory, open it (single-dir kubo).
/// If it's a name matching an existing container, attach to it.
fn cmd_open(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let path = PathBuf::from(target);

    if path.is_dir() {
        // It's a directory — need image to create/run container
        kubo_core::image::ensure_image()?;
        let container = Container::from_path(&path)?;
        return open_container(container);
    }

    // Otherwise treat it as a kubo name — fail fast if it doesn't exist
    if !Container::name_exists(target)? {
        return Err(format!("'{target}' is not a directory or existing kubo").into());
    }

    kubo_core::image::ensure_image()?;
    let container = Container::load(target)?;
    open_container(container)
}

fn cmd_new(name: &str, dirs: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    if dirs.is_empty() {
        return Err("provide at least one directory".into());
    }

    Container::check_docker()?;
    kubo_core::image::ensure_image()?;

    let container = Container::new(name, dirs)?;
    open_container(container)
}

fn cmd_add(name: &str, dirs: &[PathBuf], force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if dirs.is_empty() {
        return Err("provide at least one directory".into());
    }

    Container::check_docker()?;
    kubo_core::image::ensure_image()?;

    let mut container = Container::load(name)?;
    for dir in dirs {
        container.add_mount(dir)?;
    }

    let sessions = container.exec_session_count()?;
    if sessions > 0 && !force {
        // Defer: save pending mounts for next idle attach
        container.save_pending_mounts(&container.mounts)?;
        eprintln!(
            "{} has {} active session{}. New mounts will apply when all sessions disconnect.",
            container.display_name(),
            sessions,
            if sessions == 1 { "" } else { "s" }
        );
        eprintln!(
            "Or use `kubo add --force {} {}` to recreate now.",
            name,
            dirs.iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
        return Ok(());
    }

    if sessions > 0 {
        eprintln!(
            "Warning: {} has {} active session{}, forcing recreate...",
            container.display_name(),
            sessions,
            if sessions == 1 { "" } else { "s" }
        );
    }

    eprintln!("Recreating {} with new mounts...", container.display_name());
    container.recreate()?;
    // Clear any stale pending mounts from the home volume
    container.clear_pending_mounts()?;
    open_container(container)
}

fn cmd_detach(name: &str, dirs: &[PathBuf], force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if dirs.is_empty() {
        return Err("provide at least one directory to detach".into());
    }

    Container::check_docker()?;

    let mut container = Container::load(name)?;
    for dir in dirs {
        container.remove_mount(dir)?;
    }

    let sessions = container.exec_session_count()?;
    if sessions > 0 && !force {
        container.save_pending_mounts(&container.mounts)?;
        eprintln!(
            "{} has {} active session{}. Mount changes will apply when all sessions disconnect.",
            container.display_name(),
            sessions,
            if sessions == 1 { "" } else { "s" }
        );
        eprintln!(
            "Or use `kubo detach --force {} {}` to recreate now.",
            name,
            dirs.iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
        return Ok(());
    }

    if sessions > 0 {
        eprintln!(
            "Warning: {} has {} active session{}, forcing recreate...",
            container.display_name(),
            sessions,
            if sessions == 1 { "" } else { "s" }
        );
    }

    eprintln!(
        "Recreating {} without detached mounts...",
        container.display_name()
    );
    container.recreate()?;
    container.clear_pending_mounts()?;
    eprintln!("Done.");
    Ok(())
}

fn open_container(mut container: Container) -> Result<(), Box<dyn std::error::Error>> {
    let created = container.ensure_running()?;

    // Check for deferred mount changes (from `kubo add` while sessions were active)
    match container.read_pending_mounts() {
        Ok(Some(pending)) => {
            let sessions = container.exec_session_count().unwrap_or(0);
            if sessions == 0 {
                // No other sessions — safe to apply
                if pending != container.mounts {
                    eprintln!("Applying deferred mount changes...");
                    container.mounts = pending;
                    container.recreate()?;
                }
                container.clear_pending_mounts()?;
            } else {
                eprintln!(
                    "Note: pending mount changes waiting. {} other session{} must disconnect first.",
                    sessions,
                    if sessions == 1 { "" } else { "s" }
                );
            }
        }
        Err(e) => {
            eprintln!("Warning: corrupt pending-mounts file, clearing: {e}");
            container.clear_pending_mounts()?;
        }
        Ok(None) => {}
    }

    if created {
        eprintln!("Created container: {}", container.display_name());
    } else {
        eprintln!("Attaching to container: {}", container.display_name());
    }

    // Check for updates in the background while the user is in the shell
    let update_rx = spawn_update_check();

    let status = container.exec_shell()?;

    // After the shell exits, show a hint if a newer version was found
    if let Some(rx) = update_rx
        && let Ok(Some(latest)) = rx.try_recv()
    {
        eprintln!("\n  kubo v{latest} available — run `kubo upgrade` to update\n");
    }

    std::process::exit(status.code().unwrap_or(1));
}

fn cmd_ls() -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let containers = Container::list_all()?;

    if containers.is_empty() {
        println!("No kubo containers. Try: kubo .");
        return Ok(());
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let term_width = terminal_width();

    // Partition into running and stopped
    let (running, stopped): (Vec<_>, Vec<_>) = containers.iter().partition(|c| c.running);

    let name_w = containers
        .iter()
        .map(|c| c.display_name().len())
        .max()
        .unwrap_or(4)
        .max(4);

    let print_container = |c: &ContainerStatus| {
        let icon = if c.active_sessions > 0 {
            "\u{25cf}" // filled — active sessions
        } else if c.running {
            "\u{25cb}" // hollow — running but idle
        } else {
            "\u{25ab}" // small square — stopped
        };
        let name = c.display_name();

        // Show version — just the semver part, with an indicator if outdated
        let current_ver = kubo_core::image::version();
        let ver_display = if c.image_version.is_empty() {
            "?".to_string()
        } else if c.image_version == current_ver {
            // Extract just the semver (before the hash)
            c.image_version
                .split('-')
                .next()
                .unwrap_or(&c.image_version)
                .to_string()
        } else {
            let ver = c
                .image_version
                .split('-')
                .next()
                .unwrap_or(&c.image_version);
            format!("{ver} \u{2191}") // ↑ = update available
        };

        if c.mounts.is_empty() {
            println!("{icon} {name:<name_w$}  v{ver_display}");
            return;
        }

        let shorten = |p: &str| -> String {
            if !home.is_empty()
                && let Some(rest) = p.strip_prefix(&home)
            {
                return format!("~{rest}");
            }
            p.to_string()
        };

        if c.mounts.len() == 1 {
            println!(
                "{icon} {name:<name_w$}  {}  v{ver_display}",
                shorten(&c.mounts[0])
            );
            return;
        }

        // Multiple mounts: find common prefix, show root then project names
        let prefix = common_dir_prefix(&c.mounts);
        // indent for wrapped lines: "● " + name + "  "
        let indent = 2 + name_w + 2;

        if prefix.components().count() >= 2 {
            let prefix_display = shorten(&prefix.to_string_lossy());
            println!("{icon} {name:<name_w$}  {prefix_display}  v{ver_display}");

            let suffixes: Vec<String> = c
                .mounts
                .iter()
                .map(|m| {
                    let prefix_str = prefix.to_string_lossy();
                    m.strip_prefix(prefix_str.as_ref())
                        .and_then(|s| s.strip_prefix('/'))
                        .unwrap_or(m.as_str())
                        .to_string()
                })
                .collect();

            // Word-wrap the project names to terminal width
            let available = term_width.saturating_sub(indent);
            let mut line = String::new();
            for (i, name) in suffixes.iter().enumerate() {
                let sep = if i > 0 { ", " } else { "" };
                if !line.is_empty() && line.len() + sep.len() + name.len() > available {
                    println!("{:indent$}{line}", "");
                    line = name.clone();
                } else {
                    line.push_str(sep);
                    line.push_str(name);
                }
            }
            if !line.is_empty() {
                println!("{:indent$}{line}", "");
            }
        } else {
            // No useful common prefix, just list paths
            let paths: Vec<String> = c.mounts.iter().map(|m| shorten(m)).collect();
            println!(
                "{icon} {name:<name_w$}  {}  v{ver_display}",
                paths.join(", ")
            );
        }
    };

    for c in &running {
        print_container(c);
    }
    if !running.is_empty() && !stopped.is_empty() {
        println!();
    }
    for c in &stopped {
        print_container(c);
    }

    Ok(())
}

/// Find the longest common directory prefix among a set of paths.
fn common_dir_prefix(paths: &[String]) -> std::path::PathBuf {
    paths
        .iter()
        .map(std::path::PathBuf::from)
        .reduce(|acc, p| {
            acc.components()
                .zip(p.components())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect()
        })
        .unwrap_or_default()
}

/// Best-effort terminal width, falls back to 80.
fn terminal_width() -> usize {
    // Try the COLUMNS env var first, then default
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80)
}

fn cmd_stop(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.stop()?;
    println!("Stopped {}", container.display_name());
    Ok(())
}

fn cmd_restart(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    if container.is_running()? {
        container.stop()?;
    }
    container.ensure_running()?;
    eprintln!("Restarted {}", container.display_name());
    Ok(())
}

fn cmd_rm(name: &str, volumes: bool) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.remove(volumes)?;
    println!("Removed {}", container.display_name());
    if volumes {
        println!("Removed persistent volumes");
    }
    Ok(())
}

fn cmd_update(name: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.update(force)?;
    open_container(container)
}

fn cmd_export(
    name: &str,
    output: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    let default_output = PathBuf::from(format!("{}.kubo", container.display_name()));
    let output_path = output.unwrap_or(&default_output);

    eprintln!(
        "Exporting {} → {} ...",
        container.display_name(),
        output_path.display()
    );
    container.export(output_path)?;

    let size = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);
    let size_mb = size as f64 / 1_048_576.0;
    eprintln!(
        "Exported ({:.1} MB). Copy this file anywhere and `kubo import` it.",
        size_mb
    );

    Ok(())
}

fn cmd_import(
    file: &std::path::Path,
    name: Option<&str>,
    dirs: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    eprintln!("Importing from {} ...", file.display());
    let container = Container::import(file, name, dirs)?;
    eprintln!("Created container: {}", container.display_name());

    if dirs.is_empty() {
        eprintln!(
            "Note: using original mount paths from export. If those paths don't exist on this \
             machine, re-import with --dir to specify local directories."
        );
    }

    eprintln!("Run `kubo {}` to attach.", container.display_name());
    Ok(())
}

fn cmd_refresh() -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    // Rebuild image from scratch (no cache) to pick up latest remote tools
    eprintln!("Rebuilding kubo image (no cache)...");
    kubo_core::image::build_image(true)?;
    eprintln!("Image rebuilt.");

    // Update all running containers to the new image
    let containers = Container::list_all()?;
    let running: Vec<_> = containers.iter().filter(|c| c.running).collect();

    if running.is_empty() {
        eprintln!("No running containers to update.");
        return Ok(());
    }

    for c in &running {
        let container = Container::load(&c.name)?;
        let sessions = container.exec_session_count()?;
        if sessions > 0 {
            eprintln!(
                "Skipping {} ({} active session{}) — stop sessions first or use `kubo update --force {}`.",
                container.display_name(),
                sessions,
                if sessions == 1 { "" } else { "s" },
                container.display_name(),
            );
            continue;
        }
        eprintln!("Updating {}...", container.display_name());
        container.recreate()?;
        // Start the recreated container
        container.ensure_running()?;
        eprintln!("  {} updated.", container.display_name());
    }

    eprintln!("All containers refreshed.");
    Ok(())
}

/// Spawn a background thread to check for a newer kubo release.
/// Returns a receiver that yields `Some(latest_version)` if an update is
/// available, or `None` if already current. The check is skipped entirely
/// if we already checked within the last 24 hours.
fn spawn_update_check() -> Option<mpsc::Receiver<Option<String>>> {
    let cache_path = dirs();
    let cache_file = cache_path.join("update-check");

    // Rate-limit: skip if checked within the last 24 hours
    if let Ok(meta) = std::fs::metadata(&cache_file)
        && let Ok(modified) = meta.modified()
        && modified.elapsed().unwrap_or_default() < std::time::Duration::from_secs(86400)
    {
        // Still fresh — read cached result
        if let Ok(cached) = std::fs::read_to_string(&cache_file) {
            let latest = cached.trim().to_string();
            let current = env!("CARGO_PKG_VERSION");
            if !latest.is_empty() && latest != current {
                let (tx, rx) = mpsc::channel();
                let _ = tx.send(Some(latest));
                return Some(rx);
            }
        }
        return None;
    }

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = (|| -> Option<String> {
            let output = std::process::Command::new("curl")
                .args([
                    "-fsSL",
                    "--connect-timeout",
                    "3",
                    "--max-time",
                    "5",
                    "https://api.github.com/repos/Dorky-Robot/kubo/releases/latest",
                ])
                .output()
                .ok()?;

            if !output.status.success() {
                return None;
            }

            let body = String::from_utf8_lossy(&output.stdout);
            let tag = body
                .lines()
                .find(|l| l.contains("\"tag_name\""))
                .and_then(|l| l.split('"').nth(3))?;

            let latest = tag.strip_prefix('v').unwrap_or(tag).to_string();

            // Cache the result
            let _ = std::fs::create_dir_all(&cache_path);
            let _ = std::fs::write(&cache_file, &latest);

            let current = env!("CARGO_PKG_VERSION");
            if latest != current {
                Some(latest)
            } else {
                None
            }
        })();

        let _ = tx.send(result);
    });

    Some(rx)
}

/// kubo cache directory (~/.cache/kubo)
fn dirs() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cache").join("kubo")
}

fn cmd_upgrade() -> Result<(), Box<dyn std::error::Error>> {
    let current = env!("CARGO_PKG_VERSION");

    // Fetch latest release tag from GitHub
    eprintln!("Checking for updates...");
    let output = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "https://api.github.com/repos/Dorky-Robot/kubo/releases/latest",
        ])
        .output()?;

    if !output.status.success() {
        return Err("failed to check for updates — could not reach GitHub".into());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let tag = body
        .lines()
        .find(|l| l.contains("\"tag_name\""))
        .and_then(|l| l.split('"').nth(3))
        .ok_or("could not parse latest release tag")?
        .to_string();

    let latest = tag.strip_prefix('v').unwrap_or(&tag);

    if latest == current {
        eprintln!("Already on the latest version (v{current}).");
        return Ok(());
    }

    eprintln!("Upgrading v{current} → v{latest}...");

    // Detect platform
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let target = match (arch, os) {
        ("x86_64", "linux") => "x86_64-unknown-linux-musl",
        ("aarch64", "linux") => "aarch64-unknown-linux-musl",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        _ => return Err(format!("unsupported platform: {arch}-{os}").into()),
    };

    let url = format!(
        "https://github.com/Dorky-Robot/kubo/releases/download/{tag}/kubo-{tag}-{target}.tar.gz"
    );

    // Download and extract to temp dir
    let tmpdir = std::env::temp_dir().join(format!("kubo-upgrade-{}", std::process::id()));
    std::fs::create_dir_all(&tmpdir)?;

    let tarball = tmpdir.join("kubo.tar.gz");
    let dl = std::process::Command::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(&tarball)
        .status()?;

    if !dl.success() {
        std::fs::remove_dir_all(&tmpdir).ok();
        return Err(format!("failed to download {url}").into());
    }

    let extract = std::process::Command::new("tar")
        .args(["xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(&tmpdir)
        .status()?;

    if !extract.success() {
        std::fs::remove_dir_all(&tmpdir).ok();
        return Err("failed to extract archive".into());
    }

    // Find the extracted binary
    let extracted = tmpdir.join(format!("kubo-{tag}-{target}")).join("kubo");
    if !extracted.exists() {
        std::fs::remove_dir_all(&tmpdir).ok();
        return Err("kubo binary not found in archive".into());
    }

    // Replace current binary
    let current_exe = std::env::current_exe()?;
    let install_dir = current_exe
        .parent()
        .ok_or("could not determine install directory")?;

    // Check if we can write directly or need sudo
    let dest = install_dir.join("kubo");
    let needs_sudo = !is_writable(&dest);

    if needs_sudo {
        eprintln!("Installing to {} (requires sudo)...", install_dir.display());
        let mv = std::process::Command::new("sudo")
            .args(["cp", "-f"])
            .arg(&extracted)
            .arg(&dest)
            .status()?;

        if !mv.success() {
            std::fs::remove_dir_all(&tmpdir).ok();
            return Err("failed to install binary (sudo cp failed)".into());
        }

        std::process::Command::new("sudo")
            .args(["chmod", "+x"])
            .arg(&dest)
            .status()?;
    } else {
        std::fs::copy(&extracted, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    std::fs::remove_dir_all(&tmpdir).ok();
    eprintln!("Upgraded to v{latest}.");
    Ok(())
}

/// Check if the parent directory is writable by the current user.
fn is_writable(path: &std::path::Path) -> bool {
    // Test the directory, not the file — opening a running binary for writing
    // fails on macOS even if you own it.
    let dir = match path.parent() {
        Some(d) => d,
        None => return false,
    };
    let probe = dir.join(".kubo-write-test");
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe)
        .map(|_| {
            std::fs::remove_file(&probe).ok();
            true
        })
        .unwrap_or(false)
}

fn cmd_build(no_cache: bool) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;
    kubo_core::image::build_image(no_cache)?;
    eprintln!("Done.");
    Ok(())
}

fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("kubo {}", kubo_core::image::version());
    Ok(())
}
