mod monaco_lox;

use std::sync::Mutex;

use clox_rs::{config, vm::VM};
use js_sys::Object;
use log::{Level, LevelFilter, Metadata, Record};
use monaco::{
    api::{CodeEditorOptions, TextModel},
    sys::editor::{BuiltinTheme, IStandaloneCodeEditor},
    yew::{CodeEditor, CodeEditorLink},
};
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

struct LogEntry {
    class: &'static str,
    message: String,
}

impl LogEntry {
    fn new(class: &'static str, message: String) -> Self {
        Self { class, message }
    }
}

#[derive(PartialEq, Clone)]
pub struct PropLogEntry {
    class: &'static str,
    message: AttrValue,
}

impl From<LogEntry> for PropLogEntry {
    fn from(e: LogEntry) -> Self {
        Self {
            class: e.class,
            message: AttrValue::from(e.message),
        }
    }
}

struct Logger {
    records: Mutex<Vec<LogEntry>>,
}

impl Logger {
    #[must_use]
    const fn new() -> Self {
        Self {
            records: Mutex::new(vec![]),
        }
    }

    fn flush_entries(&self) -> Vec<LogEntry> {
        let mut records = self.records.lock().unwrap();
        std::mem::take(&mut *records)
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    // This is not elegant at all and very monolithic, but also super simple /shrug
    fn log(&self, record: &Record) {
        if record.level() == Level::Trace {
            console_log::log(record);
        } else {
            let mut records = self.records.lock().unwrap();
            records.push(LogEntry::new(
                if record.level() <= Level::Warn {
                    "error"
                } else if record.level() == Level::Debug {
                    "debug"
                } else {
                    ""
                },
                format!("{}", record.args()),
            ));
        }
    }

    fn flush(&self) {}
}
static LOGGER: Logger = Logger::new();

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
    let default_code = include_str!("../../programs/fib_short.lox");

    // Communicate with the editor
    let text_model =
        use_state_eq(|| TextModel::create(default_code, Some(monaco_lox::ID), None).unwrap());
    // Store the code
    let code = use_state_eq(|| String::from(default_code));
    // Control behavior
    let flags = use_state_eq(|| Flags::new());
    // Store the output
    let output = use_state_eq(|| Vec::new());

    // code -> results
    {
        let code = code.clone();
        let output = output.clone();
        let flags = flags.clone();
        use_effect_with_deps(
            move |(code, _flags)| {
                let mut vm = VM::new();
                vm.interpret(code.as_bytes());
                output.set(
                    LOGGER
                        .flush_entries()
                        .into_iter()
                        .map(PropLogEntry::from)
                        .collect(),
                );
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
                log::trace!("render {editor_link:?}");
                editor_link.with_editor(|editor| {
                    // Register Ctrl/Cmd + Enter hotkey
                    let keycode = monaco::sys::KeyCode::Enter.to_value()
                        | (monaco::sys::KeyMod::ctrl_cmd() as u32);
                    let raw_editor: &IStandaloneCodeEditor = editor.as_ref();
                    raw_editor.add_command(
                        keycode.into(),
                        js_closure.as_ref().unchecked_ref(),
                        None,
                    );

                    // While we have the raw editor, also set the indentation level
                    let opts: monaco::sys::editor::ITextModelUpdateOptions =
                        Object::new().unchecked_into();
                    opts.set_tab_size(Some(2.0));
                    raw_editor.get_model().unwrap().update_options(&opts);
                });
            },
            text_model,
        )
    };

    // Load examples when requested
    let on_example_selected = {
        let text_model = text_model.clone();
        let code = code.clone();
        use_callback(
            move |new_code, text_model| {
                text_model.set_value(new_code);
                code.set(String::from(new_code));
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

    html! {
        <div class="main-container">
            <div class="controls">
                <button onclick={on_run_clicked}>{ "Run (Ctrl/Cmd + Enter)" }</button>

                <Examples onchange={on_example_selected} />
                <button>{ "What am I looking at?" }</button>

                <Checkbox label="Show Bytecode" onchange={on_show_bytecode_clicked} />
                <Checkbox label="Trace Execution" onchange={on_trace_clicked} />
                <Checkbox label="STD Mode" onchange={on_std_clicked} />
                <Checkbox label="Stress GC (slow)" onchange={on_stress_gc_clicked} />
                <Checkbox label="Log GC (spammy)" onchange={on_log_gc_clicked} />
            </div>

            <div class="code-container">
                <CloxEditor {on_editor_created} text_model={(*text_model).clone()} />
                <Output entries={(*output).clone()} />
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
    entries: Vec<PropLogEntry>,
}

#[function_component]
pub fn Output(props: &OutputProps) -> Html {
    let OutputProps { entries } = props;

    html! {
        <pre class="output">
            {for entries.iter().map(|e| html! {
                <>
                    <span class={e.class}>{e.message.clone()}</span>
                    {"\n"}
                </>
            })}
        </pre>
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

#[derive(PartialEq, Properties)]
pub struct ExamplesProps {
    onchange: Callback<&'static str>,
}

#[function_component]
pub fn Examples(props: &ExamplesProps) -> Html {
    let ExamplesProps { onchange } = props;

    let onchange = onchange.clone();
    let html_on_change = use_callback(
        |e: Event, onchange| {
            let select = e.target_dyn_into::<HtmlSelectElement>();
            if let Some(select) = select {
                match select.value().as_str() {
                    "fib" => onchange.emit(include_str!("../../programs/fib_short.lox")),
                    "nested_classes" => {
                        onchange.emit(include_str!("../../programs/nested_classes.lox"))
                    }
                    "closures" => onchange.emit(include_str!("../../programs/outer.lox")),
                    _ => unimplemented!(),
                }
                select.set_value("");
            }
        },
        onchange,
    );

    html! {
        <select class="examples" onchange={html_on_change}>
            <option value="" selected={true}>{ "-- Load an Example --" }</option>
            <option value="fib">{"Fibonacci"}</option>
            <option value="closures">{"Closures"}</option>
            <option value="nested_classes">{"Nested Classes"}</option>
        </select>
    }
}

fn main() {
    monaco_lox::register_lox();
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
    yew::Renderer::<App>::new().render();
}
