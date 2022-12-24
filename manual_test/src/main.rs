#[cfg(target_arch = "wasm32")]
async fn run() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    let ip = reqwest::get("http://httpbin.org/ip")
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    log::debug!("ip: {:?}", ip);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm_bindgen_futures::spawn_local(run());
}
