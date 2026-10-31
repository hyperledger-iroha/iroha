//! Live chain charts and celebration motion: the chain pulse and the sakura shower.

use super::{Desktop, ledger::BlockRow, theme::Palette, widgets};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui,
    Vec2,
};
use std::collections::{HashMap, VecDeque};

/// Seconds a newly streamed block's bar takes to grow to its height.
const GROW_SECONDS: f64 = 0.45;
/// Seconds the newest bar keeps its arrival glow.
const GLOW_SECONDS: f64 = 1.4;
/// Seconds a sakura shower lasts, including each petal's staggered start.
const SHOWER_SECONDS: f64 = 3.4;
/// Petals in one shower.
const PETALS: usize = 46;

/// Animation state that outlives a frame.
#[derive(Default)]
pub(crate) struct Motion {
    /// UI time at which each block height was first drawn.
    first_seen: HashMap<u64, f64>,
    shower: Option<Shower>,
}

#[derive(Clone, Copy)]
enum Shower {
    /// Requested by an event; starts on the next painted frame.
    Pending,
    Running {
        started: f64,
        seed: u64,
    },
}

impl Motion {
    /// Forget block arrivals, e.g. when the observed network changes.
    pub(crate) fn clear_blocks(&mut self) {
        self.first_seen.clear();
    }

    /// Shower sakura petals over the window, e.g. after a deployment applies.
    pub(crate) fn celebrate(&mut self) {
        self.shower = Some(Shower::Pending);
    }

    fn arrival(&mut self, height: u64, now: f64) -> f64 {
        let seen = self.first_seen.entry(height).or_insert(now);
        // A clock that restarts, as with a new UI context, shows known blocks settled.
        if *seen > now {
            *seen = now - GLOW_SECONDS;
        }
        *seen
    }
}

/// Rolling chain figures from the newest streamed blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pulse {
    /// Non-genesis blocks in the window.
    pub(crate) blocks: usize,
    /// Transactions in those blocks, rejected ones included.
    pub(crate) transactions: usize,
    /// Rejected transactions in those blocks.
    pub(crate) rejected: usize,
    /// Mean milliseconds between consecutive blocks, with two or more blocks.
    pub(crate) mean_interval_ms: Option<u64>,
    /// Transactions per second across the window's time span.
    pub(crate) tx_per_second: Option<f64>,
}

/// Summarize the newest `window` non-genesis blocks of a newest-first feed.
pub(crate) fn pulse(blocks: &VecDeque<BlockRow>, window: usize) -> Pulse {
    let recent = blocks
        .iter()
        .filter(|row| !row.genesis)
        .take(window)
        .collect::<Vec<_>>();
    let transactions = recent.iter().map(|row| row.transactions).sum();
    let rejected = recent.iter().map(|row| row.rejected).sum();
    let span_ms = match (recent.first(), recent.last()) {
        (Some(newest), Some(oldest)) if recent.len() >= 2 => {
            newest.created_ms.saturating_sub(oldest.created_ms)
        }
        _ => 0,
    };
    let intervals = u64::try_from(recent.len().saturating_sub(1)).unwrap_or(u64::MAX);
    let mean_interval_ms = (intervals > 0 && span_ms > 0).then(|| span_ms / intervals);
    #[allow(clippy::cast_precision_loss)]
    let tx_per_second = (span_ms > 0).then(|| transactions as f64 * 1_000.0 / span_ms as f64);
    Pulse {
        blocks: recent.len(),
        transactions,
        rejected,
        mean_interval_ms,
        tx_per_second,
    }
}

/// Ease-out cubic: fast start, gentle landing.
fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn stat_tile(ui: &mut Ui, palette: &Palette, label: &str, value: &str, color: Color32) {
    egui::Frame::new()
        .fill(palette.inset)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(118.0);
            ui.label(widgets::eyebrow(palette, label));
            ui.label(RichText::new(value).size(20.0).strong().color(color));
        });
}

impl Desktop {
    /// Key chain figures over an animated bar chart of the newest `window` blocks.
    pub(super) fn chain_pulse(&mut self, ui: &mut Ui, window: usize) {
        let palette = widgets::palette(ui);
        let now = ui.input(|input| input.time);
        let figures = pulse(&self.blocks_feed, window);
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::card_title(ui, widgets::icon::BLOCKS, "Chain pulse");
                if self.blocks.is_some() {
                    widgets::pill(ui, "LIVE", palette.matcha, palette.matcha_soft);
                }
            });
            ui.horizontal_wrapped(|ui| {
                // The stream starts at the next block, so the height is unknown until one arrives.
                let height = self.latest_height.map_or_else(
                    || "—".to_owned(),
                    |height| {
                        // The counter rolls up to each new height instead of jumping.
                        #[allow(clippy::cast_precision_loss)]
                        let rolled = ui.ctx().animate_value_with_time(
                            egui::Id::unique("mochi-chain-height"),
                            height as f32,
                            0.6,
                        );
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let shown = rolled.round().max(0.0) as u64;
                        format!("#{shown}")
                    },
                );
                stat_tile(ui, palette, "Height", &height, palette.sky);
                let block_time = figures.mean_interval_ms.map_or_else(
                    || "—".to_owned(),
                    |ms| format!("{:.1} s", ms as f64 / 1_000.0),
                );
                stat_tile(ui, palette, "Between blocks", &block_time, palette.text);
                let rate = figures
                    .tx_per_second
                    .map_or_else(|| "—".to_owned(), |rate| format!("{rate:.2}"));
                stat_tile(ui, palette, "Tx per second", &rate, palette.matcha);
                let rejected = if figures.transactions == 0 {
                    "—".to_owned()
                } else {
                    format!("{} of {}", figures.rejected, figures.transactions)
                };
                let rejected_color = if figures.rejected > 0 {
                    palette.aka
                } else {
                    palette.text
                };
                stat_tile(ui, palette, "Rejected", &rejected, rejected_color);
            });
            ui.add_space(6.0);
            self.block_bars(ui, palette, window, now);
            widgets::hint(
                ui,
                if self.blocks_feed.is_empty() {
                    "Bars appear as blocks commit. Idle chains create no blocks."
                } else {
                    "Each bar is a committed block: transactions in blue, rejected ones in red."
                },
            );
        });
    }

    fn block_bars(&mut self, ui: &mut Ui, palette: &Palette, window: usize, now: f64) {
        let width = ui.available_width().max(120.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 92.0), Sense::hover());
        let painter = ui.painter_at(rect);
        let chart = Rect::from_min_max(
            rect.min + Vec2::new(0.0, 6.0),
            rect.max - Vec2::new(0.0, 14.0),
        );
        painter.line_segment(
            [chart.left_bottom(), chart.right_bottom()],
            Stroke::new(1.0, palette.border),
        );
        // Oldest on the left; the newest block enters at the right edge.
        let rows = self
            .blocks_feed
            .iter()
            .filter(|row| !row.genesis)
            .take(window)
            .cloned()
            .collect::<Vec<_>>();
        let slots = window.max(1);
        #[allow(clippy::cast_precision_loss)]
        let slot = chart.width() / slots as f32;
        let gap = (slot * 0.22).clamp(1.0, 5.0);
        let tallest = rows
            .iter()
            .map(|row| row.transactions)
            .max()
            .unwrap_or(0)
            .max(1);
        let mut animating = false;
        let mut hovered = None;
        for position in 0..slots {
            // Column `position` from the left shows the block `slots - 1 - position` back.
            let back = slots - 1 - position;
            #[allow(clippy::cast_precision_loss)]
            let left = chart.left() + position as f32 * slot + gap / 2.0;
            let column = Rect::from_min_max(
                Pos2::new(left, chart.top()),
                Pos2::new(left + slot - gap, chart.bottom()),
            );
            let Some(row) = rows.get(back) else {
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(column.left(), chart.bottom() - 3.0),
                        column.right_bottom(),
                    ),
                    CornerRadius::same(1),
                    palette.inset,
                );
                continue;
            };
            let age = now - self.motion.arrival(row.height, now);
            let grow = ease_out(age / GROW_SECONDS);
            animating |= age < GLOW_SECONDS;
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let full = (row.transactions.max(1) as f32 / tallest as f32) * chart.height();
            #[allow(clippy::cast_possible_truncation)]
            let height = (full * grow as f32).max(3.0);
            let bar = Rect::from_min_max(
                Pos2::new(column.left(), chart.bottom() - height),
                column.right_bottom(),
            );
            let is_hovered = response.hover_pos().is_some_and(|pointer| {
                pointer.x >= column.left() && pointer.x < column.right() + gap
            });
            let base = if is_hovered {
                palette.sakura
            } else {
                palette.sky
            };
            painter.rect_filled(bar, CornerRadius::same(3), base);
            if row.rejected > 0 && row.transactions > 0 {
                #[allow(clippy::cast_precision_loss)]
                let share = row.rejected.min(row.transactions) as f32 / row.transactions as f32;
                let cap = Rect::from_min_max(
                    bar.min,
                    Pos2::new(bar.right(), bar.top() + bar.height() * share),
                );
                painter.rect_filled(cap, CornerRadius::same(3), palette.aka);
            }
            if back == 0 && age < GLOW_SECONDS {
                // The newest block arrives with a soft sakura halo that fades out.
                #[allow(clippy::cast_possible_truncation)]
                let fade = (1.0 - age / GLOW_SECONDS).clamp(0.0, 1.0) as f32;
                painter.circle_filled(
                    Pos2::new(bar.center().x, bar.top()),
                    (slot * 0.9).clamp(4.0, 14.0) * (1.0 + 0.4 * (1.0 - fade)),
                    palette.sakura.gamma_multiply(0.35 * fade),
                );
            }
            if is_hovered {
                hovered = Some(row.clone());
            }
        }
        painter.text(
            rect.left_bottom(),
            Align2::LEFT_BOTTOM,
            format!("last {} blocks", rows.len()),
            FontId::proportional(10.5),
            palette.faint,
        );
        painter.text(
            rect.right_bottom(),
            Align2::RIGHT_BOTTOM,
            format!("max {tallest} tx"),
            FontId::proportional(10.5),
            palette.faint,
        );
        if let Some(oldest) = rows.last() {
            self.motion
                .first_seen
                .retain(|height, _| *height >= oldest.height);
        }
        if animating {
            ui.ctx().request_repaint();
        }
        if let Some(row) = hovered {
            response.on_hover_text(format!(
                "Block #{}\n{} transaction{}{}\n{} signatures · {} bytes",
                row.height,
                row.transactions,
                if row.transactions == 1 { "" } else { "s" },
                if row.rejected > 0 {
                    format!(" · {} rejected", row.rejected)
                } else {
                    String::new()
                },
                row.signatures,
                row.bytes
            ));
        }
    }
}

/// A uniform value in `[0, 1)` from a seed and a stream index (SplitMix64).
fn unit(seed: u64, index: u64) -> f64 {
    let mut z = seed
        .wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    #[allow(clippy::cast_precision_loss)]
    let value = (z >> 11) as f64 / (1_u64 << 53) as f64;
    value
}

/// Where one petal is `elapsed` seconds into a shower over `area`, or `None` off-screen.
fn petal(area: Rect, seed: u64, index: usize, elapsed: f64) -> Option<(Pos2, f32, f32, f32)> {
    let draw = |k: u64| unit(seed, (index as u64) * 16 + k);
    let delay = draw(0) * 0.9;
    let fall = 1.8 + draw(1) * 1.1;
    let t = elapsed - delay;
    if t < 0.0 {
        return None;
    }
    let progress = t / fall;
    if progress >= 1.0 {
        return None;
    }
    let sway = 12.0 + draw(2) * 28.0;
    let wobble = (1.0 + draw(3) * 1.4) * std::f64::consts::TAU;
    let phase = draw(4) * std::f64::consts::TAU;
    let x = f64::from(area.left())
        + draw(5) * f64::from(area.width())
        + sway * (t * wobble / 2.0 + phase).sin();
    let y = f64::from(area.top()) - 18.0 + progress * (f64::from(area.height()) + 36.0);
    let spin = if draw(6) < 0.5 { -1.0 } else { 1.0 };
    let angle = draw(7) * std::f64::consts::TAU + spin * t * (1.2 + draw(8) * 2.2);
    let size = 5.0 + draw(9) * 5.5;
    let alpha = (progress / 0.1).min(1.0) * ((1.0 - progress) / 0.18).min(1.0);
    #[allow(clippy::cast_possible_truncation)]
    Some((
        Pos2::new(x as f32, y as f32),
        angle as f32,
        size as f32,
        alpha.clamp(0.0, 1.0) as f32,
    ))
}

/// Paint any running sakura shower above every view; it never takes input.
pub(crate) fn paint_shower(context: &egui::Context, motion: &mut Motion) {
    let now = context.input(|input| input.time);
    let (started, seed) = match motion.shower {
        None => return,
        Some(Shower::Pending) => {
            let seed = now.to_bits() ^ 0x5A4B_A0E5_A4B5_A0E5;
            motion.shower = Some(Shower::Running { started: now, seed });
            (now, seed)
        }
        Some(Shower::Running { started, seed }) => (started, seed),
    };
    let elapsed = now - started;
    if elapsed > SHOWER_SECONDS {
        motion.shower = None;
        return;
    }
    let area = context.content_rect();
    let painter = context.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("mochi-sakura-shower"),
    ));
    for index in 0..PETALS {
        let Some((center, angle, size, alpha)) = petal(area, seed, index, elapsed) else {
            continue;
        };
        let (sin, cos) = angle.sin_cos();
        let outline = (0..14)
            .map(|step| {
                #[allow(clippy::cast_precision_loss)]
                let theta = step as f32 / 14.0 * std::f32::consts::TAU;
                // A petal is a slightly egg-shaped ellipse, wider toward its tip.
                let along = size * theta.cos() * (1.0 + 0.18 * theta.cos());
                let across = size * 0.62 * theta.sin();
                center + Vec2::new(along * cos - across * sin, along * sin + across * cos)
            })
            .collect::<Vec<_>>();
        let tint = if index % 3 == 0 {
            Color32::from_rgb(252, 214, 224)
        } else {
            Color32::from_rgb(246, 170, 194)
        };
        painter.add(Shape::convex_polygon(
            outline,
            tint.gamma_multiply(alpha),
            Stroke::new(
                0.8,
                Color32::from_rgb(222, 120, 152).gamma_multiply(alpha * 0.6),
            ),
        ));
    }
    context.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(height: u64, transactions: usize, rejected: usize, created_ms: u64) -> BlockRow {
        BlockRow {
            height,
            hash: format!("{height:064x}"),
            transactions,
            rejected,
            signatures: 3,
            created_ms,
            genesis: height == 1,
            bytes: 512,
            body: None,
        }
    }

    #[test]
    fn pulse_skips_genesis_and_measures_the_newest_window() {
        let feed = VecDeque::from([
            row(5, 4, 1, 10_000),
            row(4, 2, 0, 8_000),
            row(3, 2, 0, 6_000),
            row(2, 1, 0, 4_000),
            row(1, 0, 0, 0),
        ]);
        let figures = pulse(&feed, 3);
        assert_eq!(figures.blocks, 3);
        assert_eq!(figures.transactions, 8);
        assert_eq!(figures.rejected, 1);
        assert_eq!(figures.mean_interval_ms, Some(2_000));
        assert_eq!(figures.tx_per_second, Some(2.0));
        // Genesis alone, or one block, yields no rates.
        let lone = pulse(&VecDeque::from([row(2, 3, 0, 4_000), row(1, 0, 0, 0)]), 10);
        assert_eq!(
            (lone.blocks, lone.mean_interval_ms, lone.tx_per_second),
            (1, None, None)
        );
        assert_eq!(pulse(&VecDeque::new(), 10).blocks, 0);
    }

    #[test]
    fn bars_ease_out_and_settle() {
        assert!(ease_out(-1.0).abs() < f64::EPSILON);
        assert!((ease_out(1.0) - 1.0).abs() < f64::EPSILON);
        assert!((ease_out(2.0) - 1.0).abs() < f64::EPSILON);
        assert!(ease_out(0.5) > 0.5, "ease-out runs ahead of linear time");
    }

    #[test]
    fn known_blocks_stay_settled_when_the_clock_restarts() {
        let mut motion = Motion::default();
        assert_eq!(motion.arrival(7, 110.0), 110.0);
        assert_eq!(
            motion.arrival(7, 111.0),
            110.0,
            "the first sighting is kept"
        );
        let restarted = motion.arrival(7, 100.0);
        assert!(
            100.0 - restarted >= GLOW_SECONDS,
            "settled, not regrowing or glowing"
        );
    }

    #[test]
    fn petals_fall_through_the_area_and_finish() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut seen = 0;
        for index in 0..PETALS {
            if let Some((center, _, size, alpha)) = petal(area, 7, index, 1.4) {
                seen += 1;
                assert!((-60.0..=860.0).contains(&center.x), "{center:?}");
                assert!((-20.0..=640.0).contains(&center.y), "{center:?}");
                assert!((5.0..=10.5).contains(&size));
                assert!((0.0..=1.0).contains(&alpha));
            }
        }
        assert!(seen > PETALS / 2, "most petals are falling mid-shower");
        assert!(
            (0..PETALS).all(|index| petal(area, 7, index, SHOWER_SECONDS + 0.1).is_none()),
            "every petal has left by the end of the shower"
        );
        assert_eq!(
            petal(area, 7, 3, 1.0),
            petal(area, 7, 3, 1.0),
            "petals are deterministic"
        );
    }

    #[test]
    fn showers_start_on_their_first_frame_and_end() {
        let mut motion = Motion::default();
        motion.celebrate();
        let context = egui::Context::default();
        let _ =
            super::super::tests::run_headless_frame(&context, egui::RawInput::default(), |ui| {
                paint_shower(ui.ctx(), &mut motion)
            });
        assert!(matches!(motion.shower, Some(Shower::Running { .. })));
        let late = egui::RawInput {
            time: Some(SHOWER_SECONDS + 10.0),
            ..Default::default()
        };
        let _ = super::super::tests::run_headless_frame(&context, late, |ui| {
            paint_shower(ui.ctx(), &mut motion)
        });
        assert!(motion.shower.is_none());
    }
}
