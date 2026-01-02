use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// A note name like "c#3" or "f4".
pub type NoteName = String;

/// Stores the mapping from keyboard keys (like 'z', 's', ',') to note names.
#[derive(Debug, Clone)]
pub struct KeyMap {
    map: HashMap<char, NoteName>,
}

impl KeyMap {
    /// Load keymap from a JSON file that looks like:
    /// { "z": "c2", "s": "c#2", ... }
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("Failed to read keymap file: {e}"))?;

        // Parse into a temporary map with String keys, because JSON object keys are strings.
        let raw: HashMap<String, String> =
            serde_json::from_str(&text).map_err(|e| format!("Failed to parse keymap JSON: {e}"))?;

        let mut map: HashMap<char, NoteName> = HashMap::new();

        for (k, v) in raw {
            let mut chars = k.chars();

            let ch = match (chars.next(), chars.next()) {
                (Some(first), None) => first, // exactly 1 char
                _ => {
                    return Err(format!(
                        "Invalid key '{k}' in keymap. Keys must be exactly 1 character."
                    ));
                }
            };

            map.insert(ch, v);
        }

        Ok(Self { map })
    }

    /// Look up a note name from a keyboard character.
    pub fn note_for_char(&self, ch: char) -> Option<&str> {
        self.map.get(&ch).map(|s| s.as_str())
    }
}

/// Tracks which keys are currently pressed and which notes are active.
#[derive(Debug, Default, Clone)]
pub struct PressedKeys {
    /// Which physical keys are down.
    keys_down: HashSet<char>,

    /// For keys that are down, which note they started.
    /// (This matters later for audio "note off".)
    key_to_note: HashMap<char, NoteName>,
}

impl PressedKeys {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call this when a key is pressed.
    /// Returns Some(note) only if this press activated a note (not a repeat).
    pub fn key_down(&mut self, ch: char, keymap: &KeyMap) -> Option<NoteName> {
        // If already down, ignore repeats.
        if self.keys_down.contains(&ch) {
            return None;
        }

        self.keys_down.insert(ch);

        // Translate to a note if mapped.
        let note = keymap.note_for_char(ch)?.to_string();
        self.key_to_note.insert(ch, note.clone());
        Some(note)
    }

    /// Call this when a key is released.
    /// Returns Some(note) only if that key had activated a note.
    pub fn key_up(&mut self, ch: char) -> Option<NoteName> {
        self.keys_down.remove(&ch);
        self.key_to_note.remove(&ch)
    }

    /// List of active notes (useful for UI display).
    pub fn active_notes(&self) -> Vec<NoteName> {
        let mut notes: Vec<NoteName> = self.key_to_note.values().cloned().collect();
        notes.sort();
        notes
    }

    /// Simple query: is this key currently held?
    pub fn _is_down(&self, ch: char) -> bool {
        self.keys_down.contains(&ch)
    }
}
