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
        Some(Command::Add { name, dirs }) => cmd_add(&name, &dirs),
        Some(Command::Ls) => cmd_ls(),
        Some(Command::Stop { name }) => cmd_stop(&name),
        Some(Command::Rm { name }) => cmd_rm(&name),
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

fn cmd_add(name: &str, dirs: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    if dirs.is_empty() {
        return Err("provide at least one directory".into());
    }

    Container::check_docker()?;
    kubo_core::image::ensure_image()?;

    let mut container = Container::load(name)?;
    for dir in dirs {
        container.add_mount(dir)?;
    }

    eprintln!("Recreating {} with new mounts...", container.name);
    container.recreate()?;
    open_container(container)
}

fn open_container(container: Container) -> Result<(), Box<dyn std::error::Error>> {
    let created = container.ensure_running()?;
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

fn cmd_rm(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container::load(name)?;
    container.remove()?;
    println!("Removed {}", container.name);
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
