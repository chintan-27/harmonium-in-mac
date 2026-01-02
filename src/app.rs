pub struct HarmoniumApp {
    // We'll store all UI state here later (sliders, values, pressed keys, etc.)
    pub counter: u32,
}

impl HarmoniumApp {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Harmonium (Phase 1)");
            ui.label("Goal: get a window running, then add bellows math and sensor input.");

            ui.separator();

            ui.label(format!("Counter = {}", self.counter));

            if ui.button("Increment").clicked() {
                self.counter += 1;
            }
        });
    }
}
