use failure;
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use toml;

#[derive(Debug, Deserialize, Serialize)]
struct Backup {
    repository: PathBuf,
    password_file: PathBuf,
    excludes: Vec<String>,
    targets: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Secretz {
    path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    codez_path: PathBuf,
    secretz: Secretz,
    backup: Backup,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backup: Backup {
                repository: Path::new("/mnt/backupz/wk").to_path_buf(),
                password_file: Path::new("~/.config/.wk-bk.key").to_path_buf(),
                excludes: vec!["target".to_string()],
                targets: vec![
                    Path::new("/mnt/codez").to_path_buf(),
                    Path::new("/mnt/secretz").to_path_buf(),
                ],
            },
            secretz: Secretz {
                path: Path::new("/mnt/secretz").to_path_buf(),
            },
            codez_path: Path::new("/mnt/codez").to_path_buf(),
        }
    }
}

#[derive(StructOpt, Debug)]
enum Cli {
    #[structopt(name = "adopt")]
    /// adopt a file into secretz
    Adopt { file: PathBuf },

    #[structopt(name = "config")]
    /// manage configuration
    Config {
        #[structopt(subcommand)]
        config: ConfigSubcommands,
    },
    #[structopt(name = "backup")]
    /// start a backup
    Backup,
}

#[derive(StructOpt, Debug)]
enum ConfigSubcommands {
    #[structopt(name = "init")]
    Init,
    #[structopt(name = "show")]
    Show,
}

fn main() -> Result<(), failure::Error> {
    match Cli::from_args() {
        Cli::Adopt { file } => {
            println!("will adopt {}", &file.display());
        }
        Cli::Backup => {
            println!("will run backups");
        }
        Cli::Config { config } => match config {
            ConfigSubcommands::Init => {
                let config: Config = Default::default();
                println!("{}", toml::to_string(&config)?);
            }
            ConfigSubcommands::Show => {
                let config: Config = Default::default();
                println!("{}", toml::to_string(&config)?);
            }
        },
    }
    Ok(())
}
