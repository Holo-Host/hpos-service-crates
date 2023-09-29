use holo_happ_manager;

#[tokio::test]
async fn run_happ_manager() {
    use configure_holochain;
    use hpos_config_core::Config;
    use hpos_hc_connect::hpos_agent::get_hpos_config;
    use std::env::set_var;
    use std::path::PathBuf;

    // Point HPOS_CONFIG_PATH to test config file
    set_var(
        "HPOS_CONFIG_PATH",
        "../holochain_env_setup/config/hp-primary-bzywj.json",
    );

    let tmp_dir = holochain_env_setup::holochain::create_tmp_dir();
    let log_dir = holochain_env_setup::holochain::create_log_dir();

    // Set HOST_PUBKEY_PATH in a writable temp location
    set_var("HOST_PUBKEY_PATH", &tmp_dir.clone().join("agent.key"));

    // Set MEM_PROOF_PATH in a writable temp location
    set_var("MEM_PROOF_PATH", &tmp_dir.join("mem-proof"));

    // Set HOLOFUEL_INSTANCE_ROLE for the mem-proof server
    set_var("HOLOFUEL_INSTANCE_ROLE", "host");

    // On devNet holoports force random key
    set_var("FORCE_RANDOM_AGENT_KEY", "1");

    // Holoports do not force read-only memproof
    set_var("READ_ONLY_MEM_PROOF", "false");

    // devNet HBS server url, because given hpos-config is registered in devNet database
    set_var(
        "MEM_PROOF_SERVER_URL",
        "https://membrane-proof.dev.holotest.net",
    );

    // pass to unlock the seed
    set_var("DEVICE_SEED_DEFAULT_PASSWORD", "pass");
    set_var("HOLOCHAIN_DEFAULT_PASSWORD", "pass");

    let device_bundle = match get_hpos_config().unwrap() {
        Config::V2 { device_bundle, .. } => device_bundle,
        _ => panic!("Unsupported Config version"),
    };

    // spin up lair
    println!("Starting lair-keystore");
    let (_lair, lair_config, _) =
        holochain_env_setup::lair::spawn(&tmp_dir, &log_dir, Some(&device_bundle), None)
            .await
            .unwrap();

    println!("Spinning up holochain");
    let _holochain =
        holochain_env_setup::holochain::spawn_holochain(&tmp_dir, &log_dir, lair_config.clone());

    let happs_file_path: PathBuf = "./tests/config.yaml".into();
    let config = hpos_hc_connect::holo_config::Config {
        admin_port: 4444,
        happ_port: 42233,
        ui_store_folder: None,
        happs_file_path: happs_file_path.clone(),
        lair_url: Some(lair_config.connection_url.to_string()),
    };
    println!("Test running with config: {:?}", &config);
    println!("Run configure holochain script to install HHA");
    configure_holochain::run(config.clone()).await.unwrap();

    set_var("HOLO_PUBLISHED_HAPPS", "./tests/holo-published-happs.json");

    println!("Run holo happ manager script");
    holo_happ_manager::run(&config).await.unwrap();

    pub use hpos_hc_connect::holo_config::HappsFile;
    let happ_file = HappsFile::load_happ_file(&config.happs_file_path).unwrap();
    let core_happ = happ_file.core_app().unwrap();

    let published_happ = holo_happ_manager::get_my_apps::published(&core_happ, &config)
        .await
        .unwrap();

    println!("Published happ: {:?}", published_happ);
    assert_eq!(published_happ.len(), 2);
    assert_eq!(published_happ[0].bundle_url, "https://");
    assert_eq!(published_happ[1].bundle_url, "https://");
    println!("Successfully tested!");
}
