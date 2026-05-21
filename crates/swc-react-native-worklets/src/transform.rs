// Corresponds to `transform.ts` in
// react-native-reanimated/packages/react-native-worklets/plugin/src/
// (the `workletTransformSync` entry point used by `workletFactory.ts`).
//
// Babel applies `extraPlugins`
// (shorthand-properties, arrow-functions, optional-chaining,
// nullish-coalescing, template-literals with `loose: true`) to the worklet
// body before it gets serialized into `init_data.code`. The runtime
// `eval`s that string on the UI thread, so it must stay portable across
// JS engines that may not understand the modern syntax.
//
// This module wires the equivalent SWC compat passes together — including
// a final `hygiene` pass so the helper temporaries produced by the
// lowering (e.g. `_ref` from optional-chaining, `_components_` from
// nullish-coalescing) get unique source-level names. Without hygiene,
// adjacent chains in the same scope collide on the same sym and the
// runtime reads the wrong slot.

use swc_common::{util::take::Take, Mark, SyntaxContext};
use swc_ecma_ast::{Expr, ExprStmt, Module, ModuleItem, Pass, Program, Stmt};
use swc_ecma_compat_es2015::{arrow, template_literal};
use swc_ecma_compat_es2022::optional_chaining_impl::{optional_chaining_impl, Config};
use swc_ecma_transformer::Options;
use swc_ecma_transforms_base::fixer::fixer;
use swc_ecma_transforms_base::hygiene::hygiene;
use swc_ecma_visit::visit_mut_pass;

/// Lower the worklet body to ES5-friendly syntax before it is serialized
/// into the `init_data.code` string.
pub fn transform_worklet(module: Module) -> Module {
    let unresolved_mark = Mark::new();

    let mut env_opts = Options::default();
    env_opts.unresolved_ctxt = SyntaxContext::empty().apply_mark(unresolved_mark);
    env_opts.env.es2015.shorthand = true;
    env_opts.env.es2020.nullish_coalescing = true;

    // `swc_ecma_transformer`'s es2020 hook only chains nullish_coalescing —
    // optional chaining lives in `swc_ecma_compat_es2022` and has to be
    // wired up directly.
    //
    // `hygiene()` disambiguates the fresh-mark helper idents the lowering
    // passes alias intermediate results to. Two adjacent optional chains
    // in the same scope each ask for a fresh mark but share the same sym,
    // so without hygiene the emitted code collides (`var _ref, _ref;`)
    // and reads the wrong slot at runtime.
    //
    // `fixer()` must run last. The lowering passes build expression trees
    // that nest a low-precedence node (e.g. the `_ref = …` assignment from
    // optional chaining) inside a higher-precedence parent (the `… !== null`
    // from nullish coalescing). swc's emitter relies on `fixer` to insert
    // the explicit `Paren` nodes — without it the serialized worklet emits
    // `_ref = a ? b : c !== null …` instead of `(_ref = a ? b : c) !== null …`,
    // which silently changes evaluation order at runtime.
    let mut pass = (
        visit_mut_pass(optional_chaining_impl(Config::default(), unresolved_mark)),
        arrow(unresolved_mark),
        template_literal(swc_ecma_compat_es2015::template_literal::Config {
            mutable_template: true,
            ..Default::default()
        }),
        env_opts.into_pass(),
        hygiene(),
        fixer(None),
    );

    let mut program = Program::Module(module);
    pass.process(&mut program);
    let Program::Module(mut module) = program else {
        unreachable!("transform_worklet: pass swapped Program kind")
    };

    // `fixer` parenthesizes a function expression that opens an expression
    // statement (so it is not parsed as a declaration). The worklet body is
    // serialized as a bare `function …` string, so unwrap that one outer
    // `Paren` — the interior precedence parens added by `fixer` stay.
    if let Some(ModuleItem::Stmt(Stmt::Expr(ExprStmt { expr, .. }))) = module.body.first_mut() {
        if let Expr::Paren(paren) = expr.as_mut() {
            *expr = paren.expr.take();
        }
    }

    module
}
