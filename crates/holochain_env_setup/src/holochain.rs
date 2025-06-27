use lair_keystore_api::prelude::LairServerConfigInner as LairConfig;
use serde::Serialize;
use snafu::Snafu;
use std::{
    fs::File,
    io::{BufRead, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use taskgroup_manager::kill_on_drop::{kill_on_drop, KillChildOnDrop};
use tempfile::TempDir;
use tracing::trace;

pub fn default_password() -> String {
    std::env::var("HOLOCHAIN_DEFAULT_PASSWORD").unwrap()
}

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
        let passphrase = default_password();
        holochain_input.write_all(passphrase.as_bytes()).unwrap();
    }

    for line in std::io::BufReader::new(holochain.stdout.as_mut().unwrap()).lines() {
        let line = line.unwrap();
        trace!("{:?}", line);
        if line == "Conductor ready." {
            eprintln!("Encountered magic string");
            break;
        }
    }

    holochain
}

pub fn create_tmp_dir() -> PathBuf {
    TempDir::new().unwrap().keep()
}

pub fn create_log_dir() -> PathBuf {
    TempDir::new().unwrap().keep()
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
        data_root_path: PathBuf,
        keystore: KeystoreConfig,
        dpki: DpkiConfig,
        admin_interfaces: Option<Vec<AdminInterfaceConfig>>,
        network: NetworkConfig,
        db_sync_strategy: String,
    }
    #[derive(Serialize)]
    pub struct DpkiConfig {
        pub dna_path: Option<PathBuf>,
        pub network_seed: String,
        pub allow_throwaway_random_dpki_agent_key: bool,
        pub no_dpki: bool,
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
        Websocket { port: u16, allowed_origins: String },
    }

    #[derive(Serialize)]
    struct NetworkConfig {
        bootstrap_url: String,
        signal_url: String,
        disable_bootstrap: bool,
        disable_publish: bool,
        disable_gossip: bool,
        mem_bootstrap: bool,
    }

    let config = HolochainConfig {
        data_root_path: "./databases".into(),
        keystore: KeystoreConfig::LairServer {
            connection_url: lair_connection_url,
        },
        // Holo does not use DPKI, when we start using it this should be updated
        dpki: DpkiConfig {
            dna_path: None,
            network_seed: "".to_string(),
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: true,
        },
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: AdminInterfaceDriver::Websocket {
                port: admin_port,
                allowed_origins: "*".to_string(),
            },
        }]),
        network: NetworkConfig {
            bootstrap_url: "https://dev-test-bootstrap2.holochain.org/".to_string(),
            signal_url: "wss://dev-test-bootstrap2.holochain.org/".to_string(),
            disable_bootstrap: false,
            disable_publish: false,
            disable_gossip: false,
            mem_bootstrap: true,
        },
        db_sync_strategy: "Resilient".to_string(),
    };
    serde_yaml::to_writer(&mut holochain_config_file, &config).unwrap();

    Ok(())
}
