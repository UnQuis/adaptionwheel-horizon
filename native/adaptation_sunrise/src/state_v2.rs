//! Schema-v2 per-player adaptation state model and schema-v1 migration.
//!
//! This module is deliberately pure Rust + serde: no `mod_api`, no globals, no
//! `Host`. It mirrors the codec-persisted fields of upstream
//! `ru.adaptionwheel.data.PlayerAdaption` / `Extras` / `AdaptionTask` so the
//! integration layer can own persistence and runtime wiring.
//!
//! Schema v1 (the frozen native port) stored 7 concepts as
//! `{"schema_version":1,"wheel_active":bool,"active_source":"...","entries":{...}}`.
//! [`migrate_v1_json`] converts that shape into this model and keeps the two
//! activation fields as provenance so the lead can seed the runtime cache.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Current schema version for [`PlayerAdaptionV2`].
pub const SCHEMA_VERSION_V2: u32 = 2;

/// Upstream `HISTORY_LIMIT`: newest-first recent-concept list cap.
pub const HISTORY_LIMIT: usize = 30;

/// Upstream `PlayerAdaption.MAX_LEVEL`.
pub const MAX_LEVEL: u8 = 8;

/// The overflow fields upstream nests as `Extras.CODEC`.
///
/// These are player-owned (not wheel-item-owned) choices and earned progress:
/// `reset_for_unequip` deliberately preserves all of them.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Extras {
    /// Concepts the player toggled off via `toggleEnabled`.
    pub disabled: Vec<String>,
    /// Opaque string item ids standing in for upstream `List<ItemStack>`.
    ///
    /// A pure Rust/serde state model has no Minecraft `ItemStack` type and this
    /// layer has no Horizon dependency, so full stack NBT cannot be represented
    /// here. Keeping ordered string ids preserves slot order and multiplicity so
    /// the integration layer can map entries back to real stacks; the full
    /// stack payloads are a deliberate non-1:1 translation.
    pub cache: Vec<String>,
    /// Earned-but-not-yet-spent progress toward the next level/fist stage.
    /// Upstream uses `Map<String,Integer>`; f32 is used here so fractional
    /// adaptation progress can be stored without a second field.
    pub progress: BTreeMap<String, f32>,
    /// Whether the permanent Instabreak stance is switched on.
    pub instabreak: bool,
    /// Chosen active ability; `None` means "no explicit pick yet".
    pub selected_ability: Option<String>,
}

/// One in-flight analysis task; field names match upstream `AdaptionTask`
/// exactly so S3's `AdaptionTask` can be aliased onto this type by the lead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRecord {
    pub concept: String,
    pub timer: f32,
    pub max_timer: f32,
}

impl TaskRecord {
    pub fn new(concept: impl Into<String>, timer: f32, max_timer: f32) -> Self {
        Self {
            concept: concept.into(),
            timer,
            max_timer,
        }
    }

    /// Upstream `progress() = 1 - timer / maxTimer`, guarded against a zero/unset
    /// `max_timer` (which would yield `NaN` in Rust).
    pub fn progress(&self) -> f32 {
        if self.max_timer <= 0.0 {
            0.0
        } else {
            1.0 - self.timer / self.max_timer
        }
    }
}

/// Full schema-v2 per-player adaptation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerAdaptionV2 {
    pub schema_version: u32,
    /// Level per leveled concept (one-time concepts are normally in `adapted`).
    pub levels: BTreeMap<String, u8>,
    /// One-time adaptations acquired, including ones also present in `levels`.
    pub adapted: BTreeSet<String>,
    pub tasks: Vec<TaskRecord>,
    /// Newest-first, capped at [`HISTORY_LIMIT`].
    pub history: Vec<String>,
    /// Existence/boss adaptations acquired.
    pub existence: BTreeSet<String>,
    pub kill_counts: BTreeMap<String, u32>,
    pub existence_progress: BTreeMap<String, u32>,
    pub healing_timer: i32,
    pub adversity_cooldown: i32,
    pub adversity_timer: i32,
    pub adversity_active: bool,
    pub target_rotation: f32,
    pub wheel_rotation: f32,
    pub was_wearing: bool,
    pub shed_count: u32,
    pub extras: Extras,
    /// Runtime-only persistence-dirty marker; never serialized.
    #[serde(skip)]
    pub dirty: bool,
    /// Seeded from schema-v1 `wheel_active` by [`migrate_v1_json`] so the lead
    /// can seed the runtime activation cache at load time.
    pub legacy_wheel_active: bool,
    /// Seeded from schema-v1 `active_source` by [`migrate_v1_json`].
    pub legacy_active_source: String,
}

impl Default for PlayerAdaptionV2 {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V2,
            levels: BTreeMap::new(),
            adapted: BTreeSet::new(),
            tasks: Vec::new(),
            history: Vec::new(),
            existence: BTreeSet::new(),
            kill_counts: BTreeMap::new(),
            existence_progress: BTreeMap::new(),
            healing_timer: 0,
            adversity_cooldown: 0,
            adversity_timer: 0,
            adversity_active: false,
            target_rotation: 0.0,
            wheel_rotation: 0.0,
            was_wearing: false,
            shed_count: 0,
            extras: Extras::default(),
            dirty: false,
            legacy_wheel_active: false,
            legacy_active_source: String::new(),
        }
    }
}

impl PlayerAdaptionV2 {
    /// Upstream `active(concept) = isEnabled(concept) && adapted.contains(concept)`.
    /// `is_enabled` is supplied by the caller (usually
    /// `|c| !self.extras.disabled.contains(&c.to_string())`).
    pub fn active(&self, concept: &str, is_enabled: impl Fn(&str) -> bool) -> bool {
        is_enabled(concept) && self.adapted.contains(concept)
    }

    /// Upstream `levelOrZero`: returns 0 for a concept the player toggled off,
    /// otherwise its stored level (0 when absent).
    pub fn level_or_zero(&self, concept: &str) -> u8 {
        if self.extras.disabled.iter().any(|c| c == concept) {
            0
        } else {
            self.levels.get(concept).copied().unwrap_or(0)
        }
    }

    /// Upstream `getAdaptCount`: count the **union** of `adapted` and positive
    /// `levels` keys. A few concepts (notably All Adaptation debuff grants) live
    /// in both collections; summing them double-counted and produced the
    /// 483-vs-503 HUD/menu mismatch recorded at `PlayerAdaption.java:175-188`.
    pub fn get_adapt_count(&self) -> usize {
        let mut concepts: BTreeSet<&str> = BTreeSet::new();
        for concept in &self.adapted {
            concepts.insert(concept.as_str());
        }
        for (concept, &level) in &self.levels {
            if level > 0 {
                concepts.insert(concept.as_str());
            }
        }
        concepts.len()
    }

    /// Upstream `addHistory`: dedupe, push newest to the front, cap at
    /// [`HISTORY_LIMIT`] by dropping the oldest entry at the back.
    pub fn push_history(&mut self, concept: &str) {
        self.history.retain(|c| c != concept);
        self.history.insert(0, concept.to_string());
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop();
        }
    }

    /// Upstream `resetForUnequip`: clear every wheel-bound field but keep the
    /// five player-owned `Extras` pieces intact (disabled / cache / progress /
    /// instabreak / selectedAbility). Clearing them destroyed a full 54-slot
    /// cache, every off switch, the fist counters and the stance on unequip.
    ///
    /// `history` is also wheel-bound (upstream `reset()` clears it), so it is
    /// cleared here too. The two `legacy_*` fields are migration provenance used
    /// to seed the runtime cache at load time, not wheel state, so they are
    /// intentionally left untouched.
    pub fn reset_for_unequip(&mut self) {
        self.levels.clear();
        self.adapted.clear();
        self.tasks.clear();
        self.history.clear();
        self.existence.clear();
        self.kill_counts.clear();
        self.existence_progress.clear();
        self.healing_timer = 0;
        self.adversity_cooldown = 0;
        self.adversity_timer = 0;
        self.adversity_active = false;
        self.target_rotation = 0.0;
        self.wheel_rotation = 0.0;
        self.was_wearing = false;
        self.shed_count = 0;
    }

    /// Bring a deserialized or migrated state up to the current schema.
    pub fn normalize(&mut self) {
        self.schema_version = SCHEMA_VERSION_V2;
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop();
        }
    }

    /// Method form of the free [`encode_v2`] helper.
    pub fn encode_v2(&self) -> String {
        encode_v2(self)
    }

    /// Associated-function form of the free [`decode_v2`] helper.
    pub fn decode_v2(json: &str) -> PlayerAdaptionV2 {
        decode_v2(json)
    }
}

/// Convert a schema-v1 concept key into the upstream-style concept id used by
/// schema v2. Unknown keys fold into `Type_OTHER` (the general damage bucket).
pub fn p1_concept_id(legacy: &str) -> &'static str {
    match legacy {
        "env_fall" | "fall" | "Env_FallDamage" => "Env_FallDamage",
        "type_explosion" | "explosion" | "Type_EXPLOSION" => "Type_EXPLOSION",
        "env_fire" | "fire" | "Env_Fire" => "Env_Fire",
        "env_lava" | "lava" | "Env_Lava" => "Env_Lava",
        "env_drowning" | "drowning" | "drown" | "Env_Drowning" => "Env_Drowning",
        "type_contact" | "contact" | "cactus" | "Contact_CACTUS" => "Contact_CACTUS",
        "type_general" | "general" | "generic" | "damage" | "Type_OTHER" => "Type_OTHER",
        _ => "Type_OTHER",
    }
}

/// Per-concept max level in the frozen v1 engine (fall is one-time).
fn legacy_max_level(legacy: &str) -> u8 {
    match legacy {
        "env_fall" | "fall" | "Env_FallDamage" => 1,
        _ => 3,
    }
}

// Local mirror of the engine's v1 JSON shape. Defined privately so this module
// stays standalone (no import of `engine_impl`).
#[derive(Deserialize)]
struct LegacyEntry {
    #[serde(default)]
    level: u8,
    #[serde(default)]
    progress: f32,
    #[serde(default)]
    completed: bool,
}

#[derive(Deserialize)]
struct LegacyV1 {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    wheel_active: bool,
    #[serde(default)]
    active_source: String,
    #[serde(default)]
    entries: BTreeMap<String, LegacyEntry>,
}

/// Parse a schema-v1 JSON state into the v2 model.
///
/// * `completed || level >= v1 concept max` -> `adapted` (upstream-style id).
/// * `level > 0` otherwise -> `levels` with the same id, clamped to [`MAX_LEVEL`].
/// * `wheel_active` / `active_source` are preserved in the two `legacy_*` fields.
///
/// Malformed or unparseable JSON yields [`PlayerAdaptionV2::default()`].
pub fn migrate_v1_json(json: &str) -> PlayerAdaptionV2 {
    let Ok(v1) = serde_json::from_str::<LegacyV1>(json) else {
        return PlayerAdaptionV2::default();
    };

    let mut out = PlayerAdaptionV2::default();
    out.legacy_wheel_active = v1.wheel_active;
    out.legacy_active_source = v1.active_source;

    for (legacy, entry) in v1.entries {
        let id = p1_concept_id(&legacy);
        if entry.completed || entry.level >= legacy_max_level(&legacy) {
            out.adapted.insert(id.to_string());
        } else if entry.level > 0 {
            out.levels
                .insert(id.to_string(), entry.level.min(MAX_LEVEL));
        }
        // v1 `progress` below the next level is intentionally not migrated:
        // schema v1 stored a raw float threshold counter with no upstream key,
        // and the v2 `Extras.progress` keys have different semantics. The datum
        // is dropped rather than silently mis-keyed.
        let _ = entry.progress;
        let _ = v1.schema_version;
    }

    out.normalize();
    out
}

/// Serialize a v2 state to JSON.
pub fn encode_v2(state: &PlayerAdaptionV2) -> String {
    serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())
}

/// Deserialize a v2 state from JSON; malformed input yields the v2 default.
pub fn decode_v2(json: &str) -> PlayerAdaptionV2 {
    match serde_json::from_str::<PlayerAdaptionV2>(json) {
        Ok(mut state) => {
            state.normalize();
            state
        }
        Err(_) => PlayerAdaptionV2::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapt_count_is_union_not_sum() {
        let mut s = PlayerAdaptionV2::default();
        s.adapted.insert("Type_EXPLOSION".to_string());
        s.adapted.insert("Env_Fire".to_string());
        s.levels.insert("Type_EXPLOSION".to_string(), 3);
        s.levels.insert("Env_Fire".to_string(), 2);
        s.levels.insert("Env_Lava".to_string(), 1);
        // 2 ids in adapted + 1 extra positive level = 3, not 5.
        assert_eq!(s.get_adapt_count(), 3);

        // A zero level never contributes a new concept.
        s.levels.insert("Zero_Level".to_string(), 0);
        assert_eq!(s.get_adapt_count(), 3);
    }

    #[test]
    fn history_is_newest_first_deduped_and_capped_at_30() {
        let mut s = PlayerAdaptionV2::default();
        for i in 0..40 {
            s.push_history(&format!("Concept_{i:02}"));
        }
        assert_eq!(s.history.len(), HISTORY_LIMIT);
        assert_eq!(s.history[0], "Concept_39");
        assert_eq!(s.history[HISTORY_LIMIT - 1], "Concept_10");

        // Re-adding an existing concept moves it to the front, no duplicate.
        s.push_history("Concept_20");
        assert_eq!(s.history.len(), HISTORY_LIMIT);
        assert_eq!(s.history[0], "Concept_20");
        assert_eq!(s.history.iter().filter(|c| *c == "Concept_20").count(), 1);
    }

    #[test]
    fn active_and_level_or_zero_respect_disabled() {
        let mut s = PlayerAdaptionV2::default();
        s.adapted.insert("Env_FallDamage".to_string());
        s.levels.insert("Env_Fire".to_string(), 3);

        assert!(s.active("Env_FallDamage", |c| !s
            .extras
            .disabled
            .iter()
            .any(|d| d == c)));
        assert_eq!(s.level_or_zero("Env_Fire"), 3);

        s.extras.disabled.push("Env_FallDamage".to_string());
        s.extras.disabled.push("Env_Fire".to_string());
        assert!(!s.active("Env_FallDamage", |c| !s
            .extras
            .disabled
            .iter()
            .any(|d| d == c)));
        assert_eq!(s.level_or_zero("Env_Fire"), 0);
    }

    #[test]
    fn task_progress_mirrors_upstream() {
        let t = TaskRecord::new("Env_FallDamage", 25.0, 100.0);
        assert!((t.progress() - 0.75).abs() < f32::EPSILON);
        let zero = TaskRecord::new("x", 1.0, 0.0);
        assert_eq!(zero.progress(), 0.0);
    }

    #[test]
    fn reset_for_unequip_preserves_extras_and_clears_rest() {
        let mut s = PlayerAdaptionV2::default();
        s.levels.insert("Env_Fire".to_string(), 2);
        s.adapted.insert("Env_FallDamage".to_string());
        s.tasks.push(TaskRecord::new("Env_Lava", 1.0, 10.0));
        s.history.push("Env_Fire".to_string());
        s.existence.insert("Existence_EnderDragon".to_string());
        s.kill_counts.insert("Drop_NPC_Zombie".to_string(), 7);
        s.existence_progress.insert("minecraft:ender_dragon".to_string(), 100);
        s.healing_timer = 10;
        s.adversity_cooldown = 20;
        s.adversity_timer = 30;
        s.adversity_active = true;
        s.target_rotation = 1.5;
        s.wheel_rotation = 2.5;
        s.was_wearing = true;
        s.shed_count = 4;

        let extras = Extras {
            disabled: vec!["Env_Lava".to_string()],
            cache: vec!["minecraft:diamond".to_string(), "minecraft:iron_ingot".to_string()],
            progress: BTreeMap::from([("fist_mining_0".to_string(), 12.0)]),
            instabreak: true,
            selected_ability: Some("flight".to_string()),
        };
        s.extras = extras.clone();
        s.legacy_wheel_active = true;
        s.legacy_active_source = "inventory".to_string();

        s.reset_for_unequip();

        assert_eq!(s.extras, extras);
        assert!(s.levels.is_empty());
        assert!(s.adapted.is_empty());
        assert!(s.tasks.is_empty());
        assert!(s.history.is_empty());
        assert!(s.existence.is_empty());
        assert!(s.kill_counts.is_empty());
        assert!(s.existence_progress.is_empty());
        assert_eq!(s.healing_timer, 0);
        assert_eq!(s.adversity_cooldown, 0);
        assert_eq!(s.adversity_timer, 0);
        assert!(!s.adversity_active);
        assert_eq!(s.target_rotation, 0.0);
        assert_eq!(s.wheel_rotation, 0.0);
        assert!(!s.was_wearing);
        assert_eq!(s.shed_count, 0);
        // Migration provenance is not wheel state.
        assert!(s.legacy_wheel_active);
        assert_eq!(s.legacy_active_source, "inventory");
    }

    #[test]
    fn migrate_v1_json_maps_fall_and_fire() {
        let json = r#"{
            "schema_version": 1,
            "wheel_active": true,
            "active_source": "inventory",
            "entries": {
                "env_fall": {"level": 1, "progress": 40.0, "completed": true},
                "env_fire": {"level": 2, "progress": 5.0, "completed": false}
            }
        }"#;
        let s = migrate_v1_json(json);
        assert_eq!(s.schema_version, SCHEMA_VERSION_V2);
        assert!(s.adapted.contains("Env_FallDamage"));
        assert!(!s.adapted.contains("Env_Fire"));
        assert_eq!(s.levels.get("Env_Fire"), Some(&2));
        assert!(s.legacy_wheel_active);
        assert_eq!(s.legacy_active_source, "inventory");
        assert_eq!(s.get_adapt_count(), 2);
    }

    #[test]
    fn migrate_v1_json_completed_by_max_level() {
        let json = r#"{
            "wheel_active": false,
            "active_source": "none",
            "entries": {
                "type_explosion": {"level": 3, "progress": 0.0, "completed": false},
                "env_lava": {"level": 99, "progress": 0.0, "completed": false}
            }
        }"#;
        let s = migrate_v1_json(json);
        // `completed:false` but level at (or above) the v1 concept max counts
        // as acquired, matching the frozen engine's `completed` flag semantics.
        assert!(s.adapted.contains("Type_EXPLOSION"));
        assert!(s.adapted.contains("Env_Lava"));
        assert!(s.levels.is_empty());
    }

    #[test]
    fn concept_id_mapping() {
        assert_eq!(p1_concept_id("env_fall"), "Env_FallDamage");
        assert_eq!(p1_concept_id("type_explosion"), "Type_EXPLOSION");
        assert_eq!(p1_concept_id("env_fire"), "Env_Fire");
        assert_eq!(p1_concept_id("env_lava"), "Env_Lava");
        assert_eq!(p1_concept_id("env_drowning"), "Env_Drowning");
        assert_eq!(p1_concept_id("type_contact"), "Contact_CACTUS");
        assert_eq!(p1_concept_id("type_general"), "Type_OTHER");
        assert_eq!(p1_concept_id("something_new"), "Type_OTHER");
    }

    #[test]
    fn encode_decode_round_trip() {
        let mut s = PlayerAdaptionV2::default();
        s.levels.insert("Env_Fire".to_string(), 2);
        s.adapted.insert("Env_FallDamage".to_string());
        s.tasks.push(TaskRecord::new("Env_Lava", 3.0, 10.0));
        s.push_history("Env_Fire");
        s.kill_counts.insert("Drop_NPC_Zombie".to_string(), 9);
        s.extras.selected_ability = Some("flight".to_string());
        s.extras.instabreak = true;
        s.extras.disabled.push("Env_Lava".to_string());
        s.extras.cache.push("minecraft:diamond".to_string());
        s.legacy_wheel_active = true;
        s.legacy_active_source = "offhand".to_string();
        s.dirty = true;

        let json = encode_v2(&s);
        assert!(json.contains("\"schema_version\":2"));
        // dirty is serde-skipped: it must not survive the round trip.
        assert!(!json.contains("\"dirty\""));

        let back = decode_v2(&json);
        assert_eq!(back.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(back.levels, s.levels);
        assert_eq!(back.adapted, s.adapted);
        assert_eq!(back.tasks, s.tasks);
        assert_eq!(back.history, s.history);
        assert_eq!(back.kill_counts, s.kill_counts);
        assert_eq!(back.extras, s.extras);
        assert!(back.legacy_wheel_active);
        assert_eq!(back.legacy_active_source, "offhand");
        assert!(!back.dirty);
    }

    #[test]
    fn malformed_json_returns_default() {
        let s = decode_v2("not json at all {");
        assert_eq!(s.schema_version, SCHEMA_VERSION_V2);
        assert!(s.levels.is_empty());
        assert!(s.adapted.is_empty());
        assert!(!s.legacy_wheel_active);

        let s2 = migrate_v1_json("{broken");
        assert_eq!(s2.schema_version, SCHEMA_VERSION_V2);
        assert!(s2.entries_is_empty_for_test());
    }

    // Small test-only helper so the malformed-migration assertion reads clearly.
    impl PlayerAdaptionV2 {
        fn entries_is_empty_for_test(&self) -> bool {
            self.levels.is_empty() && self.adapted.is_empty() && self.extras == Extras::default()
        }
    }
}
