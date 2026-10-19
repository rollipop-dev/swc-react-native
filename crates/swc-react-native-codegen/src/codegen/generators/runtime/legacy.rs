// Compatibility with RN 0.84 GenerateViewConfigJs.js. ReactNativeStyleAttributes
// has no named colorAttribute export in that release.

pub(super) fn color_attribute() -> String {
    super::process("StyleSheet/processColor")
}
