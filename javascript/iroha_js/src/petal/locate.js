// Finding the four corner finders in a camera luma plane.
//
// Pipeline: adaptive threshold (local mean via an integral image) ->
// 4-connected component labelling -> blossom detection (a large, round,
// isolated blob) -> selection of the four finders that form a plausible,
// similarly sized quadrilateral. Solid blossoms survive defocus that would
// fill in the gaps of a ring-shaped marker.
//
// Large per-frame buffers (integral image, mask, labels) are kept in a
// module-level scratch area and reused across frames; every function here is
// synchronous, so the reuse is never observable.

import { fmax, fmin, totalCmp } from "./support.js";

const scratch = {
  integral: new Uint32Array(0),
  integralWide: new Float64Array(0),
  mask: new Uint8Array(0),
  labels: new Uint32Array(0),
  parent: new Uint32Array(1024),
};

function grow(current, length, Type) {
  if (current.length >= length) {
    return current;
  }
  let capacity = Math.max(current.length, 1024);
  while (capacity < length) {
    capacity *= 2;
  }
  return new Type(capacity);
}

/**
 * Integral image and global statistics that do not depend on the threshold
 * sensitivity, computed once per image.
 */
function prepareThreshold(image) {
  const w = image.width;
  const h = image.height;
  const data = image.data;
  const stride = w + 1;
  const cells = stride * (h + 1);
  // Sums stay below 255 * w * h; 32-bit cells are exact up to 16.8 Mpx.
  let integral;
  if (255 * w * h < 2 ** 32) {
    scratch.integral = grow(scratch.integral, cells, Uint32Array);
    integral = scratch.integral;
  } else {
    scratch.integralWide = grow(scratch.integralWide, cells, Float64Array);
    integral = scratch.integralWide;
  }
  for (let index = 0; index < stride; index += 1) {
    integral[index] = 0;
  }
  for (let y = 0; y < h; y += 1) {
    let row = 0;
    const base = (y + 1) * stride;
    const previous = y * stride;
    const source = y * w;
    integral[base] = 0;
    for (let x = 0; x < w; x += 1) {
      row += data[source + x];
      integral[base + x + 1] = integral[previous + x + 1] + row;
    }
  }
  const histogram = new Uint32Array(256);
  for (let index = 0; index < w * h; index += 1) {
    histogram[data[index]] += 1;
  }
  const total = w * h;
  const percentile = (p) => {
    const target = total * p;
    let seen = 0;
    for (let level = 0; level < 256; level += 1) {
      seen += histogram[level];
      if (seen >= target) {
        return level;
      }
    }
    return 255;
  };
  const low = percentile(0.02);
  const high = percentile(0.995);
  const range = fmax(high - low, 8);
  const radius = Math.min(Math.max(Math.floor(Math.min(w, h) / 8), 12), 64);
  return { integral, stride, low, range, radius, floor: low + 0.2 * range };
}

function binarizeInto(image, prepared, sensitivity, mask) {
  const w = image.width;
  const h = image.height;
  const data = image.data;
  const { integral, stride, radius, floor } = prepared;
  const margin = fmax(sensitivity * prepared.range, 5);
  for (let y = 0; y < h; y += 1) {
    const y0 = y > radius ? y - radius : 0;
    const y1 = y + radius + 1 < h ? y + radius + 1 : h;
    const top = y0 * stride;
    const bottom = y1 * stride;
    const rows = y1 - y0;
    let index = y * w;
    for (let x = 0; x < w; x += 1, index += 1) {
      const value = data[index];
      // `value > floor` is the cheaper half of the reference condition and
      // rejects most of a dark frame; the order does not change the result.
      if (value <= floor) {
        mask[index] = 0;
        continue;
      }
      const x0 = x > radius ? x - radius : 0;
      const x1 = x + radius + 1 < w ? x + radius + 1 : w;
      const sum = integral[bottom + x1] + integral[top + x0] - integral[top + x1] - integral[bottom + x0];
      const mean = sum / ((x1 - x0) * rows);
      mask[index] = value > mean + margin ? 1 : 0;
    }
  }
  return mask;
}

/**
 * Marks pixels that are clearly brighter than their neighbourhood.
 *
 * `sensitivity` scales the margin above the local mean in units of the image's
 * dynamic range (~0.12 for faint codes, larger values separate blurred finder
 * rings from their cores).
 *
 * @returns {Uint8Array} `1` for marked pixels, row-major
 */
export function adaptiveBinarize(image, sensitivity) {
  const prepared = prepareThreshold(image);
  return binarizeInto(image, prepared, sensitivity, new Uint8Array(image.width * image.height));
}

function findRoot(parent, label) {
  let current = label;
  while (parent[current] !== current) {
    parent[current] = parent[parent[current]];
    current = parent[current];
  }
  return current;
}

/**
 * Labels 4-connected components into a struct-of-arrays table. Components are
 * ordered by their smallest label, i.e. by first appearance in raster order.
 */
function labelTable(mask, w, h) {
  scratch.labels = grow(scratch.labels, w * h, Uint32Array);
  const labels = scratch.labels;
  let parent = scratch.parent;
  parent[0] = 0;
  let count = 1;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const i = y * w + x;
      if (mask[i] === 0) {
        labels[i] = 0;
        continue;
      }
      const left = x > 0 ? labels[i - 1] : 0;
      const up = y > 0 ? labels[i - w] : 0;
      if (left === 0 && up === 0) {
        if (count === parent.length) {
          const grown = new Uint32Array(parent.length * 2);
          grown.set(parent);
          parent = grown;
          scratch.parent = grown;
        }
        parent[count] = count;
        labels[i] = count;
        count += 1;
      } else if (up === 0) {
        labels[i] = left;
      } else if (left === 0) {
        labels[i] = up;
      } else {
        const a = findRoot(parent, left);
        const b = findRoot(parent, up);
        const keep = a < b ? a : b;
        const drop = a < b ? b : a;
        parent[drop] = keep;
        labels[i] = keep;
      }
    }
  }
  // Roots are the smallest label of their set, so ascending root order is the
  // reference's component order.
  const compact = new Int32Array(count).fill(-1);
  let components = 0;
  for (let label = 1; label < count; label += 1) {
    if (findRoot(parent, label) === label) {
      compact[label] = components;
      components += 1;
    }
  }
  const table = {
    count: components,
    area: new Uint32Array(components),
    minX: new Uint32Array(components),
    maxX: new Uint32Array(components),
    minY: new Uint32Array(components),
    maxY: new Uint32Array(components),
    sumX: new Float64Array(components),
    sumY: new Float64Array(components),
    sumXX: new Float64Array(components),
    sumYY: new Float64Array(components),
    sumXY: new Float64Array(components),
  };
  const { area, minX, maxX, minY, maxY, sumX, sumY, sumXX, sumYY, sumXY } = table;
  for (let y = 0; y < h; y += 1) {
    const py = y + 0.5;
    for (let x = 0; x < w; x += 1) {
      const label = labels[y * w + x];
      if (label === 0) continue;
      const c = compact[findRoot(parent, label)];
      if (area[c] === 0) {
        minX[c] = x;
        maxX[c] = x;
        minY[c] = y;
        maxY[c] = y;
      }
      area[c] += 1;
      if (x < minX[c]) minX[c] = x;
      if (x > maxX[c]) maxX[c] = x;
      if (y < minY[c]) minY[c] = y;
      if (y > maxY[c]) maxY[c] = y;
      const px = x + 0.5;
      sumX[c] += px;
      sumY[c] += py;
      sumXX[c] += px * px;
      sumYY[c] += py * py;
      sumXY[c] += px * py;
    }
  }
  return table;
}

/**
 * Labels the 4-connected components of a mask; returns the components with
 * their bounding boxes and first and second moments, ordered by first
 * appearance in raster order.
 *
 * @param {Uint8Array | ArrayLike<number | boolean>} mask row-major, truthy is set
 */
export function labelComponents(mask, w, h) {
  if (mask === null || typeof mask !== "object" || mask.length !== w * h) {
    throw new TypeError("mask must hold width * height cells");
  }
  const bytes = mask instanceof Uint8Array ? mask : Uint8Array.from(mask, (cell) => (cell ? 1 : 0));
  const table = labelTable(bytes, w, h);
  const out = [];
  for (let c = 0; c < table.count; c += 1) {
    out.push(
      Object.freeze({
        area: table.area[c],
        minX: table.minX[c],
        maxX: table.maxX[c],
        minY: table.minY[c],
        maxY: table.maxY[c],
        sumX: table.sumX[c],
        sumY: table.sumY[c],
        sumXX: table.sumXX[c],
        sumYY: table.sumYY[c],
        sumXY: table.sumXY[c],
      }),
    );
  }
  return out;
}

function tableFromComponents(components) {
  if (!Array.isArray(components)) {
    throw new TypeError("components must be an array");
  }
  const count = components.length;
  const table = {
    count,
    area: new Uint32Array(count),
    minX: new Uint32Array(count),
    maxX: new Uint32Array(count),
    minY: new Uint32Array(count),
    maxY: new Uint32Array(count),
    sumX: new Float64Array(count),
    sumY: new Float64Array(count),
    sumXX: new Float64Array(count),
    sumYY: new Float64Array(count),
    sumXY: new Float64Array(count),
  };
  components.forEach((component, index) => {
    for (const key of ["area", "minX", "maxX", "minY", "maxY", "sumX", "sumY", "sumXX", "sumYY", "sumXY"]) {
      table[key][index] = component[key];
    }
  });
  return table;
}

/** Ratio of the smaller to the larger principal axis of component `c`. */
function axisRatio(table, c) {
  const n = table.area[c];
  const cx = table.sumX[c] / n;
  const cy = table.sumY[c] / n;
  const vxx = table.sumXX[c] / n - cx * cx;
  const vyy = table.sumYY[c] / n - cy * cy;
  const vxy = table.sumXY[c] / n - cx * cy;
  const mean = 0.5 * (vxx + vyy);
  const difference = vxx - vyy;
  const spread = Math.sqrt(0.25 * (difference * difference) + vxy * vxy);
  const major = mean + spread;
  const minor = fmax(mean - spread, 0);
  return major <= 0 ? 0 : Math.sqrt(minor / major);
}

function blossomsFromTable(table) {
  const { count, area, minX, maxX, minY, maxY, sumX, sumY } = table;
  const found = [];
  for (let index = 0; index < count; index += 1) {
    const width = maxX[index] - minX[index] + 1;
    const height = maxY[index] - minY[index] + 1;
    const size = Math.max(width, height);
    const fill = area[index] / (width * height);
    if (size < 14 || area[index] < 100 || !(fill >= 0.45 && fill <= 0.9) || axisRatio(table, index) < 0.5) {
      continue;
    }
    const x = sumX[index] / area[index];
    const y = sumY[index] / area[index];
    // isolation: nothing else of substance close by
    let crowded = false;
    for (let other = 0; other < count; other += 1) {
      if (other === index || area[other] < 8 || area[other] < 0.015 * area[index]) {
        continue;
      }
      const ox = sumX[other] / area[other];
      const oy = sumY[other] / area[other];
      if (Math.sqrt((ox - x) * (ox - x) + (oy - y) * (oy - y)) < 0.8 * size) {
        crowded = true;
        break;
      }
    }
    if (!crowded) {
      found.push({ x, y, size });
    }
  }
  return found;
}

/**
 * Detects blossom finders (large, round, isolated blobs) among the
 * components returned by {@link labelComponents}.
 *
 * @returns {Array<{x: number, y: number, size: number}>}
 */
export function blossoms(components) {
  return blossomsFromTable(tableFromComponents(components));
}

function cross(o, a, b) {
  return (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
}

/**
 * Orders four finders clockwise (as displayed, `y` down) starting from the one
 * nearest the top-left of the quadrilateral's bounding box.
 */
function orderClockwise(set) {
  const cx = (set[0].x + set[1].x + set[2].x + set[3].x) / 4;
  const cy = (set[0].y + set[1].y + set[2].y + set[3].y) / 4;
  const quad = set
    .map((finder) => ({ finder, angle: Math.atan2(finder.y - cy, finder.x - cx) }))
    .sort((a, b) => totalCmp(a.angle, b.angle))
    .map((entry) => entry.finder);
  // atan2 grows clockwise on screen because y points down; verify convexity
  for (let i = 0; i < 4; i += 1) {
    if (cross(quad[i], quad[(i + 1) % 4], quad[(i + 2) % 4]) <= 0) {
      return null;
    }
  }
  let start = 0;
  for (let i = 1; i < 4; i += 1) {
    if (totalCmp(quad[i].x + quad[i].y, quad[start].x + quad[start].y) < 0) {
      start = i;
    }
  }
  return [quad[start], quad[(start + 1) % 4], quad[(start + 2) % 4], quad[(start + 3) % 4]];
}

function selectQuadFrom(finders) {
  if (finders.length < 4) {
    return null;
  }
  // Largest first (ties keep discovery order) so that clutter in a busy scene
  // cannot push the real finders out of the ten candidates that are combined.
  const ranked = finders
    .map((finder, index) => ({ finder, index }))
    .sort((a, b) => totalCmp(b.finder.size, a.finder.size) || a.index - b.index)
    .slice(0, 10)
    .map((entry) => entry.finder);
  let best = null;
  let bestScore = 0;
  const n = ranked.length;
  for (let a = 0; a < n; a += 1) {
    for (let b = a + 1; b < n; b += 1) {
      for (let c = b + 1; c < n; c += 1) {
        for (let d = c + 1; d < n; d += 1) {
          const set = [ranked[a], ranked[b], ranked[c], ranked[d]];
          let smin = Number.MAX_VALUE;
          let smax = 0;
          for (const finder of set) {
            smin = fmin(smin, finder.size);
            smax = fmax(smax, finder.size);
          }
          if (smax / smin > 1.9) {
            continue;
          }
          const quad = orderClockwise(set);
          if (quad === null) {
            continue;
          }
          const sides = [0, 1, 2, 3].map((i) => {
            const p = quad[i];
            const q = quad[(i + 1) % 4];
            return Math.sqrt((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y));
          });
          let lmin = Number.MAX_VALUE;
          let lmax = 0;
          for (const side of sides) {
            lmin = fmin(lmin, side);
            lmax = fmax(lmax, side);
          }
          const meanSize = (set[0].size + set[1].size + set[2].size + set[3].size) / 4;
          // canvas geometry: side / finder diameter = 880 / 120
          const ratio = (sides[0] + sides[1] + sides[2] + sides[3]) / 4 / meanSize;
          if (lmax / lmin > 2.6 || !(ratio >= 4.8 && ratio <= 10.5)) {
            continue;
          }
          const score = smax / smin - 1 + (lmax / lmin - 1) + Math.abs((ratio - 7.33) / 7.33);
          if (best === null || score < bestScore) {
            best = quad;
            bestScore = score;
          }
        }
      }
    }
  }
  return best;
}

/**
 * Chooses four finders that look like the corners of one code.
 *
 * Lit tiles and merged dots form blob candidates too, so the largest size
 * class is tried first and candidates are combined largest first: the corner
 * finders are always the biggest isolated round blobs in view.
 *
 * @returns {Array<{x: number, y: number, size: number}> | null} clockwise from the top-left
 */
export function selectQuad(finders) {
  if (!Array.isArray(finders)) {
    throw new TypeError("finders must be an array");
  }
  let largest = 0;
  for (const finder of finders) {
    largest = fmax(largest, finder.size);
  }
  const strong = finders.filter((finder) => finder.size >= 0.55 * largest);
  return selectQuadFrom(strong) ?? selectQuadFrom(finders);
}

/**
 * Sharpens a finder centre with an intensity-weighted centroid.
 *
 * @returns {{x: number, y: number, size: number}}
 */
export function refineCenter(image, finder) {
  const radius = Math.ceil(finder.size * 0.5);
  const cx = Math.floor(finder.x);
  const cy = Math.floor(finder.y);
  const w = image.width;
  const h = image.height;
  const data = image.data;
  const limit = finder.size * 0.5;
  // Two passes over the same pixels in the same order as the reference's
  // sample list: extremes first, then the weighted centroid.
  let floor = Number.MAX_VALUE;
  let peak = 0;
  for (let dy = -radius; dy <= radius; dy += 1) {
    const y = cy + dy;
    if (y < 0 || y >= h) continue;
    const py = y + 0.5;
    for (let dx = -radius; dx <= radius; dx += 1) {
      const x = cx + dx;
      if (x < 0 || x >= w) continue;
      const px = x + 0.5;
      if (Math.sqrt((px - finder.x) * (px - finder.x) + (py - finder.y) * (py - finder.y)) <= limit) {
        const value = data[y * w + x];
        floor = fmin(floor, value);
        peak = fmax(peak, value);
      }
    }
  }
  if (peak - floor < 20) {
    return finder;
  }
  const threshold = floor + 0.5 * (peak - floor);
  let sw = 0;
  let sx = 0;
  let sy = 0;
  for (let dy = -radius; dy <= radius; dy += 1) {
    const y = cy + dy;
    if (y < 0 || y >= h) continue;
    const py = y + 0.5;
    for (let dx = -radius; dx <= radius; dx += 1) {
      const x = cx + dx;
      if (x < 0 || x >= w) continue;
      const px = x + 0.5;
      if (Math.sqrt((px - finder.x) * (px - finder.x) + (py - finder.y) * (py - finder.y)) <= limit) {
        const weight = fmax(data[y * w + x] - threshold, 0);
        sw += weight;
        sx += weight * px;
        sy += weight * py;
      }
    }
  }
  if (sw <= 0) {
    return finder;
  }
  return { x: sx / sw, y: sy / sw, size: finder.size };
}

/** Finder search shared by `locate` and the decoder; returns four finders or `null`. */
export function locateFinders(image) {
  const prepared = prepareThreshold(image);
  scratch.mask = grow(scratch.mask, image.width * image.height, Uint8Array);
  for (const sensitivity of [0.12, 0.22, 0.34]) {
    const mask = binarizeInto(image, prepared, sensitivity, scratch.mask);
    const table = labelTable(mask, image.width, image.height);
    const quad = selectQuad(blossomsFromTable(table));
    if (quad !== null) {
      return quad.map((finder) => refineCenter(image, finder));
    }
  }
  return null;
}

/**
 * Locates the four finders of a code, trying progressively stricter
 * thresholds so blurred rings still separate from their cores.
 *
 * @returns {Array<{x: number, y: number, size: number}> | null} clockwise from the top-left
 */
export function locate(image) {
  return locateFinders(image);
}
