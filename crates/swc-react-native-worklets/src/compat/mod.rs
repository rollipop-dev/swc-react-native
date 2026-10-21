//! Versioned adapters around the current upstream worklets implementation.

mod worklets_0_4_to_0_8;

use crate::options::WorkletsOptions;
use rustc_hash::FxHashSet;
use semver::Version;
use swc_ecma_ast::{Id, Ident};

#[derive(Clone, Copy)]
pub(crate) enum RuntimeCompat {
    Current,
    Worklets0_4To0_8,
}

impl RuntimeCompat {
    // Select once per visitor. Bundle Mode always follows the current OXC
    // plugin; unknown versions and prereleases keep the current rules too.
    pub(crate) fn for_options(options: &WorkletsOptions) -> Self {
        if options.bundle_mode {
            return Self::Current;
        }
        match Version::parse(&options.plugin_version) {
            Ok(version)
                if version.major == 0
                    && (4..=8).contains(&version.minor)
                    && version.pre.is_empty() =>
            {
                Self::Worklets0_4To0_8
            }
            _ => Self::Current,
        }
    }

    pub(crate) fn filter_closure(self, closure: &mut Vec<Ident>, bindings: &FxHashSet<Id>) {
        match self {
            Self::Current => {}
            Self::Worklets0_4To0_8 => worklets_0_4_to_0_8::filter_closure(closure, bindings),
        }
    }
}
