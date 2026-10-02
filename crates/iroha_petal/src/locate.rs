//! Finding the four corner finders in a camera luma plane.
//!
//! Pipeline: adaptive threshold (local mean via an integral image) →
//! 4-connected component labelling → blossom detection (a large, round,
//! isolated blob) → selection of the four finders that form a plausible,
//! similarly sized quadrilateral. Solid blossoms survive defocus that would
//! fill in the gaps of a ring-shaped marker.

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

/// Chooses four finders that look like the corners of one code.
///
/// Lit tiles and merged dots form blob candidates too, so the largest size
/// class is tried first: the corner finders are always the biggest isolated
/// round blobs in view.
#[must_use]
pub fn select_quad(finders: &[Finder]) -> Option<[Finder; 4]> {
    let largest = finders.iter().map(|f| f.size).fold(0.0, f64::max);
    let strong: Vec<Finder> = finders
        .iter()
        .copied()
        .filter(|f| f.size >= 0.55 * largest)
        .collect();
    select_quad_from(&strong).or_else(|| select_quad_from(finders))
}

fn select_quad_from(finders: &[Finder]) -> Option<[Finder; 4]> {
    if finders.len() < 4 {
        return None;
    }
    // Largest first (ties keep discovery order) so that clutter in a busy scene
    // cannot push the real finders out of the ten candidates that are combined.
    let mut order: Vec<usize> = (0..finders.len()).collect();
    order.sort_by(|&a, &b| finders[b].size.total_cmp(&finders[a].size).then(a.cmp(&b)));
    let ranked: Vec<Finder> = order
        .into_iter()
        .take(10)
        .map(|index| finders[index])
        .collect();
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
    let radius = (finder.size * 0.5).ceil() as isize;
    let (cx, cy) = (finder.x.floor() as isize, finder.y.floor() as isize);
    let mut samples = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= image.width as isize || y >= image.height as isize {
                continue;
            }
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            if ((px - finder.x).powi(2) + (py - finder.y).powi(2)).sqrt() <= finder.size * 0.5 {
                samples.push((px, py, f64::from(image.at(x as usize, y as usize))));
            }
        }
    }
    let floor = samples.iter().map(|s| s.2).fold(f64::MAX, f64::min);
    let peak = samples.iter().map(|s| s.2).fold(0.0, f64::max);
    if peak - floor < 20.0 {
        return finder;
    }
    let threshold = floor + 0.5 * (peak - floor);
    let (mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for (px, py, v) in samples {
        let weight = (v - threshold).max(0.0);
        sw += weight;
        sx += weight * px;
        sy += weight * py;
    }
    if sw <= 0.0 {
        finder
    } else {
        Finder {
            x: sx / sw,
            y: sy / sw,
            size: finder.size,
        }
    }
}

/// Locates the four finders of a code, trying progressively stricter
/// thresholds so blurred rings still separate from their cores.
#[must_use]
pub fn locate(image: &Luma) -> Option<[Finder; 4]> {
    for sensitivity in [0.12, 0.22, 0.34] {
        let mask = adaptive_binarize(image, sensitivity);
        let components = label_components(&mask, image.width, image.height);
        let finders = blossoms(&components);
        if let Some(quad) = select_quad(&finders) {
            return Some(quad.map(|f| refine_center(image, f)));
        }
    }
    None
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
    fn a_blank_image_has_no_finders() {
        assert!(locate(&Luma::new(200, 200)).is_none());
    }
}
