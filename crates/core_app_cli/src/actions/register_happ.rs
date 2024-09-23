use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::{PresentedHappBundle, HappInput},
};


pub async fn get(
    hosted_urls: Vec<String>,
    bundle_url: String,
    name: String,
    uid: Option<String>,
    special_installed_app_id: Option<String>,
) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;
   
    let register_payload = HappInput {
        name,
        bundle_url,
        uid,
        hosted_urls,
        special_installed_app_id,
        ..HappInput::default()
    };

    let published_happ: PresentedHappBundle = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("register_happ"),
            register_payload,
        )
        .await?;

    println!("===================");
    println!("Your Published Happ Bundle is: ");
    println!("{:?}", published_happ);
    println!("===================");

    Ok(())
}
