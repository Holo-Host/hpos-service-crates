use lair_keystore_api::prelude::LairServerConfigInner as LairConfig;
use serde::Serialize;
use snafu::Snafu;
use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use taskgroup_manager::kill_on_drop::{kill_on_drop, KillChildOnDrop};

pub fn spawn_holochain(
    tmp_dir: &Path,
    logs_dir: &Path,
    lair_config: LairConfig,
) -> KillChildOnDrop {
    let lair_connection_url = lair_config.connection_url.to_string();

    let admin_port = 4444;

    let holochain_config_name = "holochain-config.yaml";
    write_holochain_config(
        &tmp_dir.join(holochain_config_name),
        lair_connection_url,
        admin_port,
    )
    .unwrap();

    // spin up holochain
    let mut holochain = kill_on_drop(
        Command::new("holochain")
            .current_dir(tmp_dir)
            .arg("--config-path")
            .arg("holochain-config.yaml")
            .arg("--piped")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(logs_dir.join("holochain.txt")).unwrap())
            .spawn()
            .unwrap(),
    );

    {
        let mut holochain_input = holochain.stdin.take().unwrap();
        holochain_input.write_all(b"passphrase\n").unwrap();
    }

    for line in std::io::BufReader::new(holochain.stdout.as_mut().unwrap()).lines() {
        let line = line.unwrap();
        if line == "Conductor ready." {
            eprintln!("Encountered magic string");
            break;
        }
    }

    holochain
}

pub fn get_tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir();
    println!("Temporary directory: {}", dir.display());
    dir
}

pub fn create_tmp_dir() -> PathBuf {
    let tmp_dir = get_tmp_dir();
    match std::fs::remove_dir_all(&tmp_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => panic!("failed to remove tmp/ directory recursively: {}", e),
    }
    std::fs::create_dir_all(&tmp_dir).unwrap();
    tmp_dir
}

pub fn create_log_dir() -> PathBuf {
    let dir = std::env::temp_dir();
    println!("Temporary directory for logs: {}", dir.display());
    dir
}

#[derive(Debug, Snafu)]
pub enum WriteHolochainConfigError {
    CreateHolochainConfig { path: PathBuf },
}

fn write_holochain_config(
    path: &Path,
    lair_connection_url: String,
    admin_port: u16,
) -> Result<(), WriteHolochainConfigError> {
    let mut holochain_config_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();

    #[derive(Serialize)]
    struct HolochainConfig {
        environment_path: PathBuf,
        keystore: KeystoreConfig,
        admin_interfaces: Option<Vec<AdminInterfaceConfig>>,
    }

    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum KeystoreConfig {
        LairServer { connection_url: String },
    }

    #[derive(Serialize)]
    struct AdminInterfaceConfig {
        driver: AdminInterfaceDriver,
    }

    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum AdminInterfaceDriver {
        Websocket { port: u16 },
    }

    let config = HolochainConfig {
        environment_path: "./databases".into(),
        keystore: KeystoreConfig::LairServer {
            connection_url: lair_connection_url,
        },
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: AdminInterfaceDriver::Websocket { port: admin_port },
        }]),
    };
    serde_yaml::to_writer(&mut holochain_config_file, &config).unwrap();

    Ok(())
}
