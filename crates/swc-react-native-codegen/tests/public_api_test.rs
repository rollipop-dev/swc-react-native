use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Module, Pass, Program};
use swc_ecma_codegen::to_code_default;
use swc_ecma_parser::{parse_file_as_module, FlowSyntax, Syntax, TsSyntax};
use swc_ecma_visit::VisitMutWith;
use swc_react_native_codegen::{codegen, CodegenOptions, CodegenVisitor};

fn parse(filename: &str, source: &str) -> (Lrc<SourceMap>, Module) {
    let cm: Lrc<SourceMap> = Default::default();
    let file = cm.new_source_file(FileName::Custom(filename.into()).into(), source.to_string());
    let syntax = if filename.ends_with(".ts") {
        Syntax::Typescript(TsSyntax::default())
    } else {
        Syntax::Flow(FlowSyntax {
            all: true,
            ..Default::default()
        })
    };
    let module =
        parse_file_as_module(&file, syntax, Default::default(), None, &mut Vec::new()).unwrap();
    (cm, module)
}

#[test]
fn public_pass_and_visitor_preserve_the_same_output() {
    assert_eq!(CodegenOptions::default().filename, "");
    for filename in ["CompatNativeComponent.ts", "CompatNativeComponent.js"] {
        let (cm, module) = parse(
            filename,
            r#"
            import type {ViewProps} from 'react-native';
            import codegenNativeComponent from 'react-native/Libraries/Utilities/codegenNativeComponent';
            interface Props extends ViewProps { enabled?: boolean }
            export default codegenNativeComponent<Props>('CompatView');
        "#,
        );
        let options = CodegenOptions {
            filename: filename.into(),
        };
        let mut program = Program::Module(module.clone());
        codegen(cm.clone(), options.clone()).process(&mut program);
        let mut output = module;
        let mut visitor = CodegenVisitor::new(cm.clone(), options);
        output.visit_mut_with(&mut visitor);
        visitor.into_result().unwrap();
        let emitted = to_code_default(cm.clone(), None, &program);
        assert_eq!(emitted, to_code_default(cm, None, &output));
        assert!(emitted.contains("__INTERNAL_VIEW_CONFIG"));
        assert!(emitted.contains("enabled: true"));
    }
}

#[test]
fn public_pass_preserves_native_modules_and_failed_transforms() {
    for filename in ["NativeCompat.ts", "NativeCompat.js"] {
        for source in [
            "import type {TurboModule} from 'react-native';
             import {TurboModuleRegistry} from 'react-native';
             type Value = Value;
             export interface Spec extends TurboModule { getValue(): Value; }
             export default TurboModuleRegistry.getEnforcing<Spec>('Compat');",
            "export const Commands = 1;",
        ] {
            let (cm, module) = parse(filename, source);
            let original = to_code_default(cm.clone(), None, &module);
            let options = CodegenOptions {
                filename: filename.into(),
            };
            let mut program = Program::Module(module.clone());
            codegen(cm.clone(), options.clone()).process(&mut program);
            assert_eq!(to_code_default(cm.clone(), None, &program), original);
            let mut output = module;
            let mut visitor = CodegenVisitor::new(cm.clone(), options);
            output.visit_mut_with(&mut visitor);
            assert_eq!(visitor.into_result().is_err(), source.contains("Commands"));
            assert_eq!(to_code_default(cm, None, &output), original);
        }
    }
}
