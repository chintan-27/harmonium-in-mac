use std::time::Instant;

use crate::audio::AudioEngine;
use crate::bellows::{BellowsOutput, BellowsParams, BellowsState};
use crate::keymap::{KeyMap, PressedKeys};
use crate::sensor::{SensorMsg, SensorSample};

pub struct HarmoniumApp {
    // ---- Sensor channel (real angle input) ----
    rx: std::sync::mpsc::Receiver<SensorMsg>,
    sensor_status: String,
    sensor_error: Option<String>,
    latest_sample: Option<SensorSample>,
    last_sample_age_sec: f32,

    // ---- Time / fake input ----
    start_time: Instant,
    fake_enabled: bool,
    fake_frequency_hz: f32,
    fake_amplitude_deg: f32,

    // ---- Bellows ----
    bellows: BellowsState,
    bellows_out: BellowsOutput,

    // ---- Keymap / input ----
    keymap: Option<KeyMap>,
    keymap_error: Option<String>,
    pressed: PressedKeys,

    // ---- Audio ----
    audio: Option<AudioEngine>,
    audio_error: Option<String>,
    master_gain: f32,
    audio_enabled: bool,
}

impl HarmoniumApp {
    pub fn new(rx: std::sync::mpsc::Receiver<SensorMsg>) -> Self {
        // Try loading keymap.json from the current working directory.
        let (keymap, keymap_error) = match KeyMap::load_from_file("key-map.json") {
            Ok(km) => (Some(km), None),
            Err(e) => (None, Some(e)),
        };

        // Create bellows math state
        let params = BellowsParams::default();
        let bellows = BellowsState::new(params);

        // Try creating audio engine (will fail if no audio device etc.)
        let (audio, audio_error) = match AudioEngine::new("harmonium-sounds") {
            Ok(a) => (Some(a), None),
            Err(e) => (None, Some(e)),
        };

        Self {
            rx,
            sensor_status: "Starting sensor...".to_string(),
            sensor_error: None,
            latest_sample: None,
            last_sample_age_sec: 0.0,

            start_time: Instant::now(),
            fake_enabled: true,
            fake_frequency_hz: 0.6,
            fake_amplitude_deg: 30.0,

            bellows,
            bellows_out: BellowsOutput::default(),

            keymap,
            keymap_error,
            pressed: PressedKeys::new(),

            audio,
            audio_error,
            master_gain: 0.8,
            audio_enabled: true,
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        // 0) Pull any sensor messages that arrived since last frame
        self.drain_sensor_messages();

        // 1) Read keyboard input and update pressed notes (and trigger audio)
        self.handle_keyboard(ctx);

        // 2) Update bellows (fake or real depending on toggle)
        self.update_bellows();

        // 3) Apply bellows amplitude to audio every frame
        self.update_audio_from_bellows();

        // 4) Draw the UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Harmonium (Phase 2: Audio)");

            self.ui_sensor_status(ui);

            ui.separator();
            self.ui_audio_status(ui);

            ui.separator();

            ui.columns(2, |cols| {
                cols[0].heading("Controls");
                self.ui_controls(&mut cols[0]);

                cols[1].heading("Live Values");
                self.ui_live_values(&mut cols[1]);
            });

            ui.separator();

            self.ui_keymap_status(ui);
            self.ui_active_notes(ui);
        });

        // Keep repainting so meters update smoothly.
        ctx.request_repaint();
    }

    fn ui_sensor_status(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sensor");

        ui.label(format!("Status: {}", self.sensor_status));

        if let Some(err) = &self.sensor_error {
            ui.colored_label(egui::Color32::RED, format!("Error: {err}"));
        }

        if let Some(s) = &self.latest_sample {
            ui.label(format!("Latest angle: {:6.2} deg   source={}", s.theta_deg, s.source));
            ui.label(format!("Last sample age: {:5.2} sec", self.last_sample_age_sec));
        } else {
            ui.label("No samples yet.");
        }
    }

    fn ui_audio_status(&mut self, ui: &mut egui::Ui) {
        ui.heading("Audio");

        if let Some(err) = &self.audio_error {
            ui.colored_label(egui::Color32::RED, format!("Audio error: {err}"));
        } else if self.audio.is_some() {
            ui.colored_label(egui::Color32::GREEN, "Audio engine ready");
        } else {
            ui.colored_label(egui::Color32::YELLOW, "Audio engine not available");
        }

        ui.checkbox(&mut self.audio_enabled, "Enable audio output");

        // Master gain slider (will affect volume)
        ui.add(egui::Slider::new(&mut self.master_gain, 0.0..=1.5).text("master volume"));

        // If audio exists, apply master gain live
        if let Some(a) = &mut self.audio {
            a.set_master_gain(self.master_gain);
        }

        if ui.button("Stop all notes").clicked() {
            if let Some(a) = &mut self.audio {
                a.stop_all();
            }
        }
    }

    fn drain_sensor_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                SensorMsg::Status(s) => {
                    self.sensor_status = s;
                    self.sensor_error = None;
                }
                SensorMsg::Error(e) => {
                    self.sensor_error = Some(e);
                }
                SensorMsg::Sample(sample) => {
                    self.latest_sample = Some(sample);
                }
            }
        }

        self.last_sample_age_sec = if let Some(s) = &self.latest_sample {
            (Instant::now() - s.t).as_secs_f32()
        } else {
            0.0
        };
    }

    fn update_bellows(&mut self) {
        if self.fake_enabled {
            self.update_bellows_fake_input();
        } else {
            self.update_bellows_real_input();
        }
    }

    fn update_bellows_fake_input(&mut self) {
        let now = Instant::now();
        let t = (now - self.start_time).as_secs_f32();

        let theta = self.fake_amplitude_deg
            * (2.0 * std::f32::consts::PI * self.fake_frequency_hz * t).sin();

        self.bellows_out = self.bellows.update(theta, now);
    }

    fn update_bellows_real_input(&mut self) {
        let Some(s) = &self.latest_sample else {
            return;
        };

        self.bellows_out = self.bellows.update(s.theta_deg, s.t);
    }

    fn update_audio_from_bellows(&mut self) {
        if !self.audio_enabled {
            // If audio disabled, we force bellows to 0 volume.
            if let Some(a) = &mut self.audio {
                a.set_bellows(0.0);
            }
            return;
        }

        if let Some(a) = &mut self.audio {
            a.set_bellows(self.bellows_out.a);
        }
    }

    fn ui_controls(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.fake_enabled, "Use fake angle input (sine wave)");
        ui.label("Turn OFF fake input to use real screen angle from the device.");

        ui.add(
            egui::Slider::new(&mut self.fake_frequency_hz, 0.05..=3.0)
                .text("fake frequency (Hz)"),
        );
        ui.add(
            egui::Slider::new(&mut self.fake_amplitude_deg, 1.0..=80.0)
                .text("fake amplitude (deg)"),
        );

        ui.separator();
        ui.label("Bellows tuning:");

        let p = &mut self.bellows.params;

        ui.add(
            egui::Slider::new(&mut p.deadzone_deg_per_s, 0.0..=40.0).text("deadzone (deg/s)"),
        );
        ui.add(
            egui::Slider::new(&mut p.vmax_deg_per_s, 10.0..=500.0).text("vmax (deg/s)"),
        );
        ui.add(egui::Slider::new(&mut p.gamma, 0.3..=4.0).text("gamma (curve)"));
        ui.add(egui::Slider::new(&mut p.ema_alpha, 0.01..=0.5).text("EMA alpha (smoothing)"));
        ui.add(egui::Slider::new(&mut p.attack_ms, 0.0..=400.0).text("attack (ms)"));
        ui.add(egui::Slider::new(&mut p.release_ms, 0.0..=1200.0).text("release (ms)"));

        ui.separator();

        if ui.button("Reset bellows state").clicked() {
            self.bellows.reset();
            self.bellows_out = BellowsOutput::default();
        }
    }

    fn ui_live_values(&mut self, ui: &mut egui::Ui) {
        let o = self.bellows_out;

        ui.label(format!("theta_deg:        {:8.3}", o.theta_deg));
        ui.label(format!("dt_sec:           {:8.4}", o.dt_sec));
        ui.label(format!("omega_deg_per_s:  {:8.3}", o.omega_deg_per_s));
        ui.label(format!("speed_raw:        {:8.3}", o.speed_raw));
        ui.label(format!("speed_smooth:     {:8.3}", o.speed_smooth));
        ui.label(format!("a_target:         {:8.3}", o.a_target));
        ui.label(format!("a (final):        {:8.3}", o.a));

        ui.separator();

        ui.label("Bellows meter (A):");
        ui.add(
            egui::ProgressBar::new(o.a.clamp(0.0, 1.0))
                .show_percentage()
                .animate(true),
        );
    }

    fn ui_keymap_status(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keymap");

        if let Some(err) = &self.keymap_error {
            ui.colored_label(egui::Color32::RED, format!("Keymap error: {err}"));
        } else if self.keymap.is_some() {
            ui.colored_label(egui::Color32::GREEN, "keymap.json loaded OK");
        } else {
            ui.colored_label(egui::Color32::YELLOW, "No keymap loaded");
        }

        if ui.button("Reload keymap.json").clicked() {
            match KeyMap::load_from_file("keymap.json") {
                Ok(km) => {
                    self.keymap = Some(km);
                    self.keymap_error = None;
                }
                Err(e) => {
                    self.keymap = None;
                    self.keymap_error = Some(e);
                }
            }
        }
    }

    fn ui_active_notes(&mut self, ui: &mut egui::Ui) {
        ui.heading("Active notes");
        let notes = self.pressed.active_notes();

        if notes.is_empty() {
            ui.label("None (press keys like z, x, c, v, ...)");
        } else {
            ui.label(notes.join("  "));
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let keymap = self.keymap.as_ref();

        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers: _,
                    ..
                } = event
                {
                    if *repeat {
                        continue;
                    }

                    if let Some(ch) = egui_key_to_char(*key) {
                        if *pressed {
                            // Key down
                            if let Some(km) = keymap {
                                if let Some(note) = self.pressed.key_down(ch, km) {
                                    // Start audio note if possible
                                    if self.audio_enabled {
                                        if let Some(a) = &mut self.audio {
                                            if let Err(e) = a.note_on(&note) {
                                                self.audio_error = Some(e);
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Key up
                            if let Some(note) = self.pressed.key_up(ch) {
                                if let Some(a) = &mut self.audio {
                                    a.note_off(&note);
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

fn egui_key_to_char(key: egui::Key) -> Option<char> {
    use egui::Key;

    let ch = match key {
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        Key::Z => 'z',

        Key::Num0 => '0',
        Key::Num1 => '1',
        Key::Num2 => '2',
        Key::Num3 => '3',
        Key::Num4 => '4',
        Key::Num5 => '5',
        Key::Num6 => '6',
        Key::Num7 => '7',
        Key::Num8 => '8',
        Key::Num9 => '9',

        Key::Comma => ',',
        Key::Period => '.',
        Key::Slash => '/',
        Key::Semicolon => ';',
        Key::Backslash => '\\',
        Key::OpenBracket => '[',
        Key::CloseBracket => ']',
        Key::Equals => '=',

        _ => return None,
    };

    Some(ch)
}
