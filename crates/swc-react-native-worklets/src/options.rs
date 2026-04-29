// Corresponds to `options.ts` in
// react-native-reanimated/packages/react-native-worklets/plugin/src/.

use serde::{Deserialize, Serialize};

/// Configuration for the worklets transform.
///
/// Field semantics mirror the upstream `react-native-worklets` Babel plugin
/// (camelCase JSON keys when consumed via JSON config).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkletsOptions {
    /// Identifiers treated as globals — never captured into worklet closures.
    pub globals: Vec<String>,

    /// When true, no globals are implicitly captured: identifiers must be
    /// explicitly listed in `globals` to be considered safe.
    pub strict_global: bool,

    /// Omit native-only data (`init_data`) from the output. Useful for web
    /// builds.
    pub omit_native_only_data: bool,

    /// Disable source map generation for worklets.
    pub disable_source_maps: bool,

    /// Use paths relative to `cwd` for source locations.
    pub relative_source_location: bool,

    /// Disable Worklet Classes support.
    pub disable_worklet_classes: bool,

    /// Suppress the inline-shared-values warning.
    pub disable_inline_styles_warning: bool,

    /// Enable Bundle Mode.
    pub bundle_mode: bool,

    /// Filename of the file being transformed (used for source map output and
    /// `init_data.location`).
    pub filename: Option<String>,

    /// Working directory used when computing relative source locations.
    /// Defaults to `std::env::current_dir()` when unset.
    pub cwd: Option<String>,

    /// Release builds skip debug info such as stack details, version, and
    /// location.
    pub is_release: bool,

    /// Version string emitted as `__pluginVersion`. Required — callers must
    /// supply the installed `react-native-worklets` package version.
    pub plugin_version: String,
}
