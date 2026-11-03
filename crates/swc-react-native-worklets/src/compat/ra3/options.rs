//! Options and validation for the Reanimated 3 Babel adapter.

use crate::options::CommonOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReanimatedOptions {
    #[serde(flatten)]
    pub common: CommonOptions,
    /// Installed Reanimated version. Only major version 3 is supported.
    pub reanimated_version: String,
    /// Transform nested worklets before serializing their parent.
    pub process_nested_worklets: bool,
    /// Babel extension hooks cannot run in the native SWC transformer.
    pub extra_plugins: Vec<String>,
    pub extra_presets: Vec<String>,
}

pub(crate) fn validate_options(options: &ReanimatedOptions) -> Result<(), String> {
    let version = &options.reanimated_version;
    if !semver::Version::parse(version).is_ok_and(|version| version.major == 3) {
        return Err(format!("Unsupported Reanimated version: {version}. Only Reanimated 3 is supported; use pluginVersion for react-native-worklets."));
    }
    if !options.extra_plugins.is_empty() || !options.extra_presets.is_empty() {
        return Err("Reanimated extraPlugins and extraPresets require Babel and are not supported by the SWC transform".into());
    }
    Ok(())
}
