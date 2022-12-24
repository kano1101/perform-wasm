(
    cd `dirname $0`
    cargo build --target wasm32-unknown-unknown
    wasm-bindgen ./target/wasm32-unknown-unknown/debug/$(basename `pwd`).wasm --out-dir . --web
    python -m http.server 8080
)
