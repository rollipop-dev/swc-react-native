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

/// Options shared by both runtime transforms.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CommonOptions {
    /// Additional runtime globals. Worklets captures shadowing local bindings;
    /// Reanimated 3 excludes these names even when bound.
    pub globals: Vec<String>,
    /// Omit native init data, for example when targeting the web.
    pub omit_native_only_data: bool,
    /// Disable source maps in serialized worklet init data.
    pub disable_source_maps: bool,
    /// Emit source locations relative to `cwd`.
    pub relative_source_location: bool,
    /// Suppress warnings for shared values used directly in inline styles.
    pub disable_inline_styles_warning: bool,
    /// Source filename used for naming, source maps and init data locations.
    pub filename: Option<String>,
    /// Working directory for relative paths and package discovery.
    /// Defaults to the process working directory.
    pub cwd: Option<String>,
    /// Omit development metadata such as stack details, version and location.
    pub is_release: bool,
    /// Fold `isWeb()` and `shouldBeUseWeb()` in host code; ignored in Bundle Mode.
    pub substitute_web_platform_checks: bool,
}

/// Configuration for the current `react-native-worklets` implementation.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkletsOptions {
    #[serde(flatten)]
    pub common: CommonOptions,
    /// Installed Worklets package version, emitted as `__pluginVersion`.
    pub plugin_version: String,
    /// Bytecode generation is currently unsupported and reports a diagnostic.
    pub hermes_bytecode: bool,
    #[serde(skip)]
    /// Bytecode compiler resolver; unused while bytecode generation is unsupported.
    pub get_hbc_binary: Option<HbcBinaryResolver>,
    /// Do not implicitly capture unbound names in Legacy Eval.
    pub strict_global: bool,
    /// Disable the Worklet Class factory transformation.
    pub disable_worklet_classes: bool,
    /// Emit worklet modules into the installed Worklets `.worklets` directory.
    pub bundle_mode: bool,
    /// Retained upstream compatibility option; the directive controls hoisting.
    pub limit_init_data_hoisting: bool,
    /// Extend Bundle Mode import-forwarding defaults.
    pub import_forwarding: ImportForwardingOptions,
    /// Deprecated upstream option retained for JSON compatibility.
    pub workletizable_modules: Vec<String>,
}

/// Select the runtime explicitly in Rust. JSON retains flat version fields.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Options {
    Reanimated(crate::compat::ra3::ReanimatedOptions),
    Worklets(WorkletsOptions),
}

impl Default for Options {
    fn default() -> Self {
        Self::Worklets(WorkletsOptions::default())
    }
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value
            .get("pluginVersion")
            .is_some_and(|version| !version.is_null())
        {
            return serde_json::from_value(value)
                .map(Self::Worklets)
                .map_err(serde::de::Error::custom);
        }
        if value
            .get("reanimatedVersion")
            .is_some_and(|version| !version.is_null())
        {
            if value.get("bundleMode").and_then(serde_json::Value::as_bool) == Some(true) {
                return Err(serde::de::Error::custom(
                    "Reanimated 3 does not support bundleMode",
                ));
            }
            return serde_json::from_value(value)
                .map(Self::Reanimated)
                .map_err(serde::de::Error::custom);
        }
        Err(serde::de::Error::custom(
            "Either pluginVersion or reanimatedVersion is required",
        ))
    }
}

impl Options {
    pub fn common(&self) -> &CommonOptions {
        match self {
            Self::Reanimated(options) => &options.common,
            Self::Worklets(options) => &options.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut CommonOptions {
        match self {
            Self::Reanimated(options) => &mut options.common,
            Self::Worklets(options) => &mut options.common,
        }
    }

    pub fn is_reanimated(&self) -> bool {
        matches!(self, Self::Reanimated(_))
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Reanimated(options) => &options.reanimated_version,
            Self::Worklets(options) if !options.plugin_version.is_empty() => {
                &options.plugin_version
            }
            Self::Worklets(_) => "unknown",
        }
    }

    pub fn bundle_mode(&self) -> bool {
        matches!(self, Self::Worklets(options) if options.bundle_mode)
    }

    pub fn strict_global(&self) -> bool {
        matches!(self, Self::Worklets(options) if options.strict_global)
    }

    pub fn disable_worklet_classes(&self) -> bool {
        matches!(self, Self::Worklets(options) if options.disable_worklet_classes)
    }

    pub fn hermes_bytecode(&self) -> bool {
        matches!(self, Self::Worklets(options) if options.hermes_bytecode)
    }

    pub fn process_nested_worklets(&self) -> bool {
        matches!(self, Self::Reanimated(options) if options.process_nested_worklets)
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Reanimated(options) => crate::compat::ra3::validate_options(options),
            Self::Worklets(_) => Ok(()),
        }
    }
}

impl From<WorkletsOptions> for Options {
    fn from(options: WorkletsOptions) -> Self {
        Self::Worklets(options)
    }
}

impl From<crate::compat::ra3::ReanimatedOptions> for Options {
    fn from(options: crate::compat::ra3::ReanimatedOptions) -> Self {
        Self::Reanimated(options)
    }
}
