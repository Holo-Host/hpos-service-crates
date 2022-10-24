mod setup;
use anyhow::Context;
use configure_holochain;
use std::env::set_var;
use std::path::PathBuf;

/// Tests how configure holochian would perform on a Holoport on devNet
#[tokio::test]
async fn configure_holochain_test() {
    let tmp_dir = setup::holochain::create_tmp_dir();
    let log_dir = setup::holochain::create_log_dir();

    // spin up lair
    let (_lair, lair_config) = setup::lair::spawn(&tmp_dir, &log_dir, None).unwrap();

    println!("Spinning up holochain");
    let _holochain = setup::holochain::spawn_holochain(&tmp_dir, &log_dir, lair_config);
    let happs_file_path: PathBuf = "./tests/config/config.yaml".into();
    let config = configure_holochain::Config {
        admin_port: 4444,
        happ_port: 42233,
        ui_store_folder: "./tmp".into(),
        happs_file_path: happs_file_path.clone(),
    };
    println!("Test running with config: {:?}", &config);

    // Set PUBKEY_PATH in a writable temp location
    set_var("PUBKEY_PATH", &tmp_dir.clone().join("agent.key"));

    // Set MEM_PROOF_PATH in a writable temp location
    set_var("MEM_PROOF_PATH", &tmp_dir.join("mem-proof"));

    // Point HPOS_CONFIG_PATH to test config file
    set_var("HPOS_CONFIG_PATH", "./tests/config/hp-primary-bzywj.json");

    // On devNet holoports force random key
    set_var("FORCE_RANDOM_AGENT_KEY", "1");

    // Holoports do not force read-only memproof
    set_var("READ_ONLY_MEM_PROOF", "false");

    //
    set_var("MEM_PROOF_SERVER_URL", "https://hbs.dev.holotest.net");

    println!("Run configure holochain script");
    configure_holochain::run(config).await.unwrap();

    let mut connection = configure_holochain::AdminWebsocket::connect(4444)
        .await
        .unwrap();

    let happ_file = configure_holochain::HappsFile::load_happ_file(happs_file_path)
        .context("failed to load hApps YAML config")
        .unwrap();

    let happs = connection
        .list_running_app()
        .await
        .context("failed to get installed hApps")
        .unwrap();

    // checking if all the happs are installed
    happ_file.self_hosted_happs.iter().for_each(|h| {
        assert!(happs.contains(&h.id()), "{} is not installed", h.id());
    });
    happ_file.core_happs.iter().for_each(|h| {
        assert!(happs.contains(&h.id()), "{} is not installed", h.id());
    });
    println!("Successfully tested! {:?}", happs);
}
