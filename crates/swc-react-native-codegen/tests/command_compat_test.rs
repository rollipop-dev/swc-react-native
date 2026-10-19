mod common;

use std::process::Command;

use common::transform_fixture;

fn check_command(typescript: bool, declaration: &str) {
    let (filename, props) = if typescript {
        (
            "CommandNativeComponent.ts",
            "interface NativeProps extends ViewProps {}",
        )
    } else {
        (
            "CommandNativeComponent.js",
            "type NativeProps = $ReadOnly<{...ViewProps}>;",
        )
    };
    let code = format!(
        r#"
import type {{ViewProps, HostComponent}} from 'react-native';
import type {{Int32}} from 'CodegenTypes';
{props}
type UpdateCommand = (viewRef: React.ElementRef<HostComponent<NativeProps>>, value: Int32, enabled: boolean) => void;
type UpdateAlias = UpdateCommand;
interface NativeCommands {{ {declaration} }}
export const Commands = codegenNativeCommands<NativeCommands>({{supportedCommands: ['update']}});
export default codegenNativeComponent<NativeProps>('CommandView');
"#
    );
    let output = transform_fixture(filename, &code).unwrap();
    let (_, commands) = output
        .rsplit_once("export const Commands = ")
        .unwrap_or_else(|| panic!("Missing Commands export for {declaration}: {output}"));
    let script = format!(
        r#"
const assert = require('node:assert/strict');
const calls = [];
const dispatchCommand = (...args) => calls.push(args);
const Commands = {commands}
const ref = {{nativeTag: 7}};
Commands.update(ref, 42, false);
assert.equal(calls[0][0], ref);
assert.deepEqual(calls, [[ref, 'update', [42, false]]]);
"#
    );
    let result = Command::new("node")
        .args(["-e", &script])
        .output()
        .expect("Node.js is required for codegen runtime tests");
    assert!(
        result.status.success(),
        "{declaration}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn typescript_method_commands_dispatch_arguments() {
    // RN 0.84's COMMANDS_DEFINED_WITH_ALL_TYPES fixture includes method syntax.
    check_command(
        true,
        "update(viewRef: React.ElementRef<HostComponent<NativeProps>>, value: Int32, enabled: boolean): void;",
    );
}

#[test]
fn typescript_aliased_commands_dispatch_arguments() {
    // These forms are accepted by the RN 0.84 Babel codegen parser.
    for annotation in [
        "UpdateCommand",
        "UpdateAlias",
        "UpdateCommand | null | undefined",
        "(UpdateCommand)",
        "Readonly<UpdateCommand>",
    ] {
        check_command(true, &format!("update: {annotation};"));
    }
}

#[test]
fn flow_aliased_commands_dispatch_arguments() {
    check_command(false, "update: UpdateAlias;");
}

#[test]
fn flow_method_commands_dispatch_arguments() {
    check_command(
        false,
        "update(viewRef: React.ElementRef<HostComponent<NativeProps>>, value: Int32, enabled: boolean): void;",
    );
}
