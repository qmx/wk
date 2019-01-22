use app_dirs::{AppDataType, AppInfo};
use directories;
use duct::cmd;
use failure;
use pathdiff::diff_paths;
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use toml;
use whoami;

use std::fs::{self, File};
use std::io::{Read, Write};

const APP_INFO: AppInfo = AppInfo {
    name: env!("CARGO_PKG_NAME"),
    author: env!("CARGO_PKG_AUTHORS"),
};

#[derive(Debug, Deserialize, Serialize)]
struct Backup {
    repository: String,
    password_file: String,
    excludes: Vec<String>,
    targets: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Secretz {
    path: PathBuf,
}

impl Secretz {
    fn pack_dir(&self) -> PathBuf {
        self.path.join(&whoami::username()).join("pack")
    }

    fn adopt(&self, path: PathBuf) -> Result<(), failure::Error> {
        if path.is_dir() {
            return Err(failure::format_err!("should not be a dir"));
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(failure::format_err!("should not be a symlink"));
        }
        if let Some(basedirs) = directories::BaseDirs::new() {
            if let Some(relpath) = diff_paths(&path, &basedirs.home_dir()) {
                if let Some(parent) = &relpath.parent() {
                    let target_dir = self.pack_dir().join(&parent);
                    fs::create_dir_all(&target_dir)?;
                    fs::copy(&path, &self.pack_dir().join(&relpath))?;
                    fs::remove_file(&path)?
                }
            }
        }

        Ok(())
    }
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
                repository: "/mnt/backupz/wk".to_string(),
                password_file: "/home/qmx/.config/.wk-bk.key".to_string(),
                excludes: vec!["target".to_string()],
                targets: vec!["/mnt/codez".to_string(), "/mnt/secretz".to_string()],
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
}

fn main() -> Result<(), failure::Error> {
    match Cli::from_args() {
        Cli::Adopt { file } => {
            let config = Config::load()?;
            config.secretz.adopt(file)?;
            println!("file adopted, now start a new shell");
        }
        Cli::Backup { backup } => match backup {
            BackupSubcommands::Init { force: _ } => {
                let config = Config::load()?;
                cmd!(
                    "restic",
                    "-r",
                    config.backup.repository,
                    "-p",
                    config.backup.password_file,
                    "init"
                )
                .run()?;
            }
            BackupSubcommands::Run => {
                let config = Config::load()?;
                let mut args = vec![
                    "-r".to_string(),
                    config.backup.repository,
                    "-p".to_string(),
                    config.backup.password_file,
                    "backup".to_string(),
                ];
                for exclude in config.backup.excludes {
                    args.push(format!("--exclude={}", exclude));
                }
                for target in config.backup.targets {
                    args.push(target);
                }
                cmd("restic", &args).run()?;
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
