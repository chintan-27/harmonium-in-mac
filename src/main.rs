mod app;
mod bellows;
mod keymap;
mod audio;
mod sensor;

fn main() -> eframe::Result<()> {
    // Create a standard (non-async) channel to send sensor messages to the GUI.
    let (tx, rx) = std::sync::mpsc::channel::<sensor::SensorMsg>();

    // Start the sensor in a background thread.
    // It will try to connect and stream angle samples.
    // If the device isn't available, you'll see the error in the UI.
    let _sensor_thread = sensor::spawn_sensor_thread(60.0, tx);

    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Harmonium",
        options,
        Box::new(move |_cc| {
            let harmonium = app::HarmoniumApp::new(rx);

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
