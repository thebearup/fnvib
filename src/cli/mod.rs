use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::content_lists::{self, ContentData};
use crate::model::layout::Layout;
use crate::plugin::Plugin;

#[derive(Parser)]
#[command(name = "fnvib", about = "Fallout New Vegas Interior Builder")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a plugin from a layout TOML file.
    Generate {
        /// Path to the layout input file (.toml)
        input: PathBuf,

        /// Output .esp path (defaults to <input stem>.esp)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// RNG seed for reproducible generation (default: 0)
        #[arg(short, long, default_value_t = 0)]
        seed: u64,
    },

    /// Generate many variants of a layout with sequential seeds.
    Batch {
        /// Path to the layout input file (.toml)
        input: PathBuf,

        /// Directory to write output .esp files into
        #[arg(short, long, default_value = ".")]
        output_dir: PathBuf,

        /// Number of variants to generate
        #[arg(short, long, default_value_t = 10)]
        count: u64,

        /// Starting seed (variants use seed, seed+1, ..., seed+count-1)
        #[arg(short, long, default_value_t = 0)]
        seed: u64,
    },

    /// List available kits and their descriptions.
    ListKits,
}

pub fn run() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Generate { input, output, seed } => cmd_generate(&input, output.as_deref(), seed),
        Commands::Batch { input, output_dir, count, seed } => {
            cmd_batch(&input, &output_dir, count, seed)
        }
        Commands::ListKits => cmd_list_kits(),
    }
}

/// Loads content_lists.toml from the given path, printing a warning on failure.
fn load_content(path: &Path) -> ContentData {
    match content_lists::load(path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("warning: could not load {}: {e}", path.display());
            ContentData::default()
        }
    }
}

fn content_path(input: &Path) -> PathBuf {
    input.parent().unwrap_or(Path::new(".")).join("content_lists.toml")
}

fn load_layout(path: &std::path::Path) -> Layout {
    let src = std::fs::read_to_string(path)
        .unwrap_or_else(|e| { eprintln!("error reading {}: {e}", path.display()); std::process::exit(1); });
    toml::from_str::<Layout>(&src)
        .unwrap_or_else(|e| { eprintln!("error parsing {}: {e}", path.display()); std::process::exit(1); })
}

fn cmd_generate(input: &std::path::Path, output: Option<&std::path::Path>, seed: u64) {
    let layout = load_layout(input);

    let errors = layout.validate();
    if !errors.is_empty() {
        for e in &errors { eprintln!("validation error: {e}"); }
        std::process::exit(1);
    }

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| input.with_extension("esp"));

    let content = load_content(&content_path(input));
    let plugin = Plugin::from_layout(&layout, seed, &content);
    plugin.write_to_file(&out_path)
        .unwrap_or_else(|e| { eprintln!("error writing {}: {e}", out_path.display()); std::process::exit(1); });

    println!("wrote {}", out_path.display());
}

fn cmd_batch(input: &std::path::Path, output_dir: &std::path::Path, count: u64, start_seed: u64) {
    let layout = load_layout(input);
    let errors = layout.validate();
    if !errors.is_empty() {
        for e in &errors { eprintln!("validation error: {e}"); }
        std::process::exit(1);
    }

    std::fs::create_dir_all(output_dir)
        .unwrap_or_else(|e| { eprintln!("cannot create output dir: {e}"); std::process::exit(1); });

    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let content = load_content(&content_path(input));
    for i in 0..count {
        let seed = start_seed + i;
        let path = output_dir.join(format!("{stem}_seed{seed:04}.esp"));
        let plugin = Plugin::from_layout(&layout, seed, &content);
        plugin.write_to_file(&path)
            .unwrap_or_else(|e| { eprintln!("error writing {}: {e}", path.display()); std::process::exit(1); });
        println!("  [{}/{count}] {}", i + 1, path.display());
    }
}

fn cmd_list_kits() {
    let content = load_content(Path::new("content_lists.toml"));
    if content.kits.is_empty() {
        eprintln!("No kits found. Is content_lists.toml present in the current directory?");
        return;
    }
    println!("{:<15} {}", "KIT", "DESCRIPTION");
    println!("{}", "-".repeat(50));
    let mut names: Vec<&String> = content.kits.keys().collect();
    names.sort();
    for name in names {
        println!("{:<15} {}", name, content.kits[name].description);
    }
}
