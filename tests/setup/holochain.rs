use super::kill_on_drop::{kill_on_drop, KillChildOnDrop};
use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn spawn_holochain() -> KillChildOnDrop {
    // spin up holochain
    let logs_dir = create_tmp_dir();

    println!("Starting up");
    let mut holochain = kill_on_drop(
        Command::new("holochain")
            .current_dir("./tests/config")
            .arg("--config-path")
            .arg("holochain-config.yaml")
            .arg("--piped")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(logs_dir.join("holochain.txt")).unwrap())
            .spawn()
            .unwrap(),
    );
    println!("Spun up");
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
    std::env::current_dir().unwrap().join("tmp")
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
