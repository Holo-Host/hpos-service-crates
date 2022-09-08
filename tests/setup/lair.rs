use super::kill_on_drop::*;
use lair_keystore_api::prelude::{
    LairServerConfigInner as LairConfig, LairServerSignatureFallback,
};
use snafu::Snafu;
use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{self, Command},
};

pub fn spawn(
    tmp_dir: &Path,
    logs_dir: &Path,
    fallback: Option<(PathBuf, u16)>,
) -> Result<(KillChildOnDrop, LairConfig), SpawnError> {
    let lair_dir = tmp_dir.join("lair");
    std::fs::create_dir_all(&lair_dir).unwrap();

    let init_log_path = logs_dir.join("lair-keystore-init.txt");
    let init_log = File::create(&init_log_path).unwrap();
    init_lair(&lair_dir, init_log).unwrap();

    let lair_config_path = lair_dir.join("lair-keystore-config.yaml");
    let mut lair_config = read_lair_config(&lair_config_path).unwrap();
    if let Some((fallback_executable_path, signing_port)) = fallback {
        set_lair_fallback(&mut lair_config, fallback_executable_path, signing_port);
        write_lair_config(lair_config_path, &lair_config).unwrap();
    }

    let server_log_path = logs_dir.join("lair-keystore-server.txt");
    let server_log = File::create(&server_log_path).unwrap();
    let lair = spawn_lair_server(&lair_dir, server_log).unwrap();

    Ok((lair, lair_config))
}

fn init_lair(lair_dir: &Path, log: File) -> Result<(), InitLairError> {
    let log_2 = log.try_clone().unwrap();
    let mut lair_init = kill_on_drop(
        Command::new("lair-keystore")
            .arg("--lair-root")
            .arg(&lair_dir)
            .arg("init")
            .arg("--piped")
            .stdin(process::Stdio::piped())
            .stdout(log)
            .stderr(log_2)
            .spawn()
            .unwrap(),
    );
    write_passphrase(&mut lair_init).unwrap();
    let exit_status = lair_init.wait().unwrap();
    if exit_status.success() {
        Ok(())
    } else {
        Err(InitLairError::NonZeroExitStatus {
            status: exit_status,
        })
    }
}

fn write_passphrase(child: &mut KillChildOnDrop) -> Result<(), io::Error> {
    child
        .stdin
        .take()
        .expect("child lair process was spawned with piped stdin")
        .write_all(b"passphrase\n")
}

fn spawn_lair_server(lair_dir: &Path, log: File) -> Result<KillChildOnDrop, SpawnLairServerError> {
    let mut lair = kill_on_drop(
        Command::new("lair-keystore")
            .arg("--lair-root")
            .arg(&lair_dir)
            .arg("server")
            .arg("--piped")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(log)
            .spawn()
            .unwrap(),
    );
    write_passphrase(&mut lair).unwrap();
    wait_for_ready_string(&mut lair).unwrap();
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

fn read_lair_config(path: &Path) -> Result<LairConfig, serde_yaml::Error> {
    let file = File::open(&path).unwrap();
    serde_yaml::from_reader(file)
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

fn write_lair_config(path: PathBuf, config: &LairConfig) -> Result<(), serde_yaml::Error> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    serde_yaml::to_writer(file, &config)
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
