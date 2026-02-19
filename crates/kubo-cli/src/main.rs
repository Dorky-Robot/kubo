use clap::{Parser, Subcommand};
use kubo_core::catalog::Catalog;
use kubo_core::claude::ClaudeGenerator;
use kubo_core::generator::Generator;
use kubo_core::intent::Intent;

#[derive(Parser)]
#[command(name = "kubo", about = "State what you want, get a pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Natural language intent (shorthand for `kubo do "..."`)
    intent: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate or find a pipeline from intent
    Do {
        /// Natural language intent
        intent: String,
        /// Show the chain without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// List all saved chains
    List,
    /// Show details of a saved chain
    Show {
        /// Chain name
        name: String,
    },
    /// Re-run an existing chain
    Run {
        /// Chain name
        name: String,
    },
    /// Delete a saved chain
    Rm {
        /// Chain name
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(cmd) => run_command(cmd),
        None => match cli.intent {
            Some(intent) => run_intent(&intent, false),
            None => {
                eprintln!("Usage: kubo \"what you want\" or kubo <command>");
                eprintln!("Try: kubo --help");
                std::process::exit(1);
            }
        },
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run_command(cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Command::Do { intent, dry_run } => run_intent(&intent, dry_run),
        Command::List => cmd_list(),
        Command::Show { name } => cmd_show(&name),
        Command::Run { name } => cmd_run(&name),
        Command::Rm { name } => cmd_rm(&name),
    }
}

fn run_intent(text: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::open()?;

    // Search for an existing chain
    let matches = catalog.search(text)?;
    if let Some(chain) = matches.first() {
        println!("Found existing chain: {}", chain.chain.name);
        print_chain(chain);
        if !dry_run {
            execute_chain(chain);
        }
        return Ok(());
    }

    // Generate a new chain
    println!("No matching chain found. Generating...");
    let generator = ClaudeGenerator::from_env()?;
    let intent = Intent {
        text: text.to_string(),
    };
    let chain = generator.generate(&intent)?;

    println!("\nGenerated chain: {}", chain.chain.name);
    print_chain(&chain);

    // Save to catalog
    let path = catalog.save(&chain)?;
    println!("\nSaved to: {}", path.display());

    if !dry_run {
        execute_chain(&chain);
    }

    Ok(())
}

fn cmd_list() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::open()?;
    let chains = catalog.list()?;

    if chains.is_empty() {
        println!("No chains saved yet. Try: kubo \"what should we get for dinner?\"");
        return Ok(());
    }

    println!("{:<24} INTENT", "NAME");
    println!("{:<24} ------", "----");
    for chain in &chains {
        let intent_preview = if chain.chain.intent.len() > 50 {
            format!("{}...", &chain.chain.intent[..50])
        } else {
            chain.chain.intent.clone()
        };
        println!("{:<24} {}", chain.chain.name, intent_preview);
    }
    println!("\n{} chain(s)", chains.len());

    Ok(())
}

fn cmd_show(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::open()?;
    match catalog.load(name)? {
        Some(chain) => {
            print_chain(&chain);
            println!("\n--- Raw TOML ---");
            println!("{}", toml::to_string_pretty(&chain)?);
        }
        None => {
            eprintln!("Chain '{name}' not found");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_run(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::open()?;
    match catalog.load(name)? {
        Some(chain) => {
            print_chain(&chain);
            execute_chain(&chain);
        }
        None => {
            eprintln!("Chain '{name}' not found");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_rm(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::open()?;
    if catalog.delete(name)? {
        println!("Deleted chain '{name}'");
    } else {
        eprintln!("Chain '{name}' not found");
        std::process::exit(1);
    }
    Ok(())
}

fn print_chain(chain: &kubo_core::chain::ActionChain) {
    println!("  intent: {}", chain.chain.intent);
    println!("  created: {}", chain.chain.created_at);
    if !chain.chain.tags.is_empty() {
        println!("  tags: {}", chain.chain.tags.join(", "));
    }
    println!("  stages:");
    for (i, stage) in chain.stages.iter().enumerate() {
        match stage {
            kubo_core::stage::Stage::Shell { command } => {
                println!("    {}. [shell] {}", i + 1, command);
            }
            kubo_core::stage::Stage::Human {
                role,
                actor,
                prompt,
            } => {
                println!("    {}. [human] {} ({}) — {}", i + 1, role, actor, prompt);
            }
        }
    }
}

fn execute_chain(chain: &kubo_core::chain::ActionChain) {
    use kubo_core::stage::Stage;

    // Build tao pipe command from stages
    let mut tao_args: Vec<String> = Vec::new();

    for stage in &chain.stages {
        match stage {
            Stage::Shell { command } => {
                tao_args.push(command.clone());
            }
            Stage::Human {
                role,
                actor,
                prompt,
            } => {
                tao_args.push(format!("echo '{}'", prompt.replace('\'', "'\\''")));
                tao_args.push(format!("tao echo {role} {actor}"));
            }
        }
    }

    if tao_args.is_empty() {
        println!("\n(no stages to execute)");
        return;
    }

    let cmd_str = tao_args.join(" -- ");
    println!("\n> tao pipe {cmd_str}");

    // Build args: tao pipe "stage1" -- "stage2" -- "stage3"
    let mut args: Vec<&str> = vec!["pipe"];
    for (i, arg) in tao_args.iter().enumerate() {
        if i > 0 {
            args.push("--");
        }
        args.push(arg);
    }

    let status = std::process::Command::new("tao").args(&args).status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("tao exited with: {s}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("tao not found — install tao to execute pipelines");
        }
        Err(e) => eprintln!("failed to run tao: {e}"),
    }
}
