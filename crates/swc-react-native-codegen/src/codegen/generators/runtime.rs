// Runtime expressions used by upstream GenerateViewConfigJs.js. Keep older
// React Native behavior in `legacy` so the current upstream path stays visible.
mod legacy;

pub(super) fn color_attribute() -> String {
    format!(
        "require('react-native/Libraries/Components/View/ReactNativeStyleAttributes').colorAttribute ?? {}",
        legacy::color_attribute()
    )
}

pub(super) fn process(module: &str) -> String {
    format!("{{ process: {} }}", default_export(module))
}

pub(super) fn diff(module: &str) -> String {
    format!("{{ diff: {} }}", default_export(module))
}

fn default_export(module: &str) -> String {
    format!(
        "((req) => 'default' in req ? req.default : req)(require('react-native/Libraries/{module}'))"
    )
}
