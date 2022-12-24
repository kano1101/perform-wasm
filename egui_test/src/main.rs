#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| Box::new(Application::new(cc))),
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    log::trace!("some trace log");
    log::debug!("some debug log");
    log::info!("some info log");
    log::warn!("some warn log");
    log::error!("some error log");

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id",
            web_options,
            Box::new(|cc| Box::new(Application::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}

mod ip {
    perform_wasm::build_perform!(String);
}
use perform_wasm::{PerformState, Performer};

#[derive(PartialEq)]
enum Progress {
    Triggered,
    Off,
}

struct Application {
    session: ip::Session,
    ip_optional: (Progress, Option<String>),
}
impl Application {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Application {
        Application {
            session: ip::Session::activate_with_spawn_local(),
            ip_optional: (Progress::Off, None),
        }
    }
}
impl eframe::App for Application {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui: &mut eframe::egui::Ui| {
            if let Some(ip) = &self.ip_optional.1 {
                log::debug!("ip: {}", ip);
                ui.label(ip);
            } else {
                let took = self.session.try_take();
                if let Ok(PerformState::Done(ip)) = took {
                    self.ip_optional.1 = Some(ip);
                    self.ip_optional.0 = Progress::Off;
                } else {
                    if self.ip_optional.0 == Progress::Off {
                        let fut = async {
                            reqwest::get("http://httpbin.org/ip")
                                .await
                                .unwrap()
                                .text()
                                .await
                                .unwrap()
                        };
                        self.session.perform_with_spawn_local(fut);
                        self.ip_optional.0 = Progress::Triggered;
                    }
                    let dur = std::time::Duration::from_millis(100);
                    ctx.request_repaint_after(dur);
                }
            }
        });
    }
}
