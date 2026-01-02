use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

/// Simple audio engine:
/// - Each active note has a Sink (a mixer track).
/// - We loop the sample forever.
/// - We control volume continuously using bellows amplitude.
///
/// Why loop forever?
/// Because your samples are 7–12 seconds, but harmonium notes should sustain
/// as long as the key is held and the bellows (screen motion) provides air.
pub struct AudioEngine {
    // Keep the stream alive. If these are dropped, audio stops.
    _stream: OutputStream,
    handle: OutputStreamHandle,

    // Where your audio files live, e.g. "harmonium-sounds"
    samples_dir: PathBuf,

    // Active note sinks: note name -> Sink
    active: HashMap<String, Sink>,

    // A master volume knob (0..1-ish). We multiply bellows amplitude by this.
    master_gain: f32,

    // Latest bellows amplitude (0..1). Stored so we can recompute sink volumes.
    bellows_a: f32,
}

impl AudioEngine {
    /// Create an audio engine. `samples_dir` is your "harmonium-sounds" folder.
    pub fn new(samples_dir: impl AsRef<Path>) -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("Audio output init failed: {e}"))?;

        Ok(Self {
            _stream: stream,
            handle,
            samples_dir: samples_dir.as_ref().to_path_buf(),
            active: HashMap::new(),
            master_gain: 0.8,
            bellows_a: 0.0,
        })
    }

    /// Set master gain (slider later).
    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain.clamp(0.0, 2.0);
        self.refresh_volumes();
    }

    /// Set current bellows amplitude (0..1). Call this every frame.
    pub fn set_bellows(&mut self, a: f32) {
        self.bellows_a = a.clamp(0.0, 1.0);
        self.refresh_volumes();
    }

    /// Start a note if it isn't already playing.
    ///
    /// We:
    /// - find a sample file in harmonium-sounds
    /// - decode it
    /// - loop it forever
    /// - put it into a Sink
    pub fn note_on(&mut self, note: &str) -> Result<(), String> {
        if self.active.contains_key(note) {
            return Ok(());
        }

        let path = self.find_sample_path(note).ok_or_else(|| {
            format!(
                "No audio file found for note '{note}'. Expected something like '{note}.wav' in {:?}",
                self.samples_dir
            )
        })?;

        let file = File::open(&path).map_err(|e| format!("Failed to open {path:?}: {e}"))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode {path:?}: {e}"))?;

        // Loop the decoded audio forever.
        let source = decoder.repeat_infinite();

        // Each note gets its own Sink (volume control).
        let sink = Sink::try_new(&self.handle).map_err(|e| format!("Failed to create sink: {e}"))?;

        // Start silent. Volume will be set by refresh_volumes().
        sink.set_volume(0.0);

        // Append the audio source to the sink.
        sink.append(source);

        // Keep playing (sink begins immediately once it has a source).
        sink.play();

        self.active.insert(note.to_string(), sink);
        self.refresh_volumes();
        Ok(())
    }

    /// Stop a note immediately (Phase 2: later we'll add a short fade-out).
    pub fn note_off(&mut self, note: &str) {
        if let Some(sink) = self.active.remove(note) {
            sink.stop();
        }
    }

    /// Stop everything (panic button).
    pub fn stop_all(&mut self) {
        for (_note, sink) in self.active.drain() {
            sink.stop();
        }
    }

    /// Recompute the volume of every active note.
    ///
    /// Harmonium idea:
    /// - Keys decide which notes exist.
    /// - Bellows amplitude decides how loud they are.
    fn refresh_volumes(&mut self) {
        let vol = (self.master_gain * self.bellows_a).clamp(0.0, 2.0);

        for (_note, sink) in self.active.iter() {
            sink.set_volume(vol);
        }
    }

    /// Look for a file like:
    /// harmonium-sounds/<note>.wav
    /// harmonium-sounds/<note>.mp3
    /// harmonium-sounds/<note>.ogg
    /// harmonium-sounds/<note>.flac
    fn find_sample_path(&self, note: &str) -> Option<PathBuf> {
        let exts = ["wav", "mp3", "ogg", "flac"];

        for ext in exts {
            let p = self.samples_dir.join(format!("{note}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }

        None
    }
}
