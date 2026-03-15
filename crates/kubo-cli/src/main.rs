use std::path::PathBuf;

use clap::{Parser, Subcommand};
use kubo_core::{Container, ContainerStatus};

#[derive(Parser)]
#[command(name = "kubo", about = "Isolated dev environments in Docker")]
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
    /// Force rebuild the kubo Docker image
    Build,
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
        Some(Command::Build) => cmd_build(),
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
    kubo_core::image::ensure_image()?;

    let path = PathBuf::from(target);

    // If it's an existing directory, use single-dir mode
    if path.is_dir() {
        let container = Container::from_path(&path)?;
        return open_container(container);
    }

    // Otherwise treat it as a kubo name
    if Container::name_exists(target)? {
        let container = Container::load(target)?;
        return open_container(container);
    }

    // Not a dir and not an existing kubo
    Err(format!("'{target}' is not a directory or existing kubo").into())
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
            container.name,
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
            container.name,
            sessions,
            if sessions == 1 { "" } else { "s" }
        );
    }

    eprintln!("Recreating {} with new mounts...", container.name);
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
        eprintln!("Created container: {}", container.name);
    } else {
        eprintln!("Attaching to container: {}", container.name);
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

    println!("{:<24} {:<20} MOUNTS", "NAME", "STATUS");
    for ContainerStatus {
        name,
        status,
        mounts,
    } in &containers
    {
        let mount_str = if mounts.is_empty() {
            "-".to_string()
        } else {
            mounts.join(", ")
        };
        println!("{:<24} {:<20} {}", name, status, mount_str);
    }

    Ok(())
}

fn cmd_stop(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.stop()?;
    println!("Stopped {}", container.name);
    Ok(())
}

fn cmd_rm(name: &str, volumes: bool) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.remove(volumes)?;
    println!("Removed {}", container.name);
    if volumes {
        println!("Removed persistent volumes");
    }
    Ok(())
}

fn cmd_update(name: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;
    kubo_core::image::ensure_image()?;

    let container = Container::load(name)?;
    container.update(force)?;
    eprintln!("Done. Run `kubo {}` to attach.", name);
    Ok(())
}

fn cmd_export(
    name: &str,
    output: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    let default_output = PathBuf::from(format!(
        "{}.kubo",
        container
            .name
            .strip_prefix("kubo-")
            .unwrap_or(&container.name)
    ));
    let output_path = output.unwrap_or(&default_output);

    eprintln!(
        "Exporting {} → {} ...",
        container.name,
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
    eprintln!("Created container: {}", container.name);

    if dirs.is_empty() {
        eprintln!(
            "Note: using original mount paths from export. If those paths don't exist on this \
             machine, re-import with --dir to specify local directories."
        );
    }

    eprintln!(
        "Run `kubo {}` to attach.",
        container
            .name
            .strip_prefix("kubo-")
            .unwrap_or(&container.name)
    );
    Ok(())
}

fn cmd_build() -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;
    kubo_core::image::build_image()?;
    eprintln!("Done.");
    Ok(())
}

fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("kubo {}", kubo_core::image::version());
    Ok(())
}
