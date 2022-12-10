mod monaco_lox;

use clox_rs::vm::VM;
use monaco::{
    api::CodeEditorOptions,
    sys::editor::{BuiltinTheme, IStandaloneCodeEditor},
    yew::{CodeEditor, CodeEditorLink},
};
use wasm_bindgen::{prelude::Closure, JsCast};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    // Communicate with the editor
    let editor_link = use_state_eq(|| CodeEditorLink::new());
    // Store the code
    let code = use_state_eq(|| String::new());

    // Run code, store stdout
    let stdout = if code.is_empty() {
        "".to_string()
    } else {
        let mut vm = VM::with_stdout(Vec::new());
        let _result = vm.interpret(code.as_bytes());
        std::str::from_utf8(&vm.into_stdout()).unwrap().to_string()
    };

    // Update the code when the Run button is clicked
    let update_code_closure = {
        let code = code.clone();
        let editor_link = editor_link.clone();
        move || {
            editor_link.with_editor(|editor| {
                if let Some(model) = editor.get_model() {
                    code.set(model.get_value());
                }
            });
        }
    };
    let on_run_clicked = {
        let update_code_closure = update_code_closure.clone();
        Callback::from(move |_| update_code_closure())
    };

    let on_editor_created = {
        let js_closure = Closure::<dyn Fn()>::new(update_code_closure);
        let editor_link = editor_link.clone();
        let editor_dep = editor_link.clone();
        use_callback(
            move |new_editor_link: CodeEditorLink, _deps| {
                log::info!("on_editor_created {new_editor_link:?}");
                new_editor_link.with_editor(|editor| {
                    // Register Ctrl/Cmd + Enter to run the code
                    let keycode = monaco::sys::KeyCode::Enter.to_value()
                        | (monaco::sys::KeyMod::ctrl_cmd() as u32);
                    let raw_editor: &IStandaloneCodeEditor = editor.as_ref();
                    raw_editor.add_command(
                        keycode.into(),
                        js_closure.as_ref().unchecked_ref(),
                        None,
                    );
                });

                // Store the (new) editor
                editor_link.set(new_editor_link);
            },
            (editor_dep,),
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
                <CloxEditor {on_editor_created} link={(*editor_link).clone()} />
                <pre id="output" class="output">{ &*stdout }</pre>
            </div>
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct CloxEditorProps {
    on_editor_created: Callback<CodeEditorLink>,
    link: CodeEditorLink,
}

#[function_component]
pub fn CloxEditor(props: &CloxEditorProps) -> Html {
    let CloxEditorProps {
        on_editor_created,
        link,
    } = props;
    let options = CodeEditorOptions::default()
        .with_language(monaco_lox::ID.to_string())
        .with_builtin_theme(BuiltinTheme::Vs)
        .with_value("print 3;".to_string())
        .with_automatic_layout(true);
    html! {
        <CodeEditor classes={"code"} options={ options.to_sys_options() } {on_editor_created} link={link.clone()} />
    }
}

fn main() {
    monaco_lox::register_lox();
    console_log::init_with_level(log::Level::Trace).unwrap();
    yew::Renderer::<App>::new().render();
}
