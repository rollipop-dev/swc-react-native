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

    /// API-parity flag for the upstream `substituteWebPlatformChecks` option.
    /// When true, calls like `isWeb()` / `shouldBeUseWeb()` should be folded
    /// to `true` for web-targeted bundles. The current SWC port keeps the
    /// flag for shape compatibility but does not perform the substitution —
    /// `web::substitute_web_call_expression` is a no-op stub.
    pub substitute_web_platform_checks: bool,

    /// API-parity flag for the upstream `limitInitDataHoisting` option.
    /// Documented as a "temporary internal option to create ShareableUnpacker"
    /// — corresponds to the `'limit-init-data-hoisting'` worklet directive.
    /// Stored for parity; no behavior change in the current port.
    pub limit_init_data_hoisting: bool,

    /// API-parity list for the upstream `workletizableModules` option used by
    /// Bundle Mode to allow-list modules that are safe on worklet runtimes.
    /// Stored for parity; Bundle Mode is not implemented in the current port,
    /// so the list is not consulted yet.
    pub workletizable_modules: Vec<String>,
}
