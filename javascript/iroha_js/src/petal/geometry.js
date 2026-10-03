// Plane homographies (row-major 3x3, IEEE double precision, same operation
// order as the reference so fitted poses are bit-identical).

/** Applies a row-major 3x3 homography `m` to `(x, y)`. */
export function applyHomography(m, x, y) {
  const w = m[6] * x + m[7] * y + m[8];
  return [(m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w];
}

/** `a * b` (apply `b` first). */
function compose(a, b) {
  const out = new Float64Array(9);
  for (let r = 0; r < 3; r += 1) {
    for (let c = 0; c < 3; c += 1) {
      out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
    }
  }
  return out;
}

function inverse(m) {
  const c00 = m[4] * m[8] - m[5] * m[7];
  const c01 = m[5] * m[6] - m[3] * m[8];
  const c02 = m[3] * m[7] - m[4] * m[6];
  const det = m[0] * c00 + m[1] * c01 + m[2] * c02;
  if (Math.abs(det) < 1e-18) {
    return null;
  }
  const inv = 1 / det;
  return Float64Array.of(
    c00 * inv,
    (m[2] * m[7] - m[1] * m[8]) * inv,
    (m[1] * m[5] - m[2] * m[4]) * inv,
    c01 * inv,
    (m[0] * m[8] - m[2] * m[6]) * inv,
    (m[2] * m[3] - m[0] * m[5]) * inv,
    c02 * inv,
    (m[1] * m[6] - m[0] * m[7]) * inv,
    (m[0] * m[4] - m[1] * m[3]) * inv,
  );
}

/** Translation to the centroid and isotropic scale to mean distance sqrt(2). */
function normalisation(xs, ys) {
  const n = xs.length;
  let cx = 0;
  let cy = 0;
  for (let index = 0; index < n; index += 1) {
    cx += xs[index] / n;
    cy += ys[index] / n;
  }
  let total = -0;
  for (let index = 0; index < n; index += 1) {
    const dx = xs[index] - cx;
    const dy = ys[index] - cy;
    total += Math.sqrt(dx * dx + dy * dy);
  }
  const mean = total / n;
  const scale = mean > 1e-12 ? Math.SQRT2 / mean : 1;
  return { scale, tx: -scale * cx, ty: -scale * cy };
}

/** Gaussian elimination with partial pivoting for an 8x8 system (row-major `a`). */
function solve8(a, b) {
  for (let col = 0; col < 8; col += 1) {
    // Largest magnitude wins; on ties the last row wins (Rust `max_by`), and
    // NaN ranks above every number (`total_cmp`).
    let pivot = col;
    let best = Math.abs(a[col * 8 + col]);
    for (let row = col + 1; row < 8; row += 1) {
      const value = Math.abs(a[row * 8 + col]);
      if (value >= best || value !== value) {
        if (best !== best && value === value) continue;
        pivot = row;
        best = value;
      }
    }
    if (Math.abs(a[pivot * 8 + col]) < 1e-14) {
      return null;
    }
    if (pivot !== col) {
      for (let k = 0; k < 8; k += 1) {
        const swap = a[col * 8 + k];
        a[col * 8 + k] = a[pivot * 8 + k];
        a[pivot * 8 + k] = swap;
      }
      const swap = b[col];
      b[col] = b[pivot];
      b[pivot] = swap;
    }
    for (let row = col + 1; row < 8; row += 1) {
      const factor = a[row * 8 + col] / a[col * 8 + col];
      for (let k = col; k < 8; k += 1) {
        a[row * 8 + k] -= factor * a[col * 8 + k];
      }
      b[row] -= factor * b[col];
    }
  }
  const x = new Float64Array(8);
  for (let row = 7; row >= 0; row -= 1) {
    let tail = -0;
    for (let k = row + 1; k < 8; k += 1) {
      tail += a[row * 8 + k] * x[k];
    }
    x[row] = (b[row] - tail) / a[row * 8 + row];
  }
  return x;
}

/**
 * Fits the homography taking `src[i]` to `dst[i]` (least squares for more
 * than four pairs, exact for four) with the Hartley-normalised DLT. Points are
 * given as flat coordinate arrays. Returns `null` for degenerate input.
 */
export function homographyFromPoints(srcX, srcY, dstX, dstY) {
  const n = srcX.length;
  if (n !== dstX.length || n < 4) {
    return null;
  }
  const ns = normalisation(srcX, srcY);
  const nd = normalisation(dstX, dstY);
  const ata = new Float64Array(64);
  const atb = new Float64Array(8);
  const row = new Float64Array(8);
  for (let index = 0; index < n; index += 1) {
    const x = ns.scale * srcX[index] + ns.tx;
    const y = ns.scale * srcY[index] + ns.ty;
    const u = nd.scale * dstX[index] + nd.tx;
    const v = nd.scale * dstY[index] + nd.ty;
    for (let pass = 0; pass < 2; pass += 1) {
      let rhs;
      if (pass === 0) {
        row[0] = x;
        row[1] = y;
        row[2] = 1;
        row[3] = 0;
        row[4] = 0;
        row[5] = 0;
        row[6] = -u * x;
        row[7] = -u * y;
        rhs = u;
      } else {
        row[0] = 0;
        row[1] = 0;
        row[2] = 0;
        row[3] = x;
        row[4] = y;
        row[5] = 1;
        row[6] = -v * x;
        row[7] = -v * y;
        rhs = v;
      }
      for (let i = 0; i < 8; i += 1) {
        for (let j = 0; j < 8; j += 1) {
          ata[i * 8 + j] += row[i] * row[j];
        }
        atb[i] += row[i] * rhs;
      }
    }
  }
  const h = solve8(ata, atb);
  if (h === null) {
    return null;
  }
  const normalised = Float64Array.of(h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1);
  // H = Td^-1 * Hn * Ts
  const tdInverse = Float64Array.of(
    1 / nd.scale,
    0,
    -nd.tx / nd.scale,
    0,
    1 / nd.scale,
    -nd.ty / nd.scale,
    0,
    0,
    1,
  );
  const tsMatrix = Float64Array.of(ns.scale, 0, ns.tx, 0, ns.scale, ns.ty, 0, 0, 1);
  const result = compose(compose(tdInverse, normalised), tsMatrix);
  const norm = result[8];
  if (Math.abs(norm) < 1e-15) {
    return null;
  }
  for (let index = 0; index < 9; index += 1) {
    result[index] /= norm;
  }
  return result;
}

function readPoints(points, name) {
  if (!Array.isArray(points)) {
    throw new TypeError(`${name} must be an array of [x, y] points`);
  }
  const xs = new Float64Array(points.length);
  const ys = new Float64Array(points.length);
  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    if (!Array.isArray(point) || point.length !== 2 || typeof point[0] !== "number" || typeof point[1] !== "number") {
      throw new TypeError(`${name} must be an array of [x, y] points`);
    }
    xs[index] = point[0];
    ys[index] = point[1];
  }
  return [xs, ys];
}

/** A 3x3 projective transform stored row-major. */
export class PetalHomography {
  /** @param {ArrayLike<number>} values nine row-major coefficients */
  constructor(values) {
    if (values === null || typeof values !== "object" || values.length !== 9) {
      throw new TypeError("a homography has exactly nine coefficients");
    }
    const m = new Float64Array(9);
    for (let index = 0; index < 9; index += 1) {
      if (typeof values[index] !== "number") {
        throw new TypeError("homography coefficients must be numbers");
      }
      m[index] = values[index];
    }
    this._m = m;
  }

  /** The identity transform. */
  static identity() {
    return new PetalHomography([1, 0, 0, 0, 1, 0, 0, 0, 1]);
  }

  /**
   * Fits the homography taking `src[i]` to `dst[i]` (least squares for more
   * than four pairs, exact for four), using the Hartley-normalised DLT.
   * Returns `null` for mismatched, too few or degenerate points.
   *
   * @param {Array<[number, number]>} src
   * @param {Array<[number, number]>} dst
   */
  static fromPoints(src, dst) {
    const [srcX, srcY] = readPoints(src, "src");
    const [dstX, dstY] = readPoints(dst, "dst");
    const m = homographyFromPoints(srcX, srcY, dstX, dstY);
    return m === null ? null : new PetalHomography(m);
  }

  /** The nine row-major coefficients. */
  get values() {
    return Array.from(this._m);
  }

  /**
   * Maps a point.
   *
   * @returns {[number, number]}
   */
  apply(x, y) {
    return applyHomography(this._m, x, y);
  }

  /** The inverse transform, or `null` when singular. */
  inverse() {
    const m = inverse(this._m);
    return m === null ? null : new PetalHomography(m);
  }

  /** `this * other` (apply `other` first). */
  compose(other) {
    if (!(other instanceof PetalHomography)) {
      throw new TypeError("compose expects a PetalHomography");
    }
    return new PetalHomography(compose(this._m, other._m));
  }
}
