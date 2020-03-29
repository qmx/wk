use anyhow::{self, format_err};
use app_dirs::{AppDataType, AppInfo};
use directories;
use duct::cmd;
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
    password: String,
    excludes: Vec<String>,
    targets: Vec<String>,
    repository: Repository,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Repository {
    S3(S3Info),
    Local(LocalPath),
}

impl Repository {
    fn path(&self) -> String {
        match self {
            Repository::Local(path) => path.path.display().to_string(),
            Repository::S3(s3) => s3.clone().url(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LocalPath {
    path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct S3Info {
    bucket: String,
    endpoint: Option<String>,
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

impl Default for S3Info {
    fn default() -> Self {
        Self {
            bucket: "my_bucket".into(),
            endpoint: Some("https://my-s3-endpoint.net".into()),
            access_key_id: "access_key_id".into(),
            secret_access_key: "secret_access_key".into(),
            region: "us-east-1".into(),
        }
    }
}

impl S3Info {
    fn url(self) -> String {
        format!(
            "s3:{}/{}",
            &self.endpoint.unwrap_or("s3.amazonaws.com".to_string()),
            self.bucket
        )
    }
}

#[test]
fn test_s3_url() {
    let s3 = S3Info {
        bucket: "foo".to_string(),
        access_key_id: "baz".to_string(),
        secret_access_key: "bar".to_string(),
        endpoint: None,
    };
    assert_eq!("s3:s3.amazonaws.com/foo", s3.url());
}

#[derive(Debug, Deserialize, Serialize)]
struct Secretz {
    path: PathBuf,
}

impl Secretz {
    fn pack_dir(&self) -> PathBuf {
        self.path.join(&whoami::username()).join("pack")
    }

    fn adopt(&self, path: PathBuf) -> Result<(), anyhow::Error> {
        if path.is_dir() {
            return Err(format_err!("should not be a dir"));
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(format_err!("should not be a symlink"));
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
    secretz: Secretz,
    backup: Backup,
}

impl Config {
    fn default_config_path() -> Result<PathBuf, anyhow::Error> {
        Ok(app_dirs::app_dir(AppDataType::UserConfig, &APP_INFO, "")?.join("config.toml"))
    }

    fn load_from_path(path: PathBuf) -> Result<Config, anyhow::Error> {
        let config = match File::open(&path) {
            Ok(mut file) => {
                let mut toml = String::new();
                file.read_to_string(&mut toml)?;
                toml::from_str(&toml)?
            }
            Err(_) => Default::default(),
        };
        Ok(config)
    }

    fn load() -> Result<Config, anyhow::Error> {
        Self::load_from_path(Self::default_config_path()?)
    }

    fn save(&self) -> Result<(), anyhow::Error> {
        let toml = toml::to_string(&self)?;
        let mut file = File::create(&Self::default_config_path()?)?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backup: Backup {
                password: "very_secure_password".to_string(),
                excludes: vec!["target".to_string()],
                targets: vec!["/mnt/codez".to_string(), "/mnt/secretz".to_string()],
                repository: Repository::Local(LocalPath {
                    path: Path::new("/mnt/backupz/wk").to_path_buf(),
                }),
            },
            secretz: Secretz {
                path: Path::new("/mnt/secretz").to_path_buf(),
            },
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

        /// add remote storage sample
        #[structopt(short = "r", long = "remote")]
        remote_storage: bool,
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

    #[structopt(name = "snapshots")]
    /// list snapshots
    Snapshots,

    #[structopt(name = "restore")]
    /// restore backup
    Restore {
        #[structopt(short = "H", long = "host")]
        /// host tag
        host: String,

        #[structopt(short = "t", long = "target")]
        /// directory to restore the backup to (usually "/")
        target: String,

        #[structopt(short = "f", long = "config")]
        /// load config from an alternate path, useful for initial restores
        alternate_config: Option<PathBuf>,

        /// the backup snapshot id, "latest" is accepted
        snapshot_id: String,
    },
}

fn restic(backup: &Backup, main_cmd: &str, extra_args: Vec<String>) -> duct::Expression {
    let path = &backup.repository.path();
    let mut args = vec![main_cmd];
    args.extend(extra_args.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
    let mut c = cmd("restic", &args)
        .env("RESTIC_REPOSITORY", path)
        .env("RESTIC_PASSWORD", &backup.password);
    if let Repository::S3(s3) = &backup.repository {
        c = c
            .env("AWS_DEFAULT_REGION", &s3.region)
            .env("AWS_ACCESS_KEY_ID", &s3.access_key_id)
            .env("AWS_SECRET_ACCESS_KEY", &s3.secret_access_key);
    }
    c
}

fn main() -> Result<(), anyhow::Error> {
    match Cli::from_args() {
        Cli::Adopt { file } => {
            let config = Config::load()?;
            config.secretz.adopt(file)?;
            println!("file adopted, now start a new shell");
        }
        Cli::Backup { backup } => match backup {
            BackupSubcommands::Init { force: _ } => {
                let config = Config::load()?;
                restic(&config.backup, "init", vec![]).run()?;
            }
            BackupSubcommands::Run => {
                let config = Config::load()?;
                let mut extra_args = vec![];
                for exclude in &config.backup.excludes {
                    extra_args.push(format!("--exclude={}", exclude));
                }
                for target in &config.backup.targets {
                    extra_args.push(target.to_string());
                }
                restic(&config.backup, "backup", extra_args).run()?;
            }
            BackupSubcommands::Snapshots => {
                let config = Config::load()?;
                restic(&config.backup, "snapshots", vec![]).run()?;
            }
            BackupSubcommands::Restore {
                host,
                target,
                snapshot_id,
                alternate_config,
            } => {
                let config = if let Some(alt) = alternate_config {
                    Config::load_from_path(alt)?
                } else {
                    Config::load()?
                };
                restic(
                    &config.backup,
                    "restore",
                    vec![
                        "-H".to_string(),
                        host,
                        "--target".to_string(),
                        target,
                        snapshot_id,
                    ],
                )
                .run()?;
            }
        },
        Cli::Config { config } => match config {
            ConfigSubcommands::Init {
                force,
                remote_storage,
            } => {
                let path = Config::default_config_path()?;
                if path.exists() && !force {
                    return Err(format_err!(
                        "config file already exists, use --force to overwrite"
                    ));
                }
                let mut config: Config = Default::default();
                if remote_storage {
                    config.backup.repository = Repository::S3(S3Info::default());
                }
                config.save()?;
                eprintln!("successfully written new config to {}", &path.display());
            }
        },
    }
    Ok(())
}
