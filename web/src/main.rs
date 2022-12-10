mod monaco_lox;

use clox_rs::vm::VM;
use monaco::{
    api::{CodeEditorOptions, TextModel},
    sys::editor::{BuiltinTheme, IStandaloneCodeEditor},
    yew::{CodeEditor, CodeEditorLink},
};
use wasm_bindgen::{prelude::Closure, JsCast};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    // Communicate with the editor
    let text_model =
        use_state_eq(|| TextModel::create("print 3;", Some(monaco_lox::ID), None).unwrap());
    // Store the code
    let code = use_state_eq(|| String::new());
    // Store the result
    let stdout = use_state_eq(|| String::new());

    // code -> stdout
    {
        let code = code.clone();
        let stdout = stdout.clone();
        use_effect_with_deps(
            move |code| {
                let mut vm = VM::with_stdout(Vec::new());
                let _result = vm.interpret(code.as_bytes());
                stdout.set(std::str::from_utf8(&vm.into_stdout()).unwrap().to_string());
            },
            code,
        )
    };

    // text_model -> code on button click
    let on_run_clicked = {
        let text_model = text_model.clone();
        let code = code.clone();
        use_callback(
            move |_, text_model| code.set(text_model.get_value()),
            text_model,
        )
    };

    // text_model -> code on hotkey
    let on_editor_created = {
        let text_model = text_model.clone();
        let code = code.clone();

        let js_closure = {
            let code = code.clone();
            let text_model = text_model.clone();
            Closure::<dyn Fn()>::new(move || {
                code.set(text_model.get_value());
            })
        };

        use_callback(
            move |editor_link: CodeEditorLink, _text_model| {
                log::info!("render {editor_link:?}");
                editor_link.with_editor(|editor| {
                    let keycode = monaco::sys::KeyCode::Enter.to_value()
                        | (monaco::sys::KeyMod::ctrl_cmd() as u32);
                    let raw_editor: &IStandaloneCodeEditor = editor.as_ref();
                    raw_editor.add_command(
                        keycode.into(),
                        js_closure.as_ref().unchecked_ref(),
                        None,
                    );
                });
            },
            text_model,
        )
    };

    html! {
        <div class="main-container">
            <div class="controls">
                <button id="run" onclick={on_run_clicked}>{ "Run (Ctrl/Cmd + Enter)" }</button>
                <select class="examples">
                <option value="">{ "-- Load an Example --" }</option>
                </select>
                <button id="help-button">
                    { "What am I looking at?" }
                </button>
            </div>

            <div class="code-container">
                <CloxEditor {on_editor_created} text_model={(*text_model).clone()} />
                <pre id="output" class="output">{ &*stdout }</pre>
            </div>
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct CloxEditorProps {
    on_editor_created: Callback<CodeEditorLink>,
    text_model: TextModel,
}

#[function_component]
pub fn CloxEditor(props: &CloxEditorProps) -> Html {
    let CloxEditorProps {
        on_editor_created,
        text_model,
    } = props;
    let options = CodeEditorOptions::default()
        .with_language(monaco_lox::ID.to_string())
        .with_builtin_theme(BuiltinTheme::Vs)
        .with_automatic_layout(true);
    html! {
        <CodeEditor classes={"code"} options={ options.to_sys_options() } {on_editor_created} model={text_model.clone()} />
    }
}

fn main() {
    monaco_lox::register_lox();
    console_log::init_with_level(log::Level::Trace).unwrap();
    yew::Renderer::<App>::new().render();
}
