mod setup;
use anyhow::Context;
use configure_holochain;
use std::path::PathBuf;

#[tokio::test]
async fn configure_holochain_test() {
    println!("Spinning up holochain");
    let _holochain = setup::holochain::spawn_holochain();
    let happs_file_path: PathBuf = "./tests/config/config.yaml".into();
    let config = configure_holochain::Config {
        admin_port: 4444,
        happ_port: 42233,
        ui_store_folder: "./tmp".into(),
        happs_file_path: happs_file_path.clone(),
    };
    println!("Test running with config: {:?}", &config);

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
