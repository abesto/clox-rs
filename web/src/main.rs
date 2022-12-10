mod monaco_lox;

use clox_rs::vm::VM;
use monaco::{api::CodeEditorOptions, sys::editor::BuiltinTheme, yew::CodeEditor};
use wasm_bindgen::JsValue;
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let mut vm = VM::with_stdout(Vec::new());
    let code = r#"print "hello from clox";"#;
    let result = vm.interpret(code.as_bytes());
    let stdout = vm.into_stdout();
    let stdout = std::str::from_utf8(&stdout).unwrap();
    html! {
        <div class="main-container">
            <div class="controls">
                <button id="run">{ "Run (Ctrl/Cmd + Enter)" }</button>
                <select class="examples">
                <option value="">{ "-- Load an Example --" }</option>
                </select>
                <button id="help-button">
                    { "What am I looking at?" }
                </button>
            </div>

            <div class="code-container">
                <CloxEditor />
                <pre id="output" class="output"></pre>
            </div>
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct CloxEditorProps {}

#[function_component]
pub fn CloxEditor(props: &CloxEditorProps) -> Html {
    let CloxEditorProps {} = props;
    let options = CodeEditorOptions::default()
        .with_language(monaco_lox::ID.to_string())
        .with_builtin_theme(BuiltinTheme::Vs)
        .with_automatic_layout(true);
    html! {
        <CodeEditor classes={"code"} options={ options.to_sys_options() } />
    }
}

fn main() {
    monaco_lox::register_lox();
    yew::Renderer::<App>::new().render();
}
