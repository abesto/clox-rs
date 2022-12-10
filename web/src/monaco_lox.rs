use js_sys::{Array, Object};
use monaco::sys::languages::{ILanguageExtensionPoint, LanguageConfiguration};
use wasm_bindgen::{prelude::*, JsCast, JsValue};

pub const ID: &str = "lox";

pub fn register_lox() {
    monaco::sys::languages::register(&language());
    monaco::sys::languages::set_monarch_tokens_provider(ID, &make_tokens_provider().into());
    monaco::sys::languages::set_language_configuration(ID, &language_configuration());
}

fn language() -> ILanguageExtensionPoint {
    let lang: ILanguageExtensionPoint = Object::new().unchecked_into();
    lang.set_id(ID);
    lang
}

#[wasm_bindgen(module = "/js/loxMonarchTokensProvider.js")]
extern "C" {
    #[wasm_bindgen(js_name = "makeTokensProvider")]
    fn make_tokens_provider() -> Object;
}

fn language_configuration() -> LanguageConfiguration {
    // I'm sure there's a neater way of doing this but failed to figure it out in like 2 minutes so /shrug
    let brackets = Array::new_with_length(2);
    {
        let pair = Array::new_with_length(2);
        pair.set(0, JsValue::from_str("("));
        pair.set(1, JsValue::from_str(")"));
        brackets.set(0, pair.into());
    }
    {
        let pair = Array::new_with_length(2);
        pair.set(0, JsValue::from_str("{"));
        pair.set(1, JsValue::from_str("}"));
        brackets.set(1, pair.into());
    }

    let cfg: LanguageConfiguration = Object::new().unchecked_into();
    cfg.set_brackets(Some(&brackets));
    cfg
}
