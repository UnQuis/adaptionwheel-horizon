//! Temporary wheel activation (no Curios). Server-authoritative.

use crate::config::{ActivationMode, MvpConfig, WHEEL_ITEM_ID, WHEEL_ITEM_ID_WOOD};
use crate::state::WheelSourceTag;

/// Abstraction over "is the adaptation wheel currently active for this player?".
/// MVP implementation checks inventory slots; future: accessory slot provider.
pub trait WheelActivationProvider {
    type PlayerCtx;

    fn compute(&self, ctx: &Self::PlayerCtx, cfg: &MvpConfig) -> (bool, WheelSourceTag);
}

#[derive(Debug, Clone, Default)]
pub struct InventorySlotProvider;

impl InventorySlotProvider {
    pub fn is_wheel_item(item_id: &str) -> bool {
        item_id == WHEEL_ITEM_ID || item_id == WHEEL_ITEM_ID_WOOD
    }
}

impl WheelActivationProvider for InventorySlotProvider {
    /// `PlayerCtx` is a placeholder until Horizon player/inventory types are wired.
    /// Expected data: main-hand item id, off-hand item id (and optionally hotbar state).
    type PlayerCtx = PlayerInventorySnapshot;

    fn compute(&self, ctx: &Self::PlayerCtx, cfg: &MvpConfig) -> (bool, WheelSourceTag) {
        if Self::is_wheel_item(&ctx.offhand_item_id) {
            return (true, WheelSourceTag::OffHand);
        }
        if matches!(cfg.activation_mode, ActivationMode::OffhandOrMainHand)
            && Self::is_wheel_item(&ctx.mainhand_item_id)
        {
            return (true, WheelSourceTag::MainHand);
        }
        (false, WheelSourceTag::None)
    }
}

/// Minimal snapshot extracted from Horizon player context in hook handlers.
#[derive(Debug, Clone, Default)]
pub struct PlayerInventorySnapshot {
    pub mainhand_item_id: String,
    pub offhand_item_id: String,
}

/// Helper to apply activation recompute to state and report whether it changed.
pub fn refresh_activation(
    provider: &impl WheelActivationProvider<PlayerCtx = PlayerInventorySnapshot>,
    snapshot: &PlayerInventorySnapshot,
    cfg: &MvpConfig,
    state: &mut crate::state::PlayerAdaptationStateV1,
) -> bool {
    let (active, source) = provider.compute(snapshot, cfg);
    let changed = state.wheel_active != active || state.active_source != source;
    if changed {
        state.wheel_active = active;
        state.active_source = source;
        state.mark_dirty();
    }
    changed
}
