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

struct Application {
    taker: ip::Taker,
    response: Option<String>,
    displaying_bg_color_step: f32,
}
impl Application {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Application {
        use perform_wasm::Performer as _;
        let session = ip::Session::activate_with_spawn_local();
        let taker = ip::Taker::new(session);
        Application {
            taker: taker,
            response: None,
            displaying_bg_color_step: 0.,
        }
    }
}
impl eframe::App for Application {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui: &mut eframe::egui::Ui| {
            let fut = async {
                reqwest::get("http://httpbin.org/ip")
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap()
            };
            if let None = self.response {
                self.response = self.taker.try_take(fut);
            }
            let micro = "Now loading...".to_string();
            let color_weight_max = 255.;
            self.response
                .as_ref()
                .or_else(|| {
                    self.displaying_bg_color_step = color_weight_max;
                    let dur = std::time::Duration::from_millis(250);
                    ctx.request_repaint_after(dur);
                    log::debug!("Request repaint again!");
                    Some(&micro)
                })
                .and_then(|state| {
                    let highlight_color = if color_weight_max == self.displaying_bg_color_step {
                        eframe::egui::Color32::LIGHT_RED.to_array()
                    } else {
                        eframe::egui::Color32::DARK_GREEN.to_array()
                    };

                    let tone_down_speed = 8.;
                    let mut tone = self.displaying_bg_color_step - tone_down_speed;
                    if tone < 0. {
                        self.displaying_bg_color_step = 0.;
                        tone = 0.;
                    } else {
                        self.displaying_bg_color_step = tone;
                        let dur = std::time::Duration::from_millis(16);
                        ctx.request_repaint_after(dur);
                    }
                    log::debug!("tone: {}", tone);

                    let visuals = ui.visuals_mut();
                    let base_color = visuals.extreme_bg_color.to_array();
                    let mut finish_color = visuals.extreme_bg_color.to_array();

                    use itertools::izip;
                    for (base, highlight, finish) in
                        izip!(&base_color, &highlight_color, &mut finish_color)
                    {
                        let current_weight = tone;
                        let diff = *highlight as f32 - *base as f32;
                        let rate = current_weight / color_weight_max;
                        *finish = (*base as f32 + (diff * rate)) as u8;
                    }
                    let [r, g, b, _a] = finish_color;
                    let finish_color = eframe::egui::Color32::from_rgb(r, g, b);
                    visuals.extreme_bg_color = finish_color;

                    let mut state = state.clone();
                    ui.text_edit_singleline(&mut state);
                    log::debug!("Color change testing");
                    Some(state)
                });
        });
    }
}
