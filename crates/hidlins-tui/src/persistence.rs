//! Filesystem persistence owned outside the input/render coordinator.

use std::path::Path;

use hidlins_core::HidlinsPaths;

use crate::config::{self, TuiConfig};
use crate::user_config::UserConfig;

pub(crate) fn save_ui_state(paths: &HidlinsPaths, config: &TuiConfig) -> Result<(), String> {
    let path = config::config_path(paths);
    config.save(paths, &path).map_err(|error| error.to_string())
}

pub(crate) fn update_user_config(
    path: &Path,
    update: impl FnOnce(&mut UserConfig),
) -> Result<UserConfig, String> {
    UserConfig::update_at(path, update)
}
