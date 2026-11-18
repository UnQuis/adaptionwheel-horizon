//! Pure adaptation engine for the AdaptionWheel v2 native mod.
//!
//! This module is deliberately decoupled from the Horizon mod API: it has no
//! `mod_api` imports, no global `Mutex`/`OnceLock`, and no `Host` dependency.
//! Global per-player state, persistence and the Horizon integration live in
//! `state_impl.rs`. All logic here is a faithful port of the frozen v1
//! adaptation core so that it can be unit-tested in isolation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STATE_SCHEMA_VERSION: u32 = 1;

pub const CONCEPT_FALL: &str = "env_fall";
pub const CONCEPT_EXPLOSION: &str = "type_explosion";
pub const CONCEPT_FIRE: &str = "env_fire";
pub const CONCEPT_LAVA: &str = "env_lava";
pub const CONCEPT_GENERAL: &str = "type_general";
pub const CONCEPT_DROWNING: &str = "env_drowning";
pub const CONCEPT_CONTACT: &str = "type_contact";

pub const ALL_CONCEPTS: [&str; 7] = [
    CONCEPT_FALL,
    CONCEPT_EXPLOSION,
    CONCEPT_FIRE,
    CONCEPT_LAVA,
    CONCEPT_GENERAL,
    CONCEPT_DROWNING,
    CONCEPT_CONTACT,
];

pub const FALL_THRESHOLD: f32 = 40.0;
pub const FALL_MAX_GAIN_PER_EVENT: f32 = 20.0;
pub const EXPLOSION_MAX_LEVEL_MVP: u8 = 3;

pub const FIRE_MAX_LEVEL: u8 = 3;
pub const LAVA_MAX_LEVEL: u8 = 3;
pub const GENERAL_MAX_LEVEL: u8 = 3;
pub const DROWNING_MAX_LEVEL: u8 = 3;
pub const CONTACT_MAX_LEVEL: u8 = 3;

pub fn wheel_item_ids() -> &'static [&'static str] {
    &[
        "adaptionwheel:adaptation_wheel",
        "adaptionwheel:mahoraga_wheel",
        "adaptionwheel:mahoraga_wheel_wood",
        // Runtime test fallback (vanilla item) when content mod item is unavailable.
        "minecraft:compass",
    ]
}

pub fn is_wheel_item_id(item_id: &str) -> bool {
    wheel_item_ids()
        .iter()
        .any(|id| item_id == *id || item_id.ends_with(":adaptation_wheel"))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WheelSource {
    #[default]
    None,
    Inventory,
    MainHand,
    OffHand,
    Armor,
    Hotbar,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdaptationEntry {
    pub level: u8,
    pub progress: f32,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAdaptationStateV1 {
    pub schema_version: u32,
    pub wheel_active: bool,
    pub active_source: WheelSource,
    pub entries: BTreeMap<String, AdaptationEntry>,
    /// Runtime-only: mark cache as needing persistence flush (not serialized).
    #[serde(skip)]
    pub dirty_persist: bool,
}

impl Default for PlayerAdaptationStateV1 {
    fn default() -> Self {
        let mut entries = BTreeMap::new();
        for c in ALL_CONCEPTS {
            entries.insert(c.to_string(), AdaptationEntry::default());
        }
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            wheel_active: false,
            active_source: WheelSource::None,
            entries,
            dirty_persist: false,
        }
    }
}

impl PlayerAdaptationStateV1 {
    pub fn normalize(&mut self) {
        self.schema_version = STATE_SCHEMA_VERSION;
        for c in ALL_CONCEPTS {
            self.entries.entry(c.to_string()).or_default();
        }
    }

    pub fn entry_mut(&mut self, concept: &str) -> &mut AdaptationEntry {
        self.entries.entry(concept.to_string()).or_default()
    }

    pub fn concept_level(&self, concept: &str) -> u8 {
        self.entries.get(concept).map(|e| e.level).unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn concept_completed(&self, concept: &str) -> bool {
        self.entries.get(concept).map(|e| e.completed).unwrap_or(false)
    }

    pub fn summary(&self) -> String {
        let fall = self.entries.get(CONCEPT_FALL).cloned().unwrap_or_default();
        let expl = self.entries.get(CONCEPT_EXPLOSION).cloned().unwrap_or_default();
        format!(
            "wheel={} ({:?}) | fall lvl={} prog={:.1}/{} done={} | explosion lvl={}/{} prog={:.1} done={} | fire lvl={} | lava lvl={} | general lvl={} | drowning lvl={} | contact lvl={}",
            self.wheel_active,
            self.active_source,
            fall.level,
            fall.progress,
            FALL_THRESHOLD,
            fall.completed,
            expl.level,
            EXPLOSION_MAX_LEVEL_MVP,
            expl.progress,
            expl.completed,
            self.concept_level(CONCEPT_FIRE),
            self.concept_level(CONCEPT_LAVA),
            self.concept_level(CONCEPT_GENERAL),
            self.concept_level(CONCEPT_DROWNING),
            self.concept_level(CONCEPT_CONTACT),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationScan {
    pub active: bool,
    pub source: WheelSource,
}

/// Scan a 41-slot inventory snapshot of item ids:
/// 0..9 hotbar, 10..35 main inventory, 36..39 armor (feet, legs, chest, head),
/// 40 offhand. Mirrors the v1 `activation_from_inventory_stacks` logic, but
/// takes plain `Option<String>` ids so the engine stays Horizon-independent.
pub fn activation_from_item_ids(ids: &[Option<String>]) -> ActivationScan {
    for (idx, slot) in ids.iter().enumerate() {
        let Some(item) = slot else { continue };
        if !is_wheel_item_id(item) {
            continue;
        }
        let source = if idx <= 8 {
            WheelSource::Hotbar
        } else if idx <= 35 {
            WheelSource::Inventory
        } else if idx <= 39 {
            WheelSource::Armor
        } else {
            WheelSource::OffHand
        };
        return ActivationScan {
            active: true,
            source,
        };
    }
    ActivationScan {
        active: false,
        source: WheelSource::None,
    }
}

// --- Concepts, thresholds, mitigation ---

pub fn concept_max_level(concept: &str) -> u8 {
    match concept {
        CONCEPT_FALL => 1,
        CONCEPT_EXPLOSION => EXPLOSION_MAX_LEVEL_MVP,
        CONCEPT_FIRE => FIRE_MAX_LEVEL,
        CONCEPT_LAVA => LAVA_MAX_LEVEL,
        CONCEPT_GENERAL => GENERAL_MAX_LEVEL,
        CONCEPT_DROWNING => DROWNING_MAX_LEVEL,
        CONCEPT_CONTACT => CONTACT_MAX_LEVEL,
        _ => 1,
    }
}

pub fn leveled_threshold(concept: &str, level: u8) -> f32 {
    let l = level as f32;
    match concept {
        CONCEPT_EXPLOSION => 30.0 + l * 10.0,
        CONCEPT_FIRE => 20.0 + l * 10.0,
        CONCEPT_LAVA => 24.0 + l * 12.0,
        CONCEPT_GENERAL => 30.0 + l * 15.0,
        CONCEPT_DROWNING => 10.0 + l * 6.0,
        CONCEPT_CONTACT => 12.0 + l * 6.0,
        _ => 20.0 + l * 10.0,
    }
}

pub fn leveled_reduction(concept: &str, level: u8) -> f32 {
    match concept {
        CONCEPT_EXPLOSION => match level { 0 => 0.0, 1 => 0.10, 2 => 0.20, _ => 0.35 },
        CONCEPT_FIRE => match level { 0 => 0.0, 1 => 0.15, 2 => 0.30, _ => 0.45 },
        CONCEPT_LAVA => match level { 0 => 0.0, 1 => 0.20, 2 => 0.35, _ => 0.50 },
        CONCEPT_GENERAL => match level { 0 => 0.0, 1 => 0.10, 2 => 0.20, _ => 0.30 },
        CONCEPT_DROWNING => match level { 0 => 0.0, 1 => 0.25, 2 => 0.50, _ => 0.75 },
        CONCEPT_CONTACT => match level { 0 => 0.0, 1 => 0.20, 2 => 0.35, _ => 0.50 },
        _ => 0.0,
    }
}

pub fn is_explosion_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t.contains("explosion") || t.contains("blast") || t.contains("fireworks") || t.contains("bad_respawn_point")
}

pub fn is_fall_damage_type(damage_type: &str) -> bool {
    damage_type.to_ascii_lowercase().contains("fall")
}

pub fn is_lava_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t == "lava" || t.ends_with(":lava") || t.contains("lava")
}

pub fn is_fire_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    // lava treated separately; "hot_floor" (magma) treated as fire-like
    matches!(t.as_str(), "fire" | "in_fire" | "on_fire" | "hot_floor" | "campfire" | "fireball" | "unattributed_fireball")
        || t.ends_with(":fire")
        || t.ends_with(":in_fire")
        || t.ends_with(":on_fire")
        || t.ends_with(":hot_floor")
        || t.ends_with(":campfire")
        || t.ends_with(":fireball")
        || t.ends_with(":unattributed_fireball")
        || (t.contains("fire") && !t.contains("firework"))
}

pub fn is_drowning_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t == "drown" || t.ends_with(":drown") || t.contains("drown")
}

pub fn is_contact_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    matches!(t.as_str(), "cactus" | "sweet_berry_bush" | "stalagmite" | "contact")
        || t.ends_with(":cactus")
        || t.ends_with(":sweet_berry_bush")
        || t.ends_with(":stalagmite")
        || t.contains("cactus")
        || t.contains("sweet_berry_bush")
        || t.contains("stalagmite")
}

/// Primary concept for a damage event (used for progress/mitigation). Fall is
/// handled by the living_fall hook; this is only a fallback for damage hook delivery.
pub fn classify_damage_concept(damage_type: &str) -> Option<&'static str> {
    if is_explosion_damage_type(damage_type) {
        Some(CONCEPT_EXPLOSION)
    } else if is_fall_damage_type(damage_type) {
        Some(CONCEPT_FALL)
    } else if is_lava_damage_type(damage_type) {
        Some(CONCEPT_LAVA)
    } else if is_fire_damage_type(damage_type) {
        Some(CONCEPT_FIRE)
    } else if is_drowning_damage_type(damage_type) {
        Some(CONCEPT_DROWNING)
    } else if is_contact_damage_type(damage_type) {
        Some(CONCEPT_CONTACT)
    } else {
        Some(CONCEPT_GENERAL)
    }
}

pub fn apply_leveled_progress(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    amount: f32,
    max_level: u8,
    gain_cap: f32,
) -> bool {
    if !state.wheel_active {
        return false;
    }
    let entry = state.entry_mut(concept);
    if entry.completed || entry.level >= max_level {
        entry.completed = true;
        entry.progress = 0.0;
        return false;
    }
    let before = (entry.level, entry.progress, entry.completed);
    entry.progress += amount.max(0.5).min(gain_cap);
    let mut threshold = leveled_threshold(concept, entry.level);
    while entry.progress >= threshold && entry.level < max_level {
        entry.progress -= threshold;
        entry.level += 1;
        if entry.level >= max_level {
            entry.completed = true;
            entry.progress = 0.0;
            break;
        }
        threshold = leveled_threshold(concept, entry.level);
    }
    if entry.level >= max_level {
        entry.completed = true;
        entry.progress = 0.0;
    }
    let changed = before.0 != entry.level
        || (before.1 - entry.progress).abs() > f32::EPSILON
        || before.2 != entry.completed;
    if changed { state.dirty_persist = true; }
    changed
}

pub fn mitigation_factor_for_concept(state: &PlayerAdaptationStateV1, concept: &str) -> f32 {
    if concept == CONCEPT_FALL {
        return fall_mitigation_factor(state);
    }
    let level = state.concept_level(concept);
    let reduction = leveled_reduction(concept, level);
    (1.0 - reduction).clamp(0.0, 1.0)
}

// --- Existing/compat functions (used by tests and hooks) ---

pub fn apply_fall_progress(state: &mut PlayerAdaptationStateV1, distance_or_damage: f32) -> bool {
    if !state.wheel_active {
        return false;
    }
    let entry = state.entry_mut(CONCEPT_FALL);
    if entry.completed || entry.level >= 1 {
        entry.completed = true;
        return false;
    }
    let gain = distance_or_damage.clamp(1.0, FALL_MAX_GAIN_PER_EVENT);
    let before = entry.progress;
    entry.progress = (entry.progress + gain).min(FALL_THRESHOLD);
    if entry.progress >= FALL_THRESHOLD {
        entry.progress = FALL_THRESHOLD;
        entry.level = 1;
        entry.completed = true;
    }
    let changed = (entry.progress - before).abs() > f32::EPSILON || entry.completed;
    if changed { state.dirty_persist = true; }
    changed
}

pub fn fall_mitigation_factor(state: &PlayerAdaptationStateV1) -> f32 {
    if state
        .entries
        .get(CONCEPT_FALL)
        .map(|e| e.completed || e.level >= 1)
        .unwrap_or(false)
    {
        0.0
    } else {
        1.0
    }
}

pub fn apply_explosion_progress(state: &mut PlayerAdaptationStateV1, amount: f32) -> bool {
    apply_leveled_progress(state, CONCEPT_EXPLOSION, amount, EXPLOSION_MAX_LEVEL_MVP, 20.0)
}

/// Pure form of v1 `apply_damage_adaptation_for_player`: the per-player state
/// is passed explicitly instead of looked up in a global map.
pub fn apply_damage_adaptation(
    state: &mut PlayerAdaptationStateV1,
    damage_type: &str,
    amount: &mut f32,
) {
    let Some(concept) = classify_damage_concept(damage_type) else { return; };
    let factor = mitigation_factor_for_concept(state, concept);
    if *amount > 0.0 {
        *amount *= factor;
    }
    match concept {
        CONCEPT_FALL => {
            let _ = apply_fall_progress(state, (*amount).max(1.0));
        }
        CONCEPT_EXPLOSION => {
            let _ = apply_explosion_progress(state, *amount);
        }
        CONCEPT_FIRE => {
            let _ = apply_leveled_progress(state, CONCEPT_FIRE, *amount, FIRE_MAX_LEVEL, 12.0);
        }
        CONCEPT_LAVA => {
            let _ = apply_leveled_progress(state, CONCEPT_LAVA, *amount, LAVA_MAX_LEVEL, 12.0);
        }
        CONCEPT_DROWNING => {
            let _ = apply_leveled_progress(state, CONCEPT_DROWNING, *amount, DROWNING_MAX_LEVEL, 8.0);
        }
        CONCEPT_CONTACT => {
            let _ = apply_leveled_progress(state, CONCEPT_CONTACT, *amount, CONTACT_MAX_LEVEL, 10.0);
        }
        CONCEPT_GENERAL => {
            let _ = apply_leveled_progress(state, CONCEPT_GENERAL, *amount, GENERAL_MAX_LEVEL, 15.0);
        }
        _ => {}
    }
}

// --- Commands/UI helpers ---

pub fn format_status(state: &PlayerAdaptationStateV1) -> String {
    let mut parts = vec![format!(
        "wheel_active={} source={:?} schema={}",
        state.wheel_active, state.active_source, state.schema_version
    )];
    for c in ALL_CONCEPTS {
        let e = state.entries.get(c).cloned().unwrap_or_default();
        let max = concept_max_level(c);
        let extra = if c == CONCEPT_FALL {
            format!(" prog={:.2}/{}", e.progress, FALL_THRESHOLD)
        } else if e.level < max && !e.completed {
            format!(" prog={:.2}/{:.2}", e.progress, leveled_threshold(c, e.level))
        } else {
            String::new()
        };
        parts.push(format!(
            "{}: lvl={}/{}{} done={}",
            c, e.level, max, extra, e.completed
        ));
    }
    parts.join(" | ")
}

pub fn normalize_concept(input: &str) -> Option<&'static str> {
    match input.to_ascii_lowercase().as_str() {
        "env_fall" | "fall" => Some(CONCEPT_FALL),
        "type_explosion" | "explosion" => Some(CONCEPT_EXPLOSION),
        "env_fire" | "fire" => Some(CONCEPT_FIRE),
        "env_lava" | "lava" => Some(CONCEPT_LAVA),
        "type_general" | "general" | "generic" | "damage" => Some(CONCEPT_GENERAL),
        "env_drowning" | "drowning" | "drown" => Some(CONCEPT_DROWNING),
        "type_contact" | "contact" | "cactus" => Some(CONCEPT_CONTACT),
        _ => None,
    }
}

pub fn clamp_fall_progress(v: f32) -> f32 { v.clamp(0.0, FALL_THRESHOLD) }

pub fn apply_set_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    field: &str,
    value: f32,
) -> Result<String, String> {
    let field = field.to_ascii_lowercase();
    let max_level = concept_max_level(concept);
    let res = match concept {
        CONCEPT_FALL => {
            let e = state.entry_mut(CONCEPT_FALL);
            match field.as_str() {
                "level" => {
                    let lvl = value.max(0.0) as u8;
                    if lvl >= 1 {
                        e.level = 1;
                        e.completed = true;
                        e.progress = FALL_THRESHOLD;
                    } else {
                        e.level = 0;
                        e.completed = false;
                        e.progress = e.progress.min(FALL_THRESHOLD - f32::EPSILON.max(0.0));
                    }
                    Ok(format!("env_fall level set to {}", e.level))
                }
                "progress" => {
                    e.progress = clamp_fall_progress(value);
                    e.completed = e.progress >= FALL_THRESHOLD || e.level >= 1;
                    if e.completed {
                        e.level = 1;
                        e.progress = FALL_THRESHOLD;
                    } else {
                        e.level = 0;
                    }
                    Ok(format!("env_fall progress set to {:.2}", e.progress))
                }
                _ => Err("field must be 'level' or 'progress'".to_string()),
            }
        }
        CONCEPT_EXPLOSION | CONCEPT_FIRE | CONCEPT_LAVA | CONCEPT_GENERAL | CONCEPT_DROWNING | CONCEPT_CONTACT => {
            let e = state.entry_mut(concept);
            match field.as_str() {
                "level" => {
                    let lvl = (value.max(0.0) as u8).min(max_level);
                    e.level = lvl;
                    e.completed = lvl >= max_level;
                    if e.completed {
                        e.progress = 0.0;
                    }
                    Ok(format!("{} level set to {}", concept, e.level))
                }
                "progress" => {
                    if e.level >= max_level {
                        e.completed = true;
                        e.progress = 0.0;
                        Ok(format!("{} already completed (level max)", concept))
                    } else {
                        let threshold = leveled_threshold(concept, e.level);
                        e.progress = value.clamp(0.0, threshold.max(0.0));
                        e.completed = false;
                        Ok(format!("{} progress set to {:.2}", concept, e.progress))
                    }
                }
                _ => Err("field must be 'level' or 'progress'".to_string()),
            }
        }
        _ => Err("unsupported concept".to_string()),
    };
    if res.is_ok() {
        state.dirty_persist = true;
    }
    res
}

pub fn apply_grant_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    level: u8,
) -> Result<String, String> {
    let max_level = concept_max_level(concept);
    let res = match concept {
        CONCEPT_FALL => {
            let e = state.entry_mut(CONCEPT_FALL);
            if level >= 1 {
                e.level = 1;
                e.completed = true;
                e.progress = FALL_THRESHOLD;
                Ok("granted env_fall level 1 (completed)".to_string())
            } else {
                e.level = 0;
                e.completed = false;
                e.progress = 0.0;
                Ok("set env_fall to level 0".to_string())
            }
        }
        CONCEPT_EXPLOSION | CONCEPT_FIRE | CONCEPT_LAVA | CONCEPT_GENERAL | CONCEPT_DROWNING | CONCEPT_CONTACT => {
            let lvl = level.min(max_level);
            let e = state.entry_mut(concept);
            e.level = lvl;
            e.completed = lvl >= max_level;
            e.progress = 0.0;
            Ok(format!("granted {} level {}", concept, lvl))
        }
        _ => Err("unsupported concept".to_string()),
    };
    if res.is_ok() {
        state.dirty_persist = true;
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_defaults_and_summary() {
        let s = PlayerAdaptationStateV1::default();
        assert!(s.entries.contains_key(CONCEPT_FALL));
        assert!(s.entries.contains_key(CONCEPT_EXPLOSION));
        assert!(s.entries.contains_key(CONCEPT_FIRE));
        assert!(s.entries.contains_key(CONCEPT_LAVA));
        assert!(s.entries.contains_key(CONCEPT_GENERAL));
        assert!(s.entries.contains_key(CONCEPT_DROWNING));
        assert!(s.entries.contains_key(CONCEPT_CONTACT));
        assert!(s.summary().contains("wheel=false"));
    }

    #[test]
    fn activation_detects_wheel_anywhere_in_inventory() {
        let mut slots: Vec<Option<String>> = vec![None; 41];
        slots[12] = Some("adaptionwheel:adaptation_wheel".into());
        let scan = activation_from_item_ids(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::Inventory);

        slots[12] = None;
        slots[0] = Some("adaptionwheel:adaptation_wheel".into());
        let scan = activation_from_item_ids(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::Hotbar);

        slots[0] = None;
        slots[40] = Some("adaptionwheel:adaptation_wheel".into());
        let scan = activation_from_item_ids(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::OffHand);

        slots[40] = None;
        let scan = activation_from_item_ids(&slots);
        assert!(!scan.active);
        assert_eq!(scan.source, WheelSource::None);
    }

    #[test]
    fn fall_progress_and_mitigation() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        assert!(apply_fall_progress(&mut s, 12.0));
        assert!(apply_fall_progress(&mut s, 15.0));
        assert!(apply_fall_progress(&mut s, 20.0));
        assert!(s.entries[CONCEPT_FALL].completed);
        assert_eq!(fall_mitigation_factor(&s), 0.0);
    }

    #[test]
    fn fall_requires_activation() {
        let mut s = PlayerAdaptationStateV1::default();
        assert!(!apply_fall_progress(&mut s, 30.0));
        assert_eq!(s.entries[CONCEPT_FALL].progress, 0.0);
    }

    #[test]
    fn explosion_progress_levels_up() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        let mut changed = false;
        for _ in 0..15 {
            changed |= apply_explosion_progress(&mut s, 20.0);
        }
        assert!(changed);
        assert!(s.entries[CONCEPT_EXPLOSION].level >= 1);
    }

    #[test]
    fn explosion_damage_classifier() {
        assert!(is_explosion_damage_type("minecraft:explosion"));
        assert!(is_explosion_damage_type("minecraft:player_explosion"));
        assert!(!is_explosion_damage_type("minecraft:fall"));
        assert!(is_fall_damage_type("minecraft:fall"));
    }

    #[test]
    fn damage_type_classification_for_new_concepts() {
        assert_eq!(classify_damage_concept("minecraft:in_fire"), Some(CONCEPT_FIRE));
        assert_eq!(classify_damage_concept("minecraft:lava"), Some(CONCEPT_LAVA));
        assert_eq!(classify_damage_concept("minecraft:drown"), Some(CONCEPT_DROWNING));
        assert_eq!(classify_damage_concept("minecraft:cactus"), Some(CONCEPT_CONTACT));
        assert_eq!(classify_damage_concept("minecraft:mob_attack"), Some(CONCEPT_GENERAL));
        assert_eq!(classify_damage_concept("minecraft:explosion"), Some(CONCEPT_EXPLOSION));
    }

    #[test]
    fn leveled_concepts_progress_with_activation() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        let mut changed = false;
        for _ in 0..20 {
            changed |= apply_leveled_progress(&mut s, CONCEPT_FIRE, 10.0, FIRE_MAX_LEVEL, 12.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_LAVA, 10.0, LAVA_MAX_LEVEL, 12.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_GENERAL, 10.0, GENERAL_MAX_LEVEL, 15.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_CONTACT, 8.0, CONTACT_MAX_LEVEL, 10.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_DROWNING, 6.0, DROWNING_MAX_LEVEL, 8.0);
        }
        assert!(changed);
        assert!(s.concept_level(CONCEPT_FIRE) >= 1);
        assert!(s.concept_level(CONCEPT_LAVA) >= 1);
        assert!(s.concept_level(CONCEPT_GENERAL) >= 1);
        assert!(s.concept_level(CONCEPT_CONTACT) >= 1);
        assert!(s.concept_level(CONCEPT_DROWNING) >= 1);
    }

    #[test]
    fn commands_set_and_grant_new_concepts() {
        let mut s = PlayerAdaptationStateV1::default();
        assert!(apply_set_command(&mut s, CONCEPT_FIRE, "progress", 15.0).is_ok());
        assert!(s.entries[CONCEPT_FIRE].progress > 0.0);
        assert!(apply_grant_command(&mut s, CONCEPT_LAVA, 2).is_ok());
        assert_eq!(s.concept_level(CONCEPT_LAVA), 2);
        assert!(apply_set_command(&mut s, CONCEPT_CONTACT, "level", 3.0).is_ok());
        assert!(s.concept_completed(CONCEPT_CONTACT));
        assert!(normalize_concept("drown").is_some());
        assert!(normalize_concept("general").is_some());
    }

    #[test]
    fn damage_adaptation_applies_mitigation_and_progress() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        let e = s.entry_mut(CONCEPT_FIRE);
        e.level = 2;
        let mut amount = 10.0f32;
        apply_damage_adaptation(&mut s, "minecraft:in_fire", &mut amount);
        assert!(amount < 10.0); // mitigation from level 2
        assert!(s.entries[CONCEPT_FIRE].progress >= 0.0);
    }
}
