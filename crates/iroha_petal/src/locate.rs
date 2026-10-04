//! Finding the four corner finders in a camera luma plane.
//!
//! Pipeline: adaptive threshold (local mean via an integral image) →
//! 4-connected component labelling → blossom detection (a large, round,
//! isolated blob) → selection of the four finders that form a plausible,
//! similarly sized quadrilateral. Solid blossoms survive defocus that would
//! fill in the gaps of a ring-shaped marker.
//!
//! When a finger, a glare or the edge of the frame hides one blossom, three
//! large blossoms that form a corner still identify the code: the fourth
//! corner is inferred (and later refined by the decoder).

use crate::image::Luma;

/// A detected finder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Finder {
    /// Centre `x` in pixel-edge coordinates.
    pub x: f64,
    /// Centre `y` in pixel-edge coordinates.
    pub y: f64,
    /// Apparent outer diameter in pixels.
    pub size: f64,
}

/// One plausible set of corner finders for a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinderSet {
    /// The four corners, clockwise from the one nearest the top-left of the image.
    pub corners: [Finder; 4],
    /// Index into `corners` of a corner that was not seen but inferred from the
    /// other three, if any.
    pub inferred: Option<usize>,
}

/// A connected component of the binarised image.
#[derive(Debug, Clone, Copy, Default)]
pub struct Component {
    /// Pixel count.
    pub area: u32,
    /// Left-most pixel column.
    pub min_x: u32,
    /// Right-most pixel column.
    pub max_x: u32,
    /// Top-most pixel row.
    pub min_y: u32,
    /// Bottom-most pixel row.
    pub max_y: u32,
    sum_x: f64,
    sum_y: f64,
    sum_xx: f64,
    sum_yy: f64,
    sum_xy: f64,
}

impl Component {
    fn width(&self) -> f64 {
        f64::from(self.max_x - self.min_x + 1)
    }

    fn height(&self) -> f64 {
        f64::from(self.max_y - self.min_y + 1)
    }

    fn centroid(&self) -> (f64, f64) {
        (
            self.sum_x / f64::from(self.area),
            self.sum_y / f64::from(self.area),
        )
    }

    /// Ratio of the smaller to the larger principal axis of the blob.
    fn axis_ratio(&self) -> f64 {
        let n = f64::from(self.area);
        let (cx, cy) = self.centroid();
        let vxx = self.sum_xx / n - cx * cx;
        let vyy = self.sum_yy / n - cy * cy;
        let vxy = self.sum_xy / n - cx * cy;
        let mean = 0.5 * (vxx + vyy);
        let spread = (0.25 * (vxx - vyy).powi(2) + vxy * vxy).sqrt();
        let (major, minor) = (mean + spread, (mean - spread).max(0.0));
        if major <= 0.0 {
            0.0
        } else {
            (minor / major).sqrt()
        }
    }
}

/// Marks pixels that are clearly brighter than their neighbourhood.
///
/// `sensitivity` scales the margin above the local mean in units of the
/// image's dynamic range (≈ 0.12 for faint codes, larger values separate
/// blurred finder rings from their cores).
#[must_use]
pub fn adaptive_binarize(image: &Luma, sensitivity: f64) -> Vec<bool> {
    let (w, h) = (image.width, image.height);
    let mut integral = vec![0u64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0u64;
        for x in 0..w {
            row += u64::from(image.data[y * w + x]);
            integral[(y + 1) * (w + 1) + x + 1] = integral[y * (w + 1) + x + 1] + row;
        }
    }
    let mut histogram = [0u32; 256];
    for &v in &image.data {
        histogram[usize::from(v)] += 1;
    }
    let total = image.data.len() as f64;
    let percentile = |p: f64| -> f64 {
        let target = total * p;
        let mut seen = 0.0;
        for (level, &count) in histogram.iter().enumerate() {
            seen += f64::from(count);
            if seen >= target {
                return level as f64;
            }
        }
        255.0
    };
    let low = percentile(0.02);
    let high = percentile(0.995);
    let range = (high - low).max(8.0);
    let radius = (w.min(h) / 8).clamp(12, 64);
    let margin = (sensitivity * range).max(5.0);
    let floor = low + 0.2 * range;
    let mut mask = vec![false; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(radius), (y + radius + 1).min(h));
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(radius), (x + radius + 1).min(w));
            let sum = integral[y1 * (w + 1) + x1] + integral[y0 * (w + 1) + x0]
                - integral[y0 * (w + 1) + x1]
                - integral[y1 * (w + 1) + x0];
            let mean = sum as f64 / ((x1 - x0) * (y1 - y0)) as f64;
            let value = f64::from(image.data[y * w + x]);
            mask[y * w + x] = value > mean + margin && value > floor;
        }
    }
    mask
}

fn find_root(parent: &mut [u32], mut label: u32) -> u32 {
    while parent[label as usize] != label {
        parent[label as usize] = parent[parent[label as usize] as usize];
        label = parent[label as usize];
    }
    label
}

/// Labels 4-connected components; returns the components with area > 0.
#[must_use]
pub fn label_components(mask: &[bool], w: usize, h: usize) -> Vec<Component> {
    let mut labels = vec![0u32; w * h];
    let mut parent: Vec<u32> = vec![0];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !mask[i] {
                continue;
            }
            let left = if x > 0 { labels[i - 1] } else { 0 };
            let up = if y > 0 { labels[i - w] } else { 0 };
            labels[i] = match (left, up) {
                (0, 0) => {
                    let label = parent.len() as u32;
                    parent.push(label);
                    label
                }
                (l, 0) | (0, l) => l,
                (l, u) => {
                    let (a, b) = (find_root(&mut parent, l), find_root(&mut parent, u));
                    let (keep, drop) = if a < b { (a, b) } else { (b, a) };
                    parent[drop as usize] = keep;
                    keep
                }
            };
        }
    }
    let mut components = vec![Component::default(); parent.len()];
    for y in 0..h {
        for x in 0..w {
            let label = labels[y * w + x];
            if label == 0 {
                continue;
            }
            let root = find_root(&mut parent, label) as usize;
            let c = &mut components[root];
            if c.area == 0 {
                c.min_x = x as u32;
                c.max_x = x as u32;
                c.min_y = y as u32;
                c.max_y = y as u32;
            }
            c.area += 1;
            c.min_x = c.min_x.min(x as u32);
            c.max_x = c.max_x.max(x as u32);
            c.min_y = c.min_y.min(y as u32);
            c.max_y = c.max_y.max(y as u32);
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            c.sum_x += px;
            c.sum_y += py;
            c.sum_xx += px * px;
            c.sum_yy += py * py;
            c.sum_xy += px * py;
        }
    }
    components.retain(|c| c.area > 0);
    components
}

/// Detects blossom finders: large, round, isolated blobs.
#[must_use]
pub fn blossoms(components: &[Component]) -> Vec<Finder> {
    let mut found = Vec::new();
    for (index, blob) in components.iter().enumerate() {
        let size = blob.width().max(blob.height());
        let fill = f64::from(blob.area) / (blob.width() * blob.height());
        if size < 14.0
            || blob.area < 100
            || !(0.45..=0.9).contains(&fill)
            || blob.axis_ratio() < 0.5
        {
            continue;
        }
        let (x, y) = blob.centroid();
        // isolation: nothing else of substance close by
        let crowded = components.iter().enumerate().any(|(other, c)| {
            if other == index || c.area < 8 || f64::from(c.area) < 0.015 * f64::from(blob.area) {
                return false;
            }
            let (ox, oy) = c.centroid();
            ((ox - x).powi(2) + (oy - y).powi(2)).sqrt() < 0.8 * size
        });
        if !crowded {
            found.push(Finder { x, y, size });
        }
    }
    found
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Orders four finders clockwise (as displayed, `y` down) starting from the
/// one nearest the top-left of the quadrilateral's bounding box.
fn order_clockwise(mut quad: [Finder; 4]) -> Option<[Finder; 4]> {
    let cx = quad.iter().map(|f| f.x).sum::<f64>() / 4.0;
    let cy = quad.iter().map(|f| f.y).sum::<f64>() / 4.0;
    quad.sort_by(|a, b| {
        (a.y - cy)
            .atan2(a.x - cx)
            .total_cmp(&(b.y - cy).atan2(b.x - cx))
    });
    // atan2 grows clockwise on screen because y points down; verify convexity
    for i in 0..4 {
        let o = (quad[i].x, quad[i].y);
        let a = (quad[(i + 1) % 4].x, quad[(i + 1) % 4].y);
        let b = (quad[(i + 2) % 4].x, quad[(i + 2) % 4].y);
        if cross(o, a, b) <= 0.0 {
            return None;
        }
    }
    let start = (0..4)
        .min_by(|&i, &j| (quad[i].x + quad[i].y).total_cmp(&(quad[j].x + quad[j].y)))
        .unwrap_or(0);
    Some([
        quad[start],
        quad[(start + 1) % 4],
        quad[(start + 2) % 4],
        quad[(start + 3) % 4],
    ])
}

/// The finders of the largest size class: lit tiles and merged dots form blob
/// candidates too, but the corner finders are the biggest isolated round blobs
/// in view.
fn strong_finders(finders: &[Finder]) -> Vec<Finder> {
    let largest = finders.iter().map(|f| f.size).fold(0.0, f64::max);
    finders
        .iter()
        .copied()
        .filter(|f| f.size >= 0.55 * largest)
        .collect()
}

/// The ten largest candidates, largest first (ties keep discovery order), so
/// that clutter in a busy scene cannot push the real finders out of the set
/// that is combined.
fn ranked(finders: &[Finder]) -> Vec<Finder> {
    let mut order: Vec<usize> = (0..finders.len()).collect();
    order.sort_by(|&a, &b| finders[b].size.total_cmp(&finders[a].size).then(a.cmp(&b)));
    order
        .into_iter()
        .take(10)
        .map(|index| finders[index])
        .collect()
}

/// Chooses four finders that look like the corners of one code, trying the
/// largest size class first.
#[must_use]
pub fn select_quad(finders: &[Finder]) -> Option<[Finder; 4]> {
    select_quad_from(&strong_finders(finders)).or_else(|| select_quad_from(finders))
}

/// Chooses three finders that look like three corners of one code (an `L`:
/// similar sizes, two similar legs at a roughly right angle) and completes the
/// fourth corner as a parallelogram. Returns the clockwise quad and the index
/// of the inferred corner in it.
#[must_use]
pub fn select_triple(finders: &[Finder]) -> Option<([Finder; 4], usize)> {
    let ranked = ranked(finders);
    let n = ranked.len();
    let mut best: Option<(f64, [Finder; 4], usize)> = None;
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                let set = [ranked[a], ranked[b], ranked[c]];
                let smin = set.iter().map(|f| f.size).fold(f64::MAX, f64::min);
                let smax = set.iter().map(|f| f.size).fold(0.0, f64::max);
                if smax / smin > 1.9 {
                    continue;
                }
                let mean_size = set.iter().map(|f| f.size).sum::<f64>() / 3.0;
                for corner in 0..3 {
                    let k = set[corner];
                    let p = set[(corner + 1) % 3];
                    let q = set[(corner + 2) % 3];
                    let (ux, uy) = (p.x - k.x, p.y - k.y);
                    let (vx, vy) = (q.x - k.x, q.y - k.y);
                    let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
                    if lu <= 0.0 || lv <= 0.0 {
                        continue;
                    }
                    let legs = lu.max(lv) / lu.min(lv);
                    let cos = (ux * vx + uy * vy) / (lu * lv);
                    // canvas geometry: side / finder diameter = 880 / 120
                    let ratio = 0.5 * (lu + lv) / mean_size;
                    if legs > 2.0 || cos.abs() > 0.5 || !(4.8..=10.5).contains(&ratio) {
                        continue;
                    }
                    let fourth = Finder {
                        x: p.x + q.x - k.x,
                        y: p.y + q.y - k.y,
                        size: mean_size,
                    };
                    let Some(quad) = order_clockwise([k, p, q, fourth]) else {
                        continue;
                    };
                    let Some(inferred) = quad.iter().position(|f| {
                        f.x.to_bits() == fourth.x.to_bits() && f.y.to_bits() == fourth.y.to_bits()
                    }) else {
                        continue;
                    };
                    let score = (smax / smin - 1.0)
                        + (legs - 1.0)
                        + cos.abs()
                        + ((ratio - 7.33) / 7.33).abs();
                    if best.as_ref().is_none_or(|(s, _, _)| score < *s) {
                        best = Some((score, quad, inferred));
                    }
                }
            }
        }
    }
    best.map(|(_, quad, inferred)| (quad, inferred))
}

fn select_quad_from(finders: &[Finder]) -> Option<[Finder; 4]> {
    if finders.len() < 4 {
        return None;
    }
    let ranked = ranked(finders);
    let mut best: Option<(f64, [Finder; 4])> = None;
    let n = ranked.len();
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                for d in (c + 1)..n {
                    let set = [ranked[a], ranked[b], ranked[c], ranked[d]];
                    let sizes: Vec<f64> = set.iter().map(|f| f.size).collect();
                    let (smin, smax) = (
                        sizes.iter().copied().fold(f64::MAX, f64::min),
                        sizes.iter().copied().fold(0.0, f64::max),
                    );
                    if smax / smin > 1.9 {
                        continue;
                    }
                    let Some(quad) = order_clockwise(set) else {
                        continue;
                    };
                    let side = |i: usize| {
                        let (p, q) = (quad[i], quad[(i + 1) % 4]);
                        ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt()
                    };
                    let sides = [side(0), side(1), side(2), side(3)];
                    let (lmin, lmax) = (
                        sides.iter().copied().fold(f64::MAX, f64::min),
                        sides.iter().copied().fold(0.0, f64::max),
                    );
                    let mean_size = sizes.iter().sum::<f64>() / 4.0;
                    // canvas geometry: side / finder diameter = 880 / 120
                    let ratio = (sides.iter().sum::<f64>() / 4.0) / mean_size;
                    if lmax / lmin > 2.6 || !(4.8..=10.5).contains(&ratio) {
                        continue;
                    }
                    let score =
                        (smax / smin - 1.0) + (lmax / lmin - 1.0) + ((ratio - 7.33) / 7.33).abs();
                    if best.as_ref().is_none_or(|(s, _)| score < *s) {
                        best = Some((score, quad));
                    }
                }
            }
        }
    }
    best.map(|(_, quad)| quad)
}

/// Sharpens a finder centre with an intensity-weighted centroid.
#[must_use]
pub fn refine_center(image: &Luma, finder: Finder) -> Finder {
    centroid(image, finder).unwrap_or(finder)
}

/// The intensity-weighted centroid of the bright part of the disc of diameter
/// `finder.size` around the finder, or `None` when that disc has less than 20
/// levels of contrast (nothing bright is there) or the finder is not finite.
fn centroid(image: &Luma, finder: Finder) -> Option<Finder> {
    if !(finder.x.is_finite() && finder.y.is_finite() && finder.size.is_finite()) {
        return None;
    }
    let radius = (finder.size * 0.5).ceil() as isize;
    let (cx, cy) = (finder.x.floor() as isize, finder.y.floor() as isize);
    // only the part of the square inside the image is visited (same pixels, same order)
    let (x0, x1) = (
        cx.saturating_sub(radius).max(0),
        cx.saturating_add(radius).min(image.width as isize - 1),
    );
    let (y0, y1) = (
        cy.saturating_sub(radius).max(0),
        cy.saturating_add(radius).min(image.height as isize - 1),
    );
    let mut samples = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            if ((px - finder.x).powi(2) + (py - finder.y).powi(2)).sqrt() <= finder.size * 0.5 {
                samples.push((px, py, f64::from(image.at(x as usize, y as usize))));
            }
        }
    }
    let floor = samples.iter().map(|s| s.2).fold(f64::MAX, f64::min);
    let peak = samples.iter().map(|s| s.2).fold(0.0, f64::max);
    if peak - floor < 20.0 {
        return None;
    }
    let threshold = floor + 0.5 * (peak - floor);
    let (mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for (px, py, v) in samples {
        let weight = (v - threshold).max(0.0);
        sw += weight;
        sx += weight * px;
        sy += weight * py;
    }
    (sw > 0.0).then(|| Finder {
        x: sx / sw,
        y: sy / sw,
        size: finder.size,
    })
}

/// Re-finds a finder near where it is expected (from the previous frame's pose).
///
/// A first centroid over a disc twice the finder's diameter catches a blossom
/// that moved up to about one diameter (nothing else bright is that close to a
/// corner finder); centroids over the finder's own disc then repeat, at most
/// five times, until the centre moves less than a quarter pixel. `None` when
/// nothing bright is there or the result is more than 0.75 diameters from the
/// expected centre, which means the code moved too far for tracking.
#[must_use]
pub fn follow(image: &Luma, expected: Finder) -> Option<Finder> {
    let wide = centroid(
        image,
        Finder {
            size: 2.0 * expected.size,
            ..expected
        },
    )?;
    let mut current = Finder {
        size: expected.size,
        ..wide
    };
    for _ in 0..5 {
        let next = centroid(image, current)?;
        let step = ((next.x - current.x).powi(2) + (next.y - current.y).powi(2)).sqrt();
        current = next;
        if step < 0.25 {
            break;
        }
    }
    let moved = ((current.x - expected.x).powi(2) + (current.y - expected.y).powi(2)).sqrt();
    (moved <= 0.75 * expected.size).then_some(current)
}

/// Locates four seen finders of a code (the first candidate of [`candidates`]
/// without an inferred corner).
#[must_use]
pub fn locate(image: &Luma) -> Option<[Finder; 4]> {
    candidates(image)
        .find(|set| set.inferred.is_none())
        .map(|set| set.corners)
}

/// All candidate finder sets for one frame, in the order of [`candidates`].
#[must_use]
pub fn locate_candidates(image: &Luma) -> Vec<FinderSet> {
    candidates(image).collect()
}

/// Binarisation thresholds, from the most to the least permissive: a higher
/// sensitivity separates blurred blossoms from their surroundings.
pub const SENSITIVITIES: [f64; 3] = [0.12, 0.22, 0.34];

/// Candidate finder sets for one frame, produced lazily in the order a decoder
/// should try them, so that a clean frame costs one binarisation.
///
/// For each threshold of [`SENSITIVITIES`] in turn: four finders of the largest
/// size class that form a quad. Then, from the first threshold that had them,
/// three large finders forming a corner — completed by the nearest smaller blob
/// within 0.3 legs of where the fourth corner belongs (steep tilt makes the far
/// finder small) — then the first quad that smaller blobs form, and last the
/// same three finders with the fourth corner inferred (the nearby blob may have
/// been merged ring dots, a quad may have been clutter).
#[must_use]
pub fn candidates(image: &Luma) -> Candidates<'_> {
    Candidates {
        image,
        stage: 0,
        completed: None,
        smaller: None,
        inferred: None,
        tail: Vec::new(),
    }
}

/// The iterator of [`candidates`].
#[derive(Debug)]
pub struct Candidates<'a> {
    image: &'a Luma,
    stage: usize,
    completed: Option<[Finder; 4]>,
    smaller: Option<[Finder; 4]>,
    inferred: Option<([Finder; 4], usize)>,
    tail: Vec<FinderSet>,
}

impl Iterator for Candidates<'_> {
    type Item = FinderSet;

    fn next(&mut self) -> Option<FinderSet> {
        let image = self.image;
        while self.stage < SENSITIVITIES.len() {
            let sensitivity = SENSITIVITIES[self.stage];
            self.stage += 1;
            let mask = adaptive_binarize(image, sensitivity);
            let components = label_components(&mask, image.width, image.height);
            let finders = blossoms(&components);
            let strong = strong_finders(&finders);
            if self.inferred.is_none()
                && let Some((quad, missing)) = select_triple(&strong)
            {
                self.completed = complete_triple(&finders, quad, missing)
                    .map(|full| full.map(|f| refine_center(image, f)));
                let mut corners = quad;
                for (i, corner) in corners.iter_mut().enumerate() {
                    if i != missing {
                        *corner = refine_center(image, *corner);
                    }
                }
                self.inferred = Some((corners, missing));
            }
            if self.smaller.is_none() {
                self.smaller =
                    select_quad_from(&finders).map(|q| q.map(|f| refine_center(image, f)));
            }
            if let Some(quad) = select_quad_from(&strong) {
                return Some(FinderSet {
                    corners: quad.map(|f| refine_center(image, f)),
                    inferred: None,
                });
            }
        }
        if self.stage == SENSITIVITIES.len() {
            self.stage += 1;
            let seen = [self.completed.take(), self.smaller.take()]
                .into_iter()
                .flatten()
                .map(|corners| FinderSet {
                    corners,
                    inferred: None,
                });
            let guessed = self.inferred.take().map(|(corners, missing)| FinderSet {
                corners,
                inferred: Some(missing),
            });
            self.tail = seen.chain(guessed).collect();
            self.tail.reverse();
        }
        self.tail.pop()
    }
}

/// A blob of at least 0.3 × the finder size within 0.3 legs of the inferred
/// corner of a triple completes it into a seen quad.
fn complete_triple(finders: &[Finder], quad: [Finder; 4], missing: usize) -> Option<[Finder; 4]> {
    let d = quad[missing];
    let distance = |f: &Finder| ((f.x - d.x).powi(2) + (f.y - d.y).powi(2)).sqrt();
    let leg = 0.5 * (distance(&quad[(missing + 1) % 4]) + distance(&quad[(missing + 3) % 4]));
    let fourth = finders
        .iter()
        .filter(|f| f.size >= 0.3 * d.size && distance(f) <= 0.3 * leg)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))?;
    let mut full = quad;
    full[missing] = *fourth;
    order_clockwise(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderOptions, render};
    use crate::stream::StreamEncoder;

    fn frame() -> crate::image::Rgb {
        let encoder = StreamEncoder::new(&[9u8; 200], 1).unwrap();
        render(
            &encoder.cells(1),
            &RenderOptions {
                size: 512,
                supersample: 2,
                ..RenderOptions::default()
            },
        )
    }

    #[test]
    fn finds_the_four_corner_blossoms_in_a_clean_render() {
        let luma = frame().to_luma();
        let quad = locate(&luma).expect("four finders");
        let expected = [(36.0, 36.0), (476.0, 36.0), (476.0, 476.0), (36.0, 476.0)];
        for (finder, (ex, ey)) in quad.iter().zip(expected) {
            assert!(
                (finder.x - ex).abs() < 1.5 && (finder.y - ey).abs() < 1.5,
                "{finder:?}"
            );
            assert!((finder.size - 60.0).abs() < 6.0, "size {}", finder.size);
        }
    }

    #[test]
    fn components_are_labelled_with_correct_geometry() {
        let mut mask = vec![false; 8 * 8];
        for y in 1..4 {
            for x in 2..6 {
                mask[y * 8 + x] = true;
            }
        }
        mask[6 * 8 + 6] = true;
        let components = label_components(&mask, 8, 8);
        assert_eq!(components.len(), 2);
        let big = components.iter().find(|c| c.area == 12).unwrap();
        assert_eq!((big.min_x, big.max_x, big.min_y, big.max_y), (2, 5, 1, 3));
        let (cx, cy) = big.centroid();
        assert!((cx - 4.0).abs() < 1e-9 && (cy - 2.5).abs() < 1e-9);
    }

    #[test]
    fn ordering_is_clockwise_from_the_top_left() {
        let f = |x: f64, y: f64| Finder { x, y, size: 10.0 };
        let quad =
            order_clockwise([f(90.0, 90.0), f(10.0, 12.0), f(88.0, 8.0), f(12.0, 92.0)]).unwrap();
        assert_eq!((quad[0].x, quad[0].y), (10.0, 12.0));
        assert_eq!((quad[1].x, quad[1].y), (88.0, 8.0));
        assert_eq!((quad[2].x, quad[2].y), (90.0, 90.0));
    }

    #[test]
    fn decoys_that_pass_the_size_filter_cannot_displace_the_real_finders() {
        // Decoys of size 40 are at least 0.55 x the real finders (about 60), so the
        // size-class filter keeps them; only largest-first ranking keeps the four
        // real finders inside the ten candidates that are combined.
        let blob = |x: f64, y: f64, size: f64| Finder { x, y, size };
        let mut candidates: Vec<Finder> = (0..12)
            .map(|i| blob(10.0 + 7.0 * f64::from(i), 5.0, 40.0))
            .collect();
        candidates.extend([
            blob(100.0, 100.0, 60.0),
            blob(700.0, 110.0, 62.0),
            blob(690.0, 520.0, 58.0),
            blob(95.0, 510.0, 61.0),
        ]);
        let quad = select_quad(&candidates).expect("real finders found");
        let mut xs: Vec<i64> = quad.iter().map(|f| f.x as i64).collect();
        xs.sort_unstable();
        assert_eq!(xs, vec![95, 100, 690, 700]);
    }

    #[test]
    fn the_largest_candidates_win_when_clutter_precedes_them() {
        // twelve small decoys discovered before the four real finders
        let blob = |x: f64, y: f64, size: f64| Finder { x, y, size };
        let mut candidates: Vec<Finder> = (0..12)
            .map(|i| blob(10.0 + 7.0 * f64::from(i), 5.0, 18.0))
            .collect();
        candidates.extend([
            blob(100.0, 100.0, 60.0),
            blob(700.0, 110.0, 62.0),
            blob(690.0, 520.0, 58.0),
            blob(95.0, 510.0, 61.0),
        ]);
        let quad = select_quad(&candidates).expect("real finders found");
        let mut xs: Vec<i64> = quad.iter().map(|f| f.x as i64).collect();
        xs.sort_unstable();
        assert_eq!(xs, vec![95, 100, 690, 700]);
    }

    #[test]
    fn three_finders_forming_a_corner_infer_the_fourth() {
        let blob = |x: f64, y: f64| Finder { x, y, size: 60.0 };
        // top-left, top-right and bottom-left of a slightly rotated square, plus clutter
        let mut finders = vec![blob(100.0, 110.0), blob(540.0, 90.0), blob(120.0, 550.0)];
        finders.extend((0..5).map(|i| Finder {
            x: 300.0 + 10.0 * f64::from(i),
            y: 300.0,
            size: 14.0,
        }));
        let (quad, inferred) = select_triple(&finders).expect("a corner of three");
        let fourth = quad[inferred];
        assert!((fourth.x - 560.0).abs() < 1e-9 && (fourth.y - 530.0).abs() < 1e-9);
        assert_eq!(
            inferred, 2,
            "the inferred corner is bottom-right in clockwise order"
        );
        // three blossoms in a row are no corner
        assert!(select_triple(&[blob(0.0, 0.0), blob(440.0, 0.0), blob(880.0, 0.0)]).is_none());
    }

    #[test]
    fn a_smaller_blob_at_the_inferred_corner_completes_the_quad() {
        let blob = |x: f64, y: f64, size: f64| Finder { x, y, size };
        // steep tilt: the far finder is under 0.55 of the largest, but it is where the
        // fourth corner belongs
        let finders = [
            blob(100.0, 100.0, 64.0),
            blob(540.0, 100.0, 60.0),
            blob(100.0, 540.0, 62.0),
            blob(520.0, 515.0, 30.0),
        ];
        let strong = strong_finders(&finders);
        assert_eq!(strong.len(), 3);
        let (quad, missing) = select_triple(&strong).expect("triple");
        let full = complete_triple(&finders, quad, missing).expect("completed");
        assert!(
            full.iter()
                .any(|f| (f.x - 520.0).abs() < 1e-9 && (f.y - 515.0).abs() < 1e-9)
        );
    }

    #[test]
    fn a_hidden_blossom_yields_an_inferred_candidate() {
        let mut rgb = frame();
        // paint over the bottom-left blossom (centre 36, 476 at this size)
        let n = rgb.width;
        for y in 420..n {
            for x in 0..92 {
                rgb.data[(y * n + x) * 3..(y * n + x) * 3 + 3].fill(0);
            }
        }
        let candidates = locate_candidates(&rgb.to_luma());
        let inferred = candidates
            .iter()
            .find(|set| set.inferred.is_some())
            .expect("an inferred candidate");
        let corner = inferred.corners[inferred.inferred.unwrap_or(0)];
        assert!(
            (corner.x - 36.0).abs() < 4.0 && (corner.y - 476.0).abs() < 4.0,
            "{corner:?}"
        );
    }

    #[test]
    fn following_finds_a_moved_blossom_and_refuses_a_lost_one() {
        let luma = frame().to_luma();
        let truth = Finder {
            x: 36.0,
            y: 36.0,
            size: 60.0,
        };
        let expected = Finder {
            x: 48.0,
            y: 27.0,
            ..truth
        };
        let found = follow(&luma, expected).expect("followed");
        assert!(
            (found.x - 36.0).abs() < 1.5 && (found.y - 36.0).abs() < 1.5,
            "{found:?}"
        );
        // nothing bright near the centre of the canvas corner gap
        let empty = Finder {
            x: 140.0,
            y: 36.0,
            size: 30.0,
        };
        assert!(follow(&luma, empty).is_none());
        // absurd expectations (a broken pose) are refused and never overflow
        for (x, y, size) in [
            (1e300, 36.0, 60.0),
            (-1e300, -1e300, 60.0),
            (f64::NAN, 36.0, 60.0),
            (36.0, f64::INFINITY, 60.0),
            (36.0, 36.0, f64::NAN),
        ] {
            assert!(follow(&luma, Finder { x, y, size }).is_none());
        }
        // a huge disc just covers the whole image
        let _ = follow(
            &luma,
            Finder {
                x: 36.0,
                y: 36.0,
                size: 1e300,
            },
        );
    }

    #[test]
    fn a_blank_image_has_no_finders() {
        assert!(locate(&Luma::new(200, 200)).is_none());
    }
}
