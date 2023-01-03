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

struct SinglelineMyText {
    taker: ip::Taker,
    response: Option<String>,
    update_request_interval: std::time::Duration,
    update_request_duration: std::time::Duration,
    micro_copy: String,
    highlighter: ColorTone,
}
impl SinglelineMyText {
    pub fn new() -> Self {
        use perform_wasm::Performer as _;
        let session = ip::Session::activate_with_spawn_local();
        let taker = ip::Taker::new(session);
        Self {
            taker: taker,
            response: None,
            update_request_interval: std::time::Duration::from_millis(250),
            update_request_duration: std::time::Duration::from_millis(16),
            micro_copy: "Now loading...".to_string(),
            highlighter: ColorTone::new(),
        }
    }
    fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut eframe::egui::Ui) {
        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        if self.is_take_required() {
            let took = self.taker.try_take(fut);
            self.take_necessity(took);
        }
        self.response
            .as_ref()
            .or_else(|| {
                self.highlighter.to_top();
                ctx.request_repaint_after(self.update_request_interval);
                log::trace!("Request repaint again!");
                Some(&self.micro_copy)
            })
            .and_then(|state| {
                if let Some(()) = self.highlighter.styling(ui) {
                    ctx.request_repaint_after(self.update_request_duration);
                }

                let mut state = state.clone();
                ui.text_edit_singleline(&mut state);
                let is_enter_pressed = ui
                    .input_mut()
                    .consume_key(eframe::egui::Modifiers::NONE, eframe::egui::Key::Enter);
                if is_enter_pressed {
                    self.highlighter.commit();
                }
                Some(state)
            });
    }
    fn is_take_required(&self) -> bool {
        self.response.is_none()
    }
    fn take_necessity(&mut self, took: Option<String>) {
        self.response = took
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ColorTone {
    Step(f32),
    Top,
}
impl ColorTone {
    pub fn new() -> Self {
        Self::Top
    }
    pub fn zero() -> Self {
        Self::Step(0.)
    }
    pub fn top_value() -> f32 {
        256.
    }
    fn tone_down() -> f32 {
        8.
    }
}
trait TopToDown {
    type Value;
    fn to_top(&mut self);
    fn current_value(&self) -> f32;
}
impl TopToDown for ColorTone {
    type Value = Self;
    fn to_top(&mut self) {
        *self = Self::Top
    }
    fn current_value(&self) -> f32 {
        match self {
            Self::Step(step) => *step,
            Self::Top => Self::top_value(),
        }
    }
}
trait Highlighter {
    fn coloring(&self) -> eframe::egui::Color32;
}
impl Highlighter for ColorTone {
    fn coloring(&self) -> eframe::egui::Color32 {
        if *self == Self::Top {
            eframe::egui::Color32::DARK_GREEN
        } else {
            eframe::egui::Color32::LIGHT_BLUE
        }
    }
}
impl Iterator for ColorTone {
    type Item = ColorTone;
    fn next(&mut self) -> Option<ColorTone> {
        match self {
            ColorTone::Step(tone) => {
                let tone = *tone - Self::tone_down();
                if tone <= 0. {
                    *self = ColorTone::Step(0.);
                    None
                } else {
                    *self = ColorTone::Step(tone);
                    Some(self.clone())
                }
            }
            ColorTone::Top => {
                *self = ColorTone::Step(Self::top_value());
                Some(self.clone())
            }
        }
    }
}
impl Default for ColorTone {
    fn default() -> ColorTone {
        ColorTone::zero()
    }
}
trait ToneHighlighter: Iterator + TopToDown + Highlighter {}
impl ToneHighlighter for ColorTone {}

trait CommitableHighlighter: ToneHighlighter {
    fn commit(&mut self);
    fn styling(&mut self, ui: &mut eframe::egui::Ui) -> Option<()>
    where
        <Self as Iterator>::Item: std::fmt::Debug + Clone + Default + TopToDown,
    {
        let visuals = ui.visuals_mut();

        let base_color = visuals.extreme_bg_color.to_array();
        let highlight_color = self.coloring().to_array();
        let mut finish_color = visuals.extreme_bg_color.to_array();

        let highlighted = self.next();
        let current_weight = highlighted.clone().unwrap_or_default();
        log::trace!("current_weight: {:?}", current_weight);

        use itertools::izip;
        for (base, highlight, finish) in izip!(&base_color, &highlight_color, &mut finish_color) {
            let diff = *highlight as f32 - *base as f32;
            let rate = current_weight.current_value() / ColorTone::top_value();
            *finish = (*base as f32 + (diff * rate)) as u8;
        }

        let [r, g, b, _a] = finish_color;
        let finish_color = eframe::egui::Color32::from_rgb(r, g, b);

        visuals.extreme_bg_color = finish_color;

        highlighted.map(|_| ())
    }
}
impl CommitableHighlighter for ColorTone {
    fn commit(&mut self) {
        self.to_top()
    }
}

struct Application {
    text: SinglelineMyText,
}
impl Application {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            text: SinglelineMyText::new(),
        }
    }
}
impl Application {}
impl eframe::App for Application {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui: &mut eframe::egui::Ui| {
            self.text.update(ctx, ui);
        });
    }
}
