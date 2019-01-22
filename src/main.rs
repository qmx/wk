use app_dirs::{AppDataType, AppInfo};
use failure;
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use toml;

use std::fs::File;
use std::io::{Read, Write};

const APP_INFO: AppInfo = AppInfo {
    name: env!("CARGO_PKG_NAME"),
    author: env!("CARGO_PKG_AUTHORS"),
};

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

impl Config {
    fn config_path() -> Result<PathBuf, failure::Error> {
        Ok(app_dirs::app_dir(AppDataType::UserConfig, &APP_INFO, "")?.join("config.toml"))
    }

    fn load() -> Result<Config, failure::Error> {
        let config = match File::open(&Self::config_path()?) {
            Ok(mut file) => {
                let mut toml = String::new();
                file.read_to_string(&mut toml)?;
                toml::from_str(&toml)?
            }
            Err(_) => Default::default(),
        };
        Ok(config)
    }

    fn save(&self) -> Result<(), failure::Error> {
        let toml = toml::to_string(&self)?;
        let mut file = File::create(&Self::config_path()?)?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }
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
    Backup {
        #[structopt(subcommand)]
        backup: BackupSubcommands,
    },
}

#[derive(StructOpt, Debug)]
enum ConfigSubcommands {
    #[structopt(name = "init")]
    /// write default config
    Init {
        /// overwrite existing config
        #[structopt(short = "f", long = "force")]
        force: bool,
    },
}

#[derive(StructOpt, Debug)]
enum BackupSubcommands {
    #[structopt(name = "init")]
    /// init new repository
    Init { force: bool },
    #[structopt(name = "run")]
    /// run backup job
    Run,
    #[structopt(name = "gc")]
    /// forget & prune, according to retention policy
    GC,
}

fn main() -> Result<(), failure::Error> {
    match Cli::from_args() {
        Cli::Adopt { file } => {
            println!("will adopt {}", &file.display());
        }
        Cli::Backup { backup } => match backup {
            BackupSubcommands::Init { force: _ } => {
                println!("will init backup repo");
            }
            BackupSubcommands::Run => {
                println!("will init backups");
            }
            BackupSubcommands::GC => {
                println!("will init backups");
            }
        },
        Cli::Config { config } => match config {
            ConfigSubcommands::Init { force } => {
                let path = Config::config_path()?;
                if path.exists() && !force {
                    return Err(failure::format_err!(
                        "config file already exists, use --force to overwrite"
                    ));
                }
                let config: Config = Default::default();
                config.save()?;
                eprintln!("successfully written new config to {}", &path.display());
            }
        },
    }
    Ok(())
}
