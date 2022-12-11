use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = MicroModal)]
    pub fn show(id: &str);

    #[wasm_bindgen(js_namespace = MicroModal)]
    pub fn close(id: &str);
}
