//! Client preferences: types, defaults, validation and atomic persistence.
//!
//! Model: a versioned plain object (same spirit as og/PURE) with profiles.
//! Project rule: the keys the game reads before loading (player_name, avatar,
//! extrapolation…) live per profile here, and the page layer writes them to
//! localStorage whenever the profile changes (F4).

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Simulated geographic location (the game's native `geo_override` key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoOverride {
    pub lat: f64,
    pub lon: f64,
    pub code: String,
}

/// A player's identity and game settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// When enabled, changes made in the game are written back to the profile.
    pub autosave: bool,
    pub player_name: Option<String>,
    pub avatar: Option<String>,
    pub extrapolation: Option<String>,
    /// Private key `idkey.x.y.z` (EC P-256).
    pub player_auth_key: Option<String>,
    pub geo_override: Option<GeoOverride>,
    pub fav_rooms: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            autosave: true,
            player_name: None,
            avatar: None,
            extrapolation: None,
            player_auth_key: None,
            geo_override: None,
            fav_rooms: Vec::new(),
        }
    }
}

/// Root of the preferences. `#[serde(default)]` lets an old file (or a
/// hand-written export) survive missing fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub active_profile: String,
    pub profiles: Vec<Profile>,
    /// F1: engine performance patches (`engine` section, all default ON).
    pub engine: EngineToggles,
}

pub const CURRENT_VERSION: u32 = 1;

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            active_profile: Profile::default().id.clone(),
            profiles: vec![Profile::default()],
            engine: EngineToggles::default(),
        }
    }
}

/// F1 engine patch toggles (keys `snake_case` in `preferences.json`; the runtime
/// uses the dashed patch names). All default ON; a patch also fails open when
/// its anchor is missing even if its toggle is on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineToggles {
    pub canvas_resize_cache: bool,
    pub stadium_layer_cache: bool,
    pub zero_input_tolerance: bool,
    pub hidden_frame_skip: bool,
}

impl Default for EngineToggles {
    fn default() -> Self {
        Self {
            canvas_resize_cache: true,
            stadium_layer_cache: true,
            zero_input_tolerance: true,
            hidden_frame_skip: true,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PrefsError {
    Io(String),
    Invalid(String),
}

impl std::fmt::Display for PrefsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrefsError::Io(e) => write!(f, "io: {e}"),
            PrefsError::Invalid(e) => write!(f, "invalid: {e}"),
        }
    }
}

impl Preferences {
    /// Minimal integrity checks. Returns all problems, not only the first one.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems = Vec::new();

        if self.profiles.is_empty() {
            problems.push("no profiles".to_string());
        }
        if !self.profiles.iter().any(|p| p.id == self.active_profile) {
            problems.push(format!("active_profile '{}' does not exist", self.active_profile));
        }

        let mut seen = std::collections::HashSet::new();
        for p in &self.profiles {
            if p.id.trim().is_empty() {
                problems.push("profile with empty id".to_string());
            } else if !seen.insert(p.id.clone()) {
                problems.push(format!("duplicate profile '{}'", p.id));
            }
            if p.name.trim().is_empty() {
                problems.push(format!("profile '{}' has no name", p.id));
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// Loads from disk. If the file is missing, corrupt or invalid, returns the
    /// default along with the reason (to log, never to crash).
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Some("file does not exist: using defaults".into()));
            }
            Err(e) => return (Self::default(), Some(format!("could not read: {e}"))),
        };

        match serde_json::from_str::<Self>(&raw) {
            Ok(prefs) => match prefs.validate() {
                Ok(()) => (prefs, None),
                Err(err) => (Self::default(), Some(format!("invalid prefs ({err}): using defaults"))),
            },
            Err(e) => (Self::default(), Some(format!("invalid JSON ({e}): using defaults"))),
        }
    }

    /// Atomic save: writes to `.tmp` and renames over the original.
    pub fn save(&self, path: &Path) -> Result<(), PrefsError> {
        self.validate().map_err(PrefsError::Invalid)?;

        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| PrefsError::Io(e.to_string()))?;
        }

        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(self).map_err(|e| PrefsError::Io(e.to_string()))?;

        let mut file = std::fs::File::create(&tmp).map_err(|e| PrefsError::Io(e.to_string()))?;
        file.write_all(body.as_bytes()).map_err(|e| PrefsError::Io(e.to_string()))?;
        file.sync_all().map_err(|e| PrefsError::Io(e.to_string()))?;
        drop(file);

        std::fs::rename(&tmp, path).map_err(|e| PrefsError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("haxfast-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_is_valid() {
        Preferences::default().validate().unwrap();
    }

    #[test]
    fn detects_duplicates_and_missing_active_profile() {
        let mut prefs = Preferences::default();
        prefs.profiles.push(prefs.profiles[0].clone());
        prefs.active_profile = "ghost".into();

        let err = prefs.validate().unwrap_err();
        assert!(err.contains("duplicate"), "{err}");
        assert!(err.contains("ghost"), "{err}");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = test_dir("prefs-rt");
        let path = dir.join("preferences.json");

        let mut prefs = Preferences::default();
        prefs.profiles[0].player_name = Some("HaxFast".into());
        prefs.save(&path).unwrap();

        let (loaded, warn) = Preferences::load(&path);
        assert!(warn.is_none());
        assert_eq!(loaded, prefs);
    }

    #[test]
    fn corrupt_file_falls_back_to_default_with_warning() {
        let dir = test_dir("prefs-corrupt");
        let path = dir.join("preferences.json");
        std::fs::write(&path, "{ not json").unwrap();

        let (loaded, warn) = Preferences::load(&path);
        assert_eq!(loaded, Preferences::default());
        assert!(warn.is_some());
    }

    #[test]
    fn missing_file_falls_back_to_default() {
        let dir = test_dir("prefs-empty");
        let (loaded, warn) = Preferences::load(&dir.join("nope.json"));
        assert_eq!(loaded, Preferences::default());
        assert!(warn.is_some());
    }

    #[test]
    fn engine_toggles_default_on_and_survive_old_files() {
        // Defaults: all four patches ON.
        let toggles = Preferences::default().engine;
        assert!(toggles.canvas_resize_cache);
        assert!(toggles.stadium_layer_cache);
        assert!(toggles.zero_input_tolerance);
        assert!(toggles.hidden_frame_skip);

        // An old file without the `engine` section falls back to all ON.
        let dir = test_dir("prefs-oldnoengine");
        let path = dir.join("preferences.json");
        std::fs::write(
            &path,
            r#"{"version":1,"active_profile":"default","profiles":[
                {"id":"default","name":"Default","autosave":true,"player_name":null,
                 "avatar":null,"extrapolation":null,"player_auth_key":null,
                 "geo_override":null,"fav_rooms":[]}]}"#,
        )
        .unwrap();
        let (loaded, warn) = Preferences::load(&path);
        assert!(warn.is_none());
        assert_eq!(loaded.engine, EngineToggles::default());
    }
}
