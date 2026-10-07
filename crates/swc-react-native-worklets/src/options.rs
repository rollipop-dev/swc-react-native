// Existing public API adapter for plugin-oxc/src/options.rs and the Legacy
// Eval options in plugin/src/options.ts.

use serde::{Deserialize, Serialize};

/// Resolves the Hermes bytecode compiler binary used by the experimental
/// `hermesBytecode` option.
pub type HbcBinaryResolver = fn() -> String;

/// Bundle Mode import-forwarding configuration.
///
/// Corresponds to `ImportForwarding` in the official OXC Rust plugin.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportForwardingOptions {
    /// Module names whose imports can be forwarded into generated worklet
    /// files.
    pub module_names: Vec<String>,

    /// Path segments whose relative imports can be forwarded into generated
    /// worklet files.
    pub relative_paths: Vec<String>,
}

/// Configuration for the worklets transform.
///
/// Keeps the existing camelCase JSON API while using the official OXC Rust
/// rules for Bundle Mode and Babel rules for Legacy Eval.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkletsOptions {
    /// Unbound identifiers treated as globals in Legacy Eval. Local bindings
    /// with these names are still captured. Bundle Mode captures only bindings.
    pub globals: Vec<String>,

    /// Request production Legacy Eval worklets as Hermes bytecode.
    ///
    /// Retained for upstream option compatibility. The current SWC port does
    /// not support bytecode generation and reports an SWC error when bytecode
    /// would be generated. Ignored in Bundle Mode and development builds.
    pub hermes_bytecode: bool,

    /// Resolves the Hermes bytecode compiler binary.
    ///
    /// Function-valued options cannot be represented in JSON, so this field
    /// is available to Rust callers and skipped by serde. It remains unused
    /// while `hermes_bytecode` is unsupported.
    #[serde(skip)]
    pub get_hbc_binary: Option<HbcBinaryResolver>,

    /// When true, no unbound globals are implicitly captured in Legacy Eval.
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
    ///
    /// Emits modules to the installed `react-native-worklets/.worklets`
    /// directory using the official OXC Rust implementation's factory and
    /// import-forwarding rules. Package discovery uses `filename` and `cwd`.
    pub bundle_mode: bool,

    /// Filename of the file being transformed (used for source map output and
    /// `init_data.location`).
    pub filename: Option<String>,

    /// Working directory for relative source locations and package discovery.
    /// Defaults to `std::env::current_dir()` when unset.
    pub cwd: Option<String>,

    /// Release builds skip debug info such as stack details, version, and
    /// location.
    pub is_release: bool,

    /// Version string emitted as `__pluginVersion`. Required — callers must
    /// supply the installed `react-native-worklets` package version.
    pub plugin_version: String,

    /// Folds `isWeb()` and `shouldBeUseWeb()` calls to `true` in Legacy Eval.
    /// Bundle Mode ignores this Legacy-specific option, matching OXC.
    pub substitute_web_platform_checks: bool,

    /// Compatibility field for the upstream temporary internal option.
    /// Hoisting is controlled by the `'limit-init-data-hoisting'` directive,
    /// matching the current Babel implementation; this field is retained.
    pub limit_init_data_hoisting: bool,

    /// Bundle Mode import-forwarding options.
    ///
    /// Extends the official OXC defaults for forwardable modules and paths.
    pub import_forwarding: ImportForwardingOptions,

    /// Deprecated compatibility field for the removed upstream
    /// `workletizableModules` option.
    ///
    /// Kept so existing JSON configs continue to deserialize while callers
    /// migrate to `importForwarding`.
    pub workletizable_modules: Vec<String>,
}
