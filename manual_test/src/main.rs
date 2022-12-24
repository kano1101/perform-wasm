async fn run() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    log::trace!("some trace log");
    log::debug!("some debug log");
    log::info!("some info log");
    log::warn!("some warn log");
    log::error!("some error log");

    let ip = reqwest::get("http://httpbin.org/ip")
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    log::debug!("ip: {:?}", ip);
}

fn main() {
    wasm_bindgen_futures::spawn_local(run());
}
