use clox_rs::vm::VM;
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let mut vm = VM::with_stdout(Vec::new());
    let code = r#"print "hello from clox";"#;
    let result = vm.interpret(code.as_bytes());
    let stdout = vm.to_stdout();
    let stdout = std::str::from_utf8(&stdout).unwrap();
    html! {
        <>
            <h1><code>{ "clox-rs web" }</code></h1>

            <h2>{ "Code" }</h2>
            <pre>{ code }</pre>

            <h2>{ "InterpretResult" }</h2>
            <pre>{ format!("{result:?}") }</pre>

            <h2>{ "STDOUT" }</h2>
            <pre>{ stdout }</pre>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
