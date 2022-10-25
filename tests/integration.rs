mod setup;
use anyhow::Context;
use configure_holochain;
use configure_holochain::agent::get_hpos_config;
use configure_holochain::membrane_proof::delete_mem_proof_file;
use hpos_config_core::Config;
use std::env::set_var;
use std::path::PathBuf;

/// Integration test for configure-holochain binary
/// The purpose of those test is to show that binary does what it's
/// supposed to do in 3 different environments:
/// - holoport on alphaNet
/// - holoport on devNet
/// - server with READ_ONLY_MEM_PROOF = true
///
/// Each test creates environment suitable for a given scenario - env vars,
/// running instance holochain and running instance of lair-keystore with
/// appropriate keys initialized

/// Testing scenario for holoport running on alphaNet
/// Holoport is in blank state with holochain running and lair-keystore initialized
/// Host's keypair is imported to lair-keystore from hpos-config file and device_bundle is unlocked
/// with default password `pass`
/// Env vars:
///   HPOS_CONFIG_PATH - local file to read from
///   MEM_PROOF_PATH - local file to write to
///   PUBKEY_PATH - local file to write to
///   FORCE_RANDOM_AGENT_KEY - set to "" on alphaNet
///   READ_ONLY_MEM_PROOF - false on holoports
///   MEM_PROOF_SERVER_URL - HBS server url
///
/// Note about MEM_PROOF_SERVER_URL:
/// this integration test is downlading memproof from an external server. It is not possible to
/// mock this interaction, because memproof server signs payload with its own private key
/// So this test will pass only as long as version of core-app in config/config.yaml
/// will match settings on HBS server. If you start getting an error of a type
/// ConductorApiError(WorkflowError(GenesisFailure(\"Joining code invalid: unexpected author ...
/// you're out of sync with HBS server
#[tokio::test]
async fn holoport_on_alpha_net() {
    // Point HPOS_CONFIG_PATH to test config file
    set_var("HPOS_CONFIG_PATH", "./tests/config/hp-primary-bzywj.json");

    let tmp_dir = setup::holochain::create_tmp_dir();
    let log_dir = setup::holochain::create_log_dir();

    // Set PUBKEY_PATH in a writable temp location
    set_var("PUBKEY_PATH", &tmp_dir.clone().join("agent.key"));

    // Set MEM_PROOF_PATH in a writable temp location
    set_var("MEM_PROOF_PATH", &tmp_dir.join("mem-proof"));

    // On devNet holoports force random key
    set_var("FORCE_RANDOM_AGENT_KEY", "");

    // Holoports do not force read-only memproof
    set_var("READ_ONLY_MEM_PROOF", "false");

    // devNet HBS server url, because given hpos-config is registered in devNet database
    set_var("MEM_PROOF_SERVER_URL", "https://hbs.dev.holotest.net");

    let device_bundle = match get_hpos_config().unwrap() {
        Config::V2 { device_bundle, .. } => device_bundle,
        _ => panic!("Unsupported Config version"),
    };

    // spin up lair
    println!("Starting lair-keystore");
    let (_lair, lair_config) =
        setup::lair::spawn(&tmp_dir, &log_dir, &device_bundle, None).unwrap();

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

    println!("Run configure holochain script");
    configure_holochain::run(config.clone()).await.unwrap();

    // Second run should not error out
    configure_holochain::run(config.clone()).await.unwrap();

    // Delete memproof which is an equivalent of changing DEV_UID_OVERRIDE for holoport
    // which was creating a bug https://github.com/Holo-Host/hpos-configure-holochain/issues/136
    delete_mem_proof_file().unwrap();

    // Third run should not error out
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
