mod setup;
use configure_holochain;

#[tokio::test]
async fn configure_holochain_test() {
    println!("Spinning up holochain");
    let _holochain = setup::holochain::spawn_holochain();

    let config = configure_holochain::Config {
        admin_port: 4444,
        happ_port: 42233,
        ui_store_folder: "./tmp".into(),
        happs_file_path: "./tests/config/config.yaml".into(),
    };
    println!("Test running with config: {:?}", &config);

    println!("Run configure holochain script");
    configure_holochain::run(config).await.unwrap();

    println!("Successfully tested!");
}
