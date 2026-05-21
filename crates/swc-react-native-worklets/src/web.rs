// Corresponds to `webOptimization.ts` + `substituteWebCallExpression.ts` in
// react-native-reanimated/packages/react-native-worklets/plugin/src/.
//
// Babel's substitution folds `isWeb()` / `shouldBeUseWeb()` to the boolean
// literal `true` so dead-code elimination on web bundles can strip
// reanimated's platform-specific branches.
//
// The SWC port keeps the public surface (option flag in `WorkletsOptions`
// and this entry point invoked from the visitor) but does not perform the
// substitution yet — the function is a no-op stub. The caller still pays a
// per-CallExpression check, but the AST is left untouched.

use swc_ecma_ast::CallExpr;

/// Returns `true` if the call expression was substituted, `false` otherwise.
/// Always returns `false` in the current port — kept as a stub so wiring at
/// the visitor stays stable when the real implementation lands.
pub fn substitute_web_call_expression(_call: &mut CallExpr) -> bool {
    false
}
