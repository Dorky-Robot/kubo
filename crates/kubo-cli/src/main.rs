use std::path::PathBuf;

use clap::{Parser, Subcommand};
use kubo_core::{Container, ContainerStatus};

#[derive(Parser)]
#[command(name = "kubo", about = "Isolated dev environments in Docker")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Directory to open in an isolated container
    path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
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
    /// Build or rebuild the kubo Docker image
    Build,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::Ls) => cmd_ls(),
        Some(Command::Stop { name }) => cmd_stop(&name),
        Some(Command::Rm { name }) => cmd_rm(&name),
        Some(Command::Build) => cmd_build(),
        None => match cli.path {
            Some(path) => cmd_open(&path),
            None => {
                eprintln!("Usage: kubo <directory>");
                eprintln!("       kubo ls");
                eprintln!("       kubo stop <name>");
                eprintln!("       kubo rm <name>");
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

fn ensure_image() -> Result<(), Box<dyn std::error::Error>> {
    if Container::image_exists()? {
        return Ok(());
    }

    eprintln!("kubo image not found, building...");

    // Find the image directory relative to the kubo binary or source
    let image_dir = find_image_dir()?;
    Container::build_image(&image_dir)?;
    eprintln!("Image built successfully.");
    Ok(())
}

fn find_image_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Check next to the binary first (installed layout)
    if let Ok(exe) = std::env::current_exe() {
        let beside_exe = exe.parent().unwrap_or(exe.as_path()).join("kubo-image");
        if beside_exe.join("Dockerfile").exists() {
            return Ok(beside_exe);
        }
    }

    // Check KUBO_IMAGE_DIR env var
    if let Ok(dir) = std::env::var("KUBO_IMAGE_DIR") {
        let p = PathBuf::from(&dir);
        if p.join("Dockerfile").exists() {
            return Ok(p);
        }
    }

    // Walk up from CWD looking for kubo repo with image/ dir
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join("image");
        if candidate.join("Dockerfile").exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    Err("cannot find kubo image directory. Set KUBO_IMAGE_DIR or run from the kubo repo.".into())
}

fn cmd_open(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;
    ensure_image()?;

    let container = Container::from_path(path)?;

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

    println!("{:<24} {:<20} PATH", "NAME", "STATUS");
    for ContainerStatus {
        name,
        status,
        host_path,
    } in &containers
    {
        println!("{:<24} {:<20} {}", name, status, host_path);
    }

    Ok(())
}

fn cmd_stop(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container {
        name: name.to_string(),
        host_path: PathBuf::new(),
    };
    container.stop()?;
    println!("Stopped {name}");
    Ok(())
}

fn cmd_rm(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;

    let container = Container {
        name: name.to_string(),
        host_path: PathBuf::new(),
    };
    container.remove()?;
    println!("Removed {name}");
    Ok(())
}

fn cmd_build() -> Result<(), Box<dyn std::error::Error>> {
    Container::check_docker()?;
    let image_dir = find_image_dir()?;
    eprintln!("Building kubo image from {}...", image_dir.display());
    Container::build_image(&image_dir)?;
    eprintln!("Done.");
    Ok(())
}
