use crate::holochain::spawn_holochain;

use super::lair;
use holochain_keystore::MetaLairClient;
use lair_keystore_api::prelude::LairServerConfigInner as LairConfig;
use log::trace;
use snafu::Snafu;
use std::path::{Path, PathBuf};
use taskgroup_manager::kill_on_drop::KillChildOnDrop;

pub async fn setup_environment(
    tmp_dir: &Path,
    log_dir: &Path,
    device_bundle: Option<&str>,
    lair_fallback: Option<(PathBuf, u16)>,
) -> Result<Environment, SetupEnvironmentError> {
    trace!("Starting lair-keystore");
    let (lair, lair_config, keystore) = lair::spawn(tmp_dir, log_dir, device_bundle, lair_fallback)
        .await
        .unwrap();

    trace!("Spinning up holochain");
    let holochain = spawn_holochain(tmp_dir, log_dir, lair_config.clone());

    Ok(Environment {
        _holochain: holochain,
        _lair: lair,
        lair_config,
        keystore,
    })
}

#[derive(Debug, Snafu)]
pub enum SetupEnvironmentError {
    AdminWs { source: anyhow::Error },
    AppWs { source: anyhow::Error },
    LairClient { source: one_err::OneErr },
    ZomeCallSigning { source: one_err::OneErr },
    Anyhow { source: anyhow::Error },
    AppBundleE { source: anyhow::Error },
    FfsIo { source: anyhow::Error },
}

impl From<anyhow::Error> for SetupEnvironmentError {
    fn from(err: anyhow::Error) -> Self {
        SetupEnvironmentError::Anyhow { source: err }
    }
}

pub struct Environment {
    _holochain: KillChildOnDrop,
    _lair: KillChildOnDrop,
    pub lair_config: LairConfig,
    pub keystore: MetaLairClient,
}
