mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Harmonium",
        options,
        Box::new(|_cc| {
            // Create our app component here
            let harmonium = app::HarmoniumApp::new();

            // eframe expects something that implements eframe::App
            Ok(Box::new(EguiAppWrapper { inner: harmonium }))
        }),
    )
}

struct EguiAppWrapper {
    inner: app::HarmoniumApp,
}

impl eframe::App for EguiAppWrapper {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.inner.ui(ctx);
    }
}
