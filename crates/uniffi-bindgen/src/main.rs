//! Generates foreign-language bindings for xel-ffi. Run through `scripts/task build`.

fn main() {
    uniffi::uniffi_bindgen_main();
}
