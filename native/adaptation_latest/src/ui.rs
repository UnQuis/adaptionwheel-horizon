//! Client-side HUD/UI helpers for AdaptionWheel on the latest Horizon mod API.
//!
//! This module is deliberately a set of plain functions and data snapshots: it
//! has no globals, no state-module imports and no `#[mod_api::horizon_mod]`
//! marker (there may be only one per crate). The mod's root `Hooks` impl wires
//! these in:
//!
//! ```rust,ignore
//! // Hooks::register_gui_layers
//! ui::register(event);
//! // Hooks::gui_layer_draw
//! ui::on_gui_layer_draw(event, &snapshot);
//! // Hooks::render_gui_post (optional second HUD path, honors Corners/F1)
//! ui::on_render_gui_post(event, &snapshot);
//! // Hooks::screen_render_post (wheel open over a screen)
//! ui::on_screen_render_post(event, &selection);
//! ```
//!
//! All drawing goes through the pinned SDK's [`DrawContext`] in GUI pixels:
//! `fill`, `fill_gradient`, `text` and `text_width` only. Layout and text are
//! pure functions so they can be unit-tested without a host.

use mod_api::{
    Corner, Corners, DrawContext, GuiLayer, GuiLayerPlacement, GuiLayerDrawEvent,
    RegisterGuiLayersEvent, RenderGuiPostEvent, ScreenRenderPostEvent,
};

/// The GUI layer this module registers. `<mod id>:<path>`, non-vanilla namespace.
pub const HUD_LAYER_ID: &str = "adaptionwheel:hud";

/// Number of concepts the compact HUD card shows (mirrors the 7-wheel layout).
pub const CONCEPT_COUNT: usize = 7;

// --- Plain snapshots: the caller owns state, this module only reads these values. ---

/// One concept row of the wheel: label plus earned level and progress to next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConceptSnapshot<'a> {
    pub label: &'a str,
    pub level: u8,
    pub max_level: u8,
    /// Fraction toward the next level, `0.0..=1.0`; clamped when drawn.
    pub progress: f32,
}

/// Adversity / status line data. `ticks` is a cooldown when inactive, a remaining
/// duration when active; it is only displayed, never interpreted as game state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdversitySnapshot<'a> {
    pub active: bool,
    pub label: &'a str,
    pub ticks: i32,
}

/// Compact HUD card data. The seven concepts are supplied by the caller in
/// display order; no state module or host lookup happens here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudSnapshot<'a> {
    pub wheel_active: bool,
    pub concepts: [ConceptSnapshot<'a>; CONCEPT_COUNT],
    pub adversity: AdversitySnapshot<'a>,
}

/// Data for the wheel selection overlay, drawn over an open screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionSnapshot<'a> {
    pub title: &'a str,
    pub concepts: &'a [ConceptSnapshot<'a>],
    /// Index into `concepts` to highlight; out-of-range means nothing selected.
    pub selected: usize,
}

// --- Layout constants and pure layout/text helpers. ---

pub const HUD_WIDTH: f32 = 176.0;
pub const HUD_PAD: f32 = 6.0;
pub const HUD_HEADER_H: f32 = 16.0;
pub const HUD_ROW_H: f32 = 12.0;
pub const HUD_STATUS_H: f32 = 16.0;

pub const SELECT_WIDTH: f32 = 220.0;
pub const SELECT_PAD: f32 = 8.0;
pub const SELECT_HEADER_H: f32 = 18.0;
pub const SELECT_ROW_H: f32 = 14.0;

const LABEL_CHARS: usize = 12;
const SELECT_LABEL_CHARS: usize = 14;
const BAR_CELLS: usize = 10;

/// Width/height (`[w, h]`) of the compact card for the seven concepts.
pub fn hud_card_size() -> [f32; 2] {
    [
        HUD_WIDTH,
        HUD_PAD * 2.0 + HUD_HEADER_H + CONCEPT_COUNT as f32 * HUD_ROW_H + HUD_STATUS_H,
    ]
}

/// `[x, y, w, h]` of a concept row inside a card rect (`[x, y, w, h]`).
pub fn hud_row_rect(card: [f32; 4], index: usize) -> [f32; 4] {
    let y = card[1] + HUD_PAD + HUD_HEADER_H + index as f32 * HUD_ROW_H;
    [card[0] + HUD_PAD, y, card[2] - 2.0 * HUD_PAD, HUD_ROW_H]
}

/// `[x, y, w, h]` of the status line inside a card rect.
pub fn hud_status_rect(card: [f32; 4]) -> [f32; 4] {
    let y = card[1] + HUD_PAD + HUD_HEADER_H + CONCEPT_COUNT as f32 * HUD_ROW_H;
    [card[0] + HUD_PAD, y, card[2] - 2.0 * HUD_PAD, HUD_STATUS_H]
}

/// Centered `[x, y, w, h]` overlay panel for `rows` concept rows, clamped small
/// enough to leave an 8-pixel margin on either axis.
pub fn selection_rect(window: [f32; 2], rows: usize) -> [f32; 4] {
    let w = (SELECT_WIDTH).min((window[0] - 16.0).max(32.0));
    let wanted_h = SELECT_PAD * 2.0 + SELECT_HEADER_H + rows as f32 * SELECT_ROW_H;
    let h = wanted_h.min((window[1] - 16.0).max(32.0));
    [(window[0] - w) * 0.5, (window[1] - h) * 0.5, w, h]
}

/// `[x, y, w, h]` of one row inside a selection panel rect.
pub fn selection_row_rect(panel: [f32; 4], index: usize) -> [f32; 4] {
    [
        panel[0] + SELECT_PAD,
        panel[1] + SELECT_PAD + SELECT_HEADER_H + index as f32 * SELECT_ROW_H,
        panel[2] - 2.0 * SELECT_PAD,
        SELECT_ROW_H,
    ]
}

/// Progress fraction in `0.0..=1.0`; non-finite input is treated as no progress.
/// A maxed concept always reads as full.
pub fn progress_fraction(c: &ConceptSnapshot<'_>) -> f32 {
    if c.level >= c.max_level {
        1.0
    } else if c.progress.is_finite() {
        c.progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// How many of `cells` bar segments are filled for this concept.
pub fn progress_cells(c: &ConceptSnapshot<'_>, cells: usize) -> usize {
    (progress_fraction(c) * cells as f32).round() as usize
}

/// Compact text for one HUD row: clipped label, level and whole percent.
pub fn concept_row_text(c: &ConceptSnapshot<'_>) -> String {
    format!(
        "{} {}/{} {:>3}%",
        clip_label(c.label, LABEL_CHARS),
        c.level.min(c.max_level),
        c.max_level,
        (progress_fraction(c) * 100.0).round() as u32
    )
}

/// Text for one selection row; the selected row gets a leading `>`.
pub fn selection_row_text(c: &ConceptSnapshot<'_>, selected: bool) -> String {
    format!(
        "{}{} {}/{}",
        if selected { "> " } else { "  " },
        clip_label(c.label, SELECT_LABEL_CHARS),
        c.level.min(c.max_level),
        c.max_level
    )
}

/// Adversity line text: active duration, cooldown, or ready.
pub fn adversity_text(a: &AdversitySnapshot<'_>) -> String {
    let ticks = a.ticks.max(0);
    if a.active {
        format!("{} ACTIVE {}t", a.label, ticks)
    } else if ticks > 0 {
        format!("{} cooldown {}t", a.label, ticks)
    } else {
        format!("{} ready", a.label)
    }
}

fn clip_label(label: &str, max_chars: usize) -> String {
    label.chars().take(max_chars).collect()
}

// --- Drawing. All coordinates are GUI pixels, rects are [x, y, w, h]. ---

const CARD_TOP: u32 = 0xE0_1018_26;
const CARD_BOTTOM: u32 = 0xE0_080C_14;
const BORDER: u32 = 0xFF_4A5A_78;
const TEXT: u32 = 0xFF_FFFFFF;
const DIM: u32 = 0xFF_A8B4_C8;
const ON: u32 = 0xFF_7BE0_8A;
const OFF: u32 = 0xFF_9AA4_B4;
const BAR_BG: u32 = 0x90_2028_38;
const BAR_FG: u32 = 0xFF_59A5_FF;
const SELECT_BG: u32 = 0x80_2E6F_D0;
const ADV_ACTIVE: u32 = 0xFF_FF7B_72;

fn text_right(draw: &mut dyn DrawContext, text: &str, right: f32, y: f32, argb: u32) {
    let width = draw.text_width(text);
    draw.text(text, [right - width, y], argb, false);
}

fn outline(draw: &mut dyn DrawContext, r: [f32; 4], argb: u32) {
    draw.fill([r[0], r[1], r[2], 1.0], argb);
    draw.fill([r[0], r[1] + r[3] - 1.0, r[2], 1.0], argb);
    draw.fill([r[0], r[1], 1.0, r[3]], argb);
    draw.fill([r[0] + r[2] - 1.0, r[1], 1.0, r[3]], argb);
}

fn bar(draw: &mut dyn DrawContext, r: [f32; 4], filled: usize, cells: usize) {
    draw.fill(r, BAR_BG);
    if cells == 0 || filled == 0 {
        return;
    }
    let gap = 1.0;
    let cell_w = (r[2] - gap * (cells - 1) as f32) / cells as f32;
    for i in 0..filled.min(cells) {
        let x = r[0] + i as f32 * (cell_w + gap);
        draw.fill([x, r[1], cell_w, r[3]], BAR_FG);
    }
}

/// Draw the card into an explicit rect (`[x, y, w, h]`).
pub fn draw_hud_at(draw: &mut dyn DrawContext, card: [f32; 4], snap: &HudSnapshot<'_>) {
    draw.fill_gradient(card, CARD_TOP, CARD_BOTTOM);
    outline(draw, card, BORDER);

    let header_y = card[1] + HUD_PAD;
    draw.text("Adaption Wheel", [card[0] + HUD_PAD, header_y], TEXT, true);
    let state = if snap.wheel_active { "ON" } else { "OFF" };
    text_right(
        draw,
        state,
        card[0] + card[2] - HUD_PAD,
        header_y,
        if snap.wheel_active { ON } else { OFF },
    );

    for (i, concept) in snap.concepts.iter().enumerate() {
        let row = hud_row_rect(card, i);
        let line = concept_row_text(concept);
        draw.text(&line, [row[0], row[1]], TEXT, false);
        let bar_rect = [row[0], row[1] + row[3] - 2.0, row[2], 2.0];
        bar(draw, bar_rect, progress_cells(concept, BAR_CELLS), BAR_CELLS);
    }

    let status = hud_status_rect(card);
    let adv = adversity_text(&snap.adversity);
    draw.text(
        &adv,
        [status[0], status[1] + 1.0],
        if snap.adversity.active {
            ADV_ACTIVE
        } else {
            DIM
        },
        false,
    );
}

/// Draw the card in a HUD corner using the frame's stacking context.
pub fn draw_hud(
    draw: &mut dyn DrawContext,
    corners: &mut Corners,
    corner: Corner,
    snap: &HudSnapshot<'_>,
) {
    let [w, h] = hud_card_size();
    let (x, y) = corners.place(corner, w, h);
    draw_hud_at(draw, [x, y, w, h], snap);
}

/// Draw our registered GUI layer only when the hook names our layer id.
pub fn draw_layer(draw: &mut dyn DrawContext, layer: &str, snap: &HudSnapshot<'_>) {
    if layer != HUD_LAYER_ID {
        return;
    }
    let [w, h] = hud_card_size();
    draw_hud_at(draw, [6.0, 6.0, w, h], snap);
}

/// Draw the wheel selection overlay over an open screen, centered on the window.
pub fn draw_selection(draw: &mut dyn DrawContext, snap: &SelectionSnapshot<'_>) {
    let panel = selection_rect(draw.size(), snap.concepts.len());
    draw.fill(panel, CARD_BOTTOM);
    outline(draw, panel, BORDER);
    draw.text(
        snap.title,
        [panel[0] + SELECT_PAD, panel[1] + SELECT_PAD],
        TEXT,
        true,
    );
    for (i, concept) in snap.concepts.iter().enumerate() {
        let row = selection_row_rect(panel, i);
        if i == snap.selected {
            draw.fill(row, SELECT_BG);
        }
        let line = selection_row_text(concept, i == snap.selected);
        draw.text(
            &line,
            [row[0] + 4.0, row[1] + 1.0],
            if i == snap.selected { TEXT } else { DIM },
            false,
        );
        let bar_rect = [row[0] + row[2] - 62.0, row[1] + 2.0, 58.0, 4.0];
        bar(draw, bar_rect, progress_cells(concept, BAR_CELLS), BAR_CELLS);
    }
}

/// Draw a standalone adversity/status line at `(x, y)`.
pub fn draw_adversity(draw: &mut dyn DrawContext, at: [f32; 2], adv: &AdversitySnapshot<'_>) {
    let line = adversity_text(adv);
    let w = draw.text_width(&line) + 6.0;
    draw.fill([at[0] - 3.0, at[1] - 1.0, w, 12.0], CARD_BOTTOM);
    draw.text(
        &line,
        at,
        if adv.active { ADV_ACTIVE } else { DIM },
        false,
    );
}

// --- Hook-shaped wrappers for the root Hooks impl. ---

/// `Hooks::register_gui_layers`: push our layer id once, placed above all others.
pub fn register(event: &mut RegisterGuiLayersEvent<'_>) {
    if event.layers.iter().any(|layer| layer.id == HUD_LAYER_ID) {
        return;
    }
    event.layers.push(GuiLayer {
        id: HUD_LAYER_ID.to_owned(),
        placement: GuiLayerPlacement::AboveAll,
    });
}

/// `Hooks::gui_layer_draw`: ignores ids that are not ours.
pub fn on_gui_layer_draw(event: &mut GuiLayerDrawEvent<'_>, snap: &HudSnapshot<'_>) {
    draw_layer(event.draw, event.layer, snap);
}

/// `Hooks::render_gui_post`: top-left stacking, honors F3 corner state via Corners.
pub fn on_render_gui_post(event: &mut RenderGuiPostEvent<'_>, snap: &HudSnapshot<'_>) {
    draw_hud(event.draw, event.corners, Corner::TopLeft, snap);
}

/// `Hooks::screen_render_post`: overlay over the currently open screen.
pub fn on_screen_render_post(event: &mut ScreenRenderPostEvent<'_>, snap: &SelectionSnapshot<'_>) {
    draw_selection(event.draw, snap);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concept(label: &'static str, level: u8, max: u8, progress: f32) -> ConceptSnapshot<'static> {
        ConceptSnapshot {
            label,
            level,
            max_level: max,
            progress,
        }
    }

    fn snapshot() -> HudSnapshot<'static> {
        HudSnapshot {
            wheel_active: true,
            concepts: [
                concept("Env_Fall", 8, 8, 0.9),
                concept("Type_Explosion", 3, 8, 0.5),
                concept("Env_Fire", 1, 8, 0.25),
                concept("Env_Lava", 0, 8, 0.0),
                concept("Type_General", 2, 8, f32::NAN),
                concept("Env_Drowning", 1, 8, 0.75),
                concept("Type_Contact", 1, 8, 0.99),
            ],
            adversity: AdversitySnapshot {
                active: false,
                label: "Adversity",
                ticks: 7,
            },
        }
    }

    #[test]
    fn card_size_covers_all_seven_rows() {
        let [w, h] = hud_card_size();
        assert_eq!(w, HUD_WIDTH);
        assert_eq!(
            h,
            HUD_PAD * 2.0 + HUD_HEADER_H + 7.0 * HUD_ROW_H + HUD_STATUS_H
        );
    }

    #[test]
    fn rows_and_status_stack_inside_card() {
        let card = [10.0, 20.0, HUD_WIDTH, hud_card_size()[1]];
        let first = hud_row_rect(card, 0);
        let last = hud_row_rect(card, CONCEPT_COUNT - 1);
        assert_eq!(first[1], 20.0 + HUD_PAD + HUD_HEADER_H);
        assert_eq!(last[1], first[1] + 6.0 * HUD_ROW_H);
        assert_eq!(last[1] + last[3], hud_status_rect(card)[1]);
        assert_eq!(hud_status_rect(card)[2], card[2] - 2.0 * HUD_PAD);
    }

    #[test]
    fn progress_clamps_and_treats_maxed_as_full() {
        assert_eq!(progress_fraction(&concept("x", 0, 8, -1.0)), 0.0);
        assert_eq!(progress_fraction(&concept("x", 0, 8, 2.0)), 1.0);
        assert_eq!(progress_fraction(&concept("x", 0, 8, f32::NAN)), 0.0);
        assert_eq!(progress_fraction(&concept("x", 8, 8, 0.0)), 1.0);
        assert_eq!(progress_cells(&concept("x", 0, 8, 0.5), BAR_CELLS), 5);
        assert_eq!(progress_cells(&concept("x", 8, 8, 0.0), BAR_CELLS), BAR_CELLS);
    }

    #[test]
    fn row_text_shows_level_and_whole_percent() {
        assert_eq!(concept_row_text(&concept("Fire", 3, 8, 0.5)), "Fire 3/8  50%");
        assert_eq!(
            concept_row_text(&concept("AReallyLongConceptName", 1, 8, 0.0)),
            "AReallyLongC 1/8   0%"
        );
    }

    #[test]
    fn selection_text_marks_selected_row() {
        let c = concept("Fall", 2, 8, 0.5);
        assert_eq!(selection_row_text(&c, true), "> Fall 2/8");
        assert_eq!(selection_row_text(&c, false), "  Fall 2/8");
    }

    #[test]
    fn selection_rect_centers_and_clamps() {
        let panel = selection_rect([400.0, 300.0], 7);
        assert_eq!(panel[0], (400.0 - panel[2]) * 0.5);
        assert_eq!(panel[1], (300.0 - panel[3]) * 0.5);
        assert!(panel[2] <= 400.0 - 16.0);
        let small = selection_rect([40.0, 40.0], 7);
        assert_eq!(small[2], 32.0);
        assert_eq!(small[3], 32.0);
    }

    #[test]
    fn selection_rows_follow_header() {
        let panel = selection_rect([400.0, 300.0], 7);
        let first = selection_row_rect(panel, 0);
        assert_eq!(first[1], panel[1] + SELECT_PAD + SELECT_HEADER_H);
        assert_eq!(
            selection_row_rect(panel, 2)[1],
            first[1] + 2.0 * SELECT_ROW_H
        );
    }

    #[test]
    fn adversity_text_covers_active_cooldown_ready() {
        assert_eq!(
            adversity_text(&AdversitySnapshot {
                active: true,
                label: "Adversity",
                ticks: 7,
            }),
            "Adversity ACTIVE 7t"
        );
        assert_eq!(
            adversity_text(&AdversitySnapshot {
                active: false,
                label: "Adversity",
                ticks: 7,
            }),
            "Adversity cooldown 7t"
        );
        assert_eq!(
            adversity_text(&AdversitySnapshot {
                active: false,
                label: "Adversity",
                ticks: -1,
            }),
            "Adversity ready"
        );
    }

    #[test]
    fn snapshot_has_seven_concepts() {
        assert_eq!(snapshot().concepts.len(), CONCEPT_COUNT);
    }
}
