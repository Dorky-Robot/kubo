use std::path::PathBuf;

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
    /// List all kubo containers
    Ls,
    /// Stop a running kubo container
    Stop {
        /// Container name (e.g. kubo-myproject)
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
    /// Show kubo version and image info
    Version,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::New { name, dirs }) => cmd_new(&name, &dirs),
        Some(Command::Add { name, dirs, force }) => cmd_add(&name, &dirs, force),
        Some(Command::Ls) => cmd_ls(),
        Some(Command::Stop { name }) => cmd_stop(&name),
        Some(Command::Rm { name, volumes }) => cmd_rm(&name, volumes),
        Some(Command::Update { name, force }) => cmd_update(&name, force),
        Some(Command::Export { name, output }) => cmd_export(&name, output.as_deref()),
        Some(Command::Import { file, name, dir }) => cmd_import(&file, name.as_deref(), &dir),
        Some(Command::Refresh) => cmd_refresh(),
        Some(Command::Build { no_cache }) => cmd_build(no_cache),
        Some(Command::Version) => cmd_version(),
        None => match cli.target {
            Some(target) => cmd_open(&target),
            None => {
                eprintln!("Usage: kubo <dir>                    open dir in a container");
                eprintln!("       kubo <name>                   attach to named kubo");
                eprintln!("       kubo new <name> <dirs...>     create named kubo");
                eprintln!("       kubo add <name> <dirs...>     add dirs to existing kubo");
                eprintln!("       kubo ls                       list containers");
                eprintln!("       kubo stop/rm <name>           manage containers");
                eprintln!(
                    "       kubo refresh                  rebuild image + update all containers"
                );
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

fn open_container(mut container: Container) -> Result<(), Box<dyn std::error::Error>> {
    let created = container.ensure_running()?;

    // Check for deferred mount changes (from `kubo add` while sessions were active)
    if let Ok(Some(pending)) = container.read_pending_mounts() {
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

    if created {
        eprintln!("Created container: {}", container.display_name());
    } else {
        eprintln!("Attaching to container: {}", container.display_name());
    }

    let status = container.exec_shell()?;
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
        eprintln!("Updating {}...", container.display_name());
        container.recreate()?;
        // Start the recreated container
        container.ensure_running()?;
        eprintln!("  {} updated.", container.display_name());
    }

    eprintln!("All containers refreshed.");
    Ok(())
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
