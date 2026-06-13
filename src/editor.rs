use egui::text::LayoutJob;
use std::time::Instant;

pub struct Editor {
    pub source: String,
    pub error_message: Option<String>,
    pub fps: f32,
    last_edit: Instant,
}

impl Editor {
    pub fn new(default_source: &str) -> Self {
        Self {
            source: default_source.to_string(),
            error_message: None,
            fps: 0.0,
            last_edit: Instant::now(),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) -> bool {
        let mut changed = false;

        egui::TopBottomPanel::bottom("editor_panel")
            .resizable(true)
            .min_height(80.0)
            .default_height(200.0)
            .frame(egui::Frame {
                fill: egui::Color32::from_black_alpha(200),
                inner_margin: egui::Margin::symmetric(8, 4),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{:.0} FPS", self.fps));
                    ui.separator();
                    if self.error_message.is_some() {
                        ui.colored_label(egui::Color32::RED, "\u{26A0} COMPILE ERROR");
                    } else {
                        ui.colored_label(egui::Color32::GREEN, "\u{2713} OK");
                    }
                });

                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, egui::RichText::new(err).size(12.0));
                }

                let mut source = self.source.clone();
                let scroll_output =
                    egui::ScrollArea::vertical()
                        .max_height(f32::INFINITY)
                        .show(ui, |ui| {
                            let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(
                                ui.ctx(),
                                ui.style(),
                            );
                            let mut layouter =
                                |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                                    let mut layout_job: LayoutJob =
                                        egui_extras::syntax_highlighting::highlight(
                                            ui.ctx(),
                                            ui.style(),
                                            &theme,
                                            text.as_str(),
                                            "rs", // language hint
                                        );
                                    layout_job.wrap.max_width = wrap_width;
                                    ui.fonts(|f| f.layout_job(layout_job))
                                };

                            ui.add_sized(
                                ui.available_size(),
                                egui::TextEdit::multiline(&mut source)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .layouter(&mut layouter)
                                    .desired_width(f32::INFINITY),
                            )
                        });

                if scroll_output.inner.changed() {
                    self.source = source;
                    self.last_edit = Instant::now();
                    changed = true;
                }
            });

        changed
    }

    pub fn needs_recompile(&self, debounce_ms: u64) -> bool {
        self.last_edit.elapsed() >= std::time::Duration::from_millis(debounce_ms)
    }
}
