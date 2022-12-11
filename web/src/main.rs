mod monaco_lox;

use clox_rs::{
    config,
    vm::{InterpretResult, VM},
};
use monaco::{
    api::{CodeEditorOptions, TextModel},
    sys::editor::{BuiltinTheme, IStandaloneCodeEditor},
    yew::{CodeEditor, CodeEditorLink},
};
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(PartialEq)]
struct Flags {
    print_code: bool,
    trace_execution: bool,
    std_mode: bool,
    stress_gc: bool,
    log_gc: bool,
}

impl Flags {
    #[must_use]
    fn new() -> Self {
        Self {
            print_code: config::PRINT_CODE.load(),
            trace_execution: config::TRACE_EXECUTION.load(),
            std_mode: config::STD_MODE.load(),
            stress_gc: config::STRESS_GC.load(),
            log_gc: config::LOG_GC.load(),
        }
    }
}

#[function_component(App)]
fn app() -> Html {
    // Communicate with the editor
    let text_model =
        use_state_eq(|| TextModel::create("print 3;", Some(monaco_lox::ID), None).unwrap());
    // Store the code
    let code = use_state_eq(|| String::new());
    // Control behavior
    let flags = use_state_eq(|| Flags::new());
    // Store the result
    let stdout = use_state_eq(|| String::new());
    let stderr = use_state_eq(|| String::new());
    let interpret_result = use_state_eq(|| InterpretResult::Ok);

    // code -> results
    {
        let code = code.clone();
        let stdout = stdout.clone();
        let stderr = stderr.clone();
        let flags = flags.clone();
        let interpret_result = interpret_result.clone();
        use_effect_with_deps(
            move |(code, _flags)| {
                let mut vm = VM::with_streams(Vec::new(), Vec::new());
                interpret_result.set(vm.interpret(code.as_bytes()));
                let (ref vm_stdout, ref vm_stderr) = vm.into_streams();
                stdout.set(std::str::from_utf8(&vm_stdout).unwrap().to_string());
                stderr.set(std::str::from_utf8(&vm_stderr).unwrap().to_string());
            },
            (code, flags),
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

    // Handle checkboxes for global flags
    macro_rules! flag_handler {
        ($flags:ident, $n:ident) => {{
            let $flags = $flags.clone();
            move |checked: bool, _| {
                config::$n.store(checked);
                $flags.set(Flags::new());
            }
        }};
    }

    let on_show_bytecode_clicked = { use_callback(flag_handler!(flags, PRINT_CODE), ()) };
    let on_trace_clicked = { use_callback(flag_handler!(flags, TRACE_EXECUTION), ()) };
    let on_std_clicked = { use_callback(flag_handler!(flags, STD_MODE), ()) };
    let on_stress_gc_clicked = { use_callback(flag_handler!(flags, STRESS_GC), ()) };
    let on_log_gc_clicked = { use_callback(flag_handler!(flags, LOG_GC), ()) };

    // Trace execution?

    html! {
        <div class="main-container">
            <div class="controls">
                <button onclick={on_run_clicked}>{ "Run (Ctrl/Cmd + Enter)" }</button>

                <select class="examples">
                <option value="">{ "-- Load an Example --" }</option>
                </select>
                <button>{ "What am I looking at?" }</button>

                <Checkbox label="Show Bytecode" onchange={on_show_bytecode_clicked} />
                <Checkbox label="Trace Execution" onchange={on_trace_clicked} />
                <Checkbox label="STD Mode" onchange={on_std_clicked} />
                //<Checkbox label="Stress GC" onchange={on_stress_gc_clicked} />
                //<Checkbox label="Log GC" onchange={on_log_gc_clicked} />
            </div>

            <div class="code-container">
                <CloxEditor {on_editor_created} text_model={(*text_model).clone()} />
                <Output
                    stdout={AttrValue::from(stdout.to_string())}
                    stderr={AttrValue::from(stderr.to_string())}
                    interpret_result={*interpret_result}
                />
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

#[derive(PartialEq, Properties)]
pub struct OutputProps {
    stdout: AttrValue,
    stderr: AttrValue,
    interpret_result: InterpretResult,
}

#[function_component]
pub fn Output(props: &OutputProps) -> Html {
    let OutputProps {
        stdout,
        stderr,
        interpret_result,
    } = props;

    let error_message = match interpret_result {
        InterpretResult::Ok => None,
        InterpretResult::CompileError => Some("Compile Error"),
        InterpretResult::RuntimeError => Some("Runtime Error"),
    };

    html! {
        <div class="output">
            <pre class="stdout">{stdout}</pre>
            <span class="status">{error_message}</span>
            <pre class="stderr">{stderr}</pre>
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct CheckboxProps {
    label: AttrValue,
    onchange: Callback<bool>,
}

#[function_component]
pub fn Checkbox(props: &CheckboxProps) -> Html {
    let CheckboxProps { label, onchange } = props;

    let onchange = onchange.clone();
    let html_change_handler = use_callback(
        move |e: Event, _| {
            let input = e.target_dyn_into::<HtmlInputElement>();
            if let Some(input) = input {
                onchange.emit(input.checked());
            };
        },
        (),
    );
    html! {
        <label>
            <input type="checkbox" onchange={html_change_handler} />
            {label}
        </label>
    }
}

fn main() {
    monaco_lox::register_lox();
    console_log::init_with_level(log::Level::Trace).unwrap();
    yew::Renderer::<App>::new().render();
}
