use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{self, Command},
    str,
};

use holochain_keystore::MetaLairClient;
use lair_keystore_api::prelude::{
    LairServerConfigInner as LairConfig, LairServerSignatureFallback,
};
use snafu::{ResultExt, Snafu};
use taskgroup_manager::kill_on_drop::{kill_on_drop, KillChildOnDrop};

pub async fn spawn(
    tmp_dir: &Path,
    logs_dir: &Path,
    fallback: Option<(PathBuf, u16)>,
    device_bundle: Option<&str>,
) -> Result<(KillChildOnDrop, LairConfig, MetaLairClient), SpawnError> {
    use dotenv::dotenv;
    dotenv().ok();

    let lair_dir = tmp_dir.join("lair-keystore");
    std::fs::create_dir_all(&lair_dir).with_context(|_error| CreateLairDirSnafu {
        path: lair_dir.clone(),
    })?;

    let init_log_path = logs_dir.join("lair-keystore-init.txt");

    let init_log = File::create(&init_log_path).with_context(|_error| CreateInitLogFileSnafu {
        path: init_log_path.clone(),
    })?;

    init_lair(&lair_dir, init_log.try_clone().unwrap()).context(InitSnafu {
        log_path: init_log_path,
    })?;

    if let Some(bundle) = device_bundle {
        import_seed(&lair_dir, init_log, bundle).unwrap();
    }

    let lair_config_path = lair_dir.join("lair-keystore-config.yaml");

    let mut lair_config = read_lair_config(&lair_config_path).context(ReadConfigSnafu)?;

    if let Some((fallback_executable_path, signing_port)) = fallback {
        set_lair_fallback(
            &mut lair_config,
            fallback_executable_path.clone(),
            signing_port,
        );

        write_lair_config(lair_config_path, &lair_config).context(WriteConfigSnafu)?
    }

    let server_log_path = logs_dir.join("lair-logs.txt");

    let server_log =
        File::create(&server_log_path).with_context(|_error| CreateServerLogFileSnafu {
            path: server_log_path.clone(),
        })?;

    let lair = spawn_lair_server(&lair_dir, server_log).context(SpawnLairServerSnafu {
        log_path: server_log_path,
    })?;

    let connection_url = lair_config.connection_url.clone();

    let env_pw = std::env::var("HOLOCHAIN_DEFAULT_PASSWORD")
        .expect("HOLOCHAIN_DEFAULT_PASSWORD must be set");
    let passphrase: sodoken::BufRead = sodoken::BufRead::from(env_pw.to_string().as_bytes());

    let keystore = match holochain_keystore::lair_keystore::spawn_lair_keystore(
        connection_url.into(),
        passphrase,
    )
    .await
    {
        Ok(keystore) => keystore,
        Err(err) => {
            log::error!("{:?}", err.str_kind());
            return Err(SpawnError::LairClient { source: err })?;
        }
    };

    Ok((lair, lair_config, keystore))
}

fn init_lair(lair_dir: &Path, log: File) -> Result<(), InitLairError> {
    let log_2 = log.try_clone().context(CloneLogFileSnafu)?;

    let mut lair_init = kill_on_drop(
        Command::new("lair-keystore")
            .arg("--lair-root")
            .arg(lair_dir)
            .arg("init")
            .arg("--piped")
            .stdin(process::Stdio::piped())
            .stdout(log)
            .stderr(log_2)
            .spawn()
            .context(SpawnLairInitSnafu)?,
    );

    write_passphrase(&mut lair_init, None).context(WritePassphraseToInitSnafu)?;

    let exit_status = lair_init.wait().context(WaitProcessSnafu)?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(InitLairError::NonZeroExitStatus {
            status: exit_status,
        })
    }
}

fn import_seed(lair_dir: &Path, log: File, device_bundle: &str) -> Result<(), InitLairError> {
    let log_2 = log.try_clone().unwrap();
    let mut lair_init = kill_on_drop(
        Command::new("lair-keystore")
            .arg("--lair-root")
            .arg(lair_dir)
            .arg("import-seed")
            .arg("host")
            .arg(device_bundle)
            .arg("--piped")
            .stdin(process::Stdio::piped())
            .stdout(log)
            .stderr(log_2)
            .spawn()
            .unwrap(),
    );
    // Here format of a passphrase is "<lair_password>/n<seed_bundle_password>"
    write_passphrase(&mut lair_init, Some(b"passphrase\npass")).unwrap();
    let exit_status = lair_init.wait().unwrap();
    if exit_status.success() {
        Ok(())
    } else {
        Err(InitLairError::NonZeroExitStatus {
            status: exit_status,
        })
    }
}

fn write_passphrase(child: &mut KillChildOnDrop, buf: Option<&[u8]>) -> Result<(), io::Error> {
    if let Some(pw) = buf {
        child
            .stdin
            .take()
            .expect("child lair process was spawned with piped stdin")
            .write_all(pw)
    } else {
        let env_pw = std::env::var("HOLOCHAIN_DEFAULT_PASSWORD")
            .expect("HOLOCHAIN_DEFAULT_PASSWORD must be set");
        child
            .stdin
            .take()
            .expect("child lair process was spawned with piped stdin")
            .write_all(env_pw.to_string().as_bytes())
    }
}

fn spawn_lair_server(lair_dir: &Path, log: File) -> Result<KillChildOnDrop, SpawnLairServerError> {
    let mut lair = kill_on_drop(
        Command::new("lair-keystore")
            .arg("--lair-root")
            .arg(lair_dir)
            .arg("server")
            .arg("--piped")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(log)
            .spawn()
            .context(SpawnLairServerCommandSnafu)?,
    );

    write_passphrase(&mut lair, None).context(WritePassphraseToServerSnafu)?;
    wait_for_ready_string(&mut lair).context(WaitReadyStringSnafu)?;
    Ok(lair)
}

fn wait_for_ready_string(child: &mut KillChildOnDrop) -> Result<(), io::Error> {
    let output = child
        .stdout
        .as_mut()
        .expect("child lair process was spawned with piped stdout");

    // Read exactly one byte from stdout to make sure it outputs its "ready" string
    output.read_exact(&mut [0])
}

fn read_lair_config(path: &Path) -> Result<LairConfig, ReadConfigError> {
    let file = File::open(path).with_context(|_error| OpenSnafu {
        path: path.to_owned(),
    })?;
    serde_yaml::from_reader(file).context(ParseSnafu)
}

fn set_lair_fallback(
    config: &mut LairConfig,
    fallback_executable_path: PathBuf,
    signing_port: u16,
) {
    config.signature_fallback = LairServerSignatureFallback::Command {
        program: fallback_executable_path,
        args: Some(vec!["--signing-port".to_string(), signing_port.to_string()]),
    };
}

fn write_lair_config(path: PathBuf, config: &LairConfig) -> Result<(), WriteConfigError> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .context(TruncateConfigFileSnafu)?;
    serde_yaml::to_writer(file, &config).context(WriteConfigFileSnafu)
}

#[derive(Debug, Snafu)]
pub enum SpawnError {
    CreateLairDir {
        path: PathBuf,
        source: io::Error,
    },
    #[snafu(display(
        "Could not initialize lair keystore: {}
Check {} for logs",
        source,
        log_path.display()
    ))]
    Init {
        source: InitLairError,
        log_path: PathBuf,
    },
    ReadConfig {
        source: ReadConfigError,
    },
    CreateInitLogFile {
        path: PathBuf,
        source: io::Error,
    },
    CreateServerLogFile {
        path: PathBuf,
        source: io::Error,
    },
    WriteConfig {
        source: WriteConfigError,
    },
    #[snafu(display(
        "Could not run lair keystore: {}
Check {} for logs",
        source,
        log_path.display()
    ))]
    SpawnLairServer {
        source: SpawnLairServerError,
        log_path: PathBuf,
    },
    LairClient {
        source: one_err::OneErr,
    },
}

#[derive(Debug, Snafu)]
pub enum InitLairError {
    CreateLogFile { path: PathBuf, source: io::Error },
    CloneLogFile { source: io::Error },
    SpawnLairInit { source: io::Error },
    WritePassphraseToInit { source: io::Error },
    WaitProcess { source: io::Error },
    NonZeroExitStatus { status: process::ExitStatus },
}

#[derive(Debug, Snafu)]
pub enum SpawnLairServerError {
    SpawnLairServerCommand { source: io::Error },
    WritePassphraseToServer { source: io::Error },
    WaitReadyString { source: io::Error },
}

#[derive(Debug, Snafu)]
pub enum ReadConfigError {
    Parse { source: serde_yaml::Error },
    Open { path: PathBuf, source: io::Error },
}

#[derive(Debug, Snafu)]
pub enum WriteConfigError {
    TruncateConfigFile { source: io::Error },
    WriteConfigFile { source: serde_yaml::Error },
}
