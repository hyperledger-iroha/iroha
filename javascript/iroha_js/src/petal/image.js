// Minimal 8-bit pixel buffers.
//
// Pixel (i, j) covers [i, i+1) x [j, j+1) and its centre is at (i + 0.5, j + 0.5).
// All homographies of the decoder map into these pixel-edge coordinates.

import { requireIndex } from "./support.js";

function isByteArray(value) {
  return value instanceof Uint8Array || value instanceof Uint8ClampedArray;
}

function requirePositive(value, name) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new TypeError(`${name} must be a positive integer`);
  }
  return value;
}

/**
 * Bilinear sample of a row-major plane at continuous pixel-edge coordinates,
 * clamped to the image border.
 */
export function sampleLuma(data, width, height, x, y) {
  let fx = x - 0.5;
  fx = fx < 0 ? 0 : fx > width - 1 ? width - 1 : fx;
  let fy = y - 0.5;
  fy = fy < 0 ? 0 : fy > height - 1 ? height - 1 : fy;
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const x1 = x0 + 1 < width - 1 ? x0 + 1 : width - 1;
  const y1 = y0 + 1 < height - 1 ? y0 + 1 : height - 1;
  const tx = fx - x0;
  const ty = fy - y0;
  const row0 = y0 * width;
  const row1 = y1 * width;
  const top = data[row0 + x0] * (1 - tx) + data[row0 + x1] * tx;
  const bottom = data[row1 + x0] * (1 - tx) + data[row1 + x1] * tx;
  return top * (1 - ty) + bottom * ty;
}

/** Rec. 601 luma of one RGB pixel, identical to the reference `Rgb::to_luma`. */
export function rec601Luma(r, g, b) {
  return ((299 * r + 587 * g + 114 * b + 500) / 1000) | 0;
}

/**
 * Converts an RGBA plane into `out` (row-major luma, `width * height` bytes).
 * `stride` is the RGBA row pitch in bytes.
 */
export function rgbaToLumaInto(rgba, width, height, stride, out) {
  let index = 0;
  for (let y = 0; y < height; y += 1) {
    let offset = y * stride;
    for (let x = 0; x < width; x += 1) {
      out[index] = ((299 * rgba[offset] + 587 * rgba[offset + 1] + 114 * rgba[offset + 2] + 500) / 1000) | 0;
      offset += 4;
      index += 1;
    }
  }
  return out;
}

/** A single-channel 8-bit image (camera luma plane). */
export class PetalLuma {
  /**
   * Wraps (without copying) or allocates a row-major luma plane.
   *
   * @param {number} width
   * @param {number} height
   * @param {Uint8Array | Uint8ClampedArray} [data] exactly `width * height` samples; black when omitted
   * @throws {RangeError} when `data` has the wrong length
   */
  constructor(width, height, data) {
    requireIndex(width, "luma width");
    requireIndex(height, "luma height");
    let plane;
    if (data === undefined) {
      plane = new Uint8Array(width * height);
    } else if (isByteArray(data)) {
      plane = data instanceof Uint8Array ? data : new Uint8Array(data.buffer, data.byteOffset, data.length);
      if (plane.length !== width * height) {
        throw new RangeError("luma data length must equal width * height");
      }
    } else {
      throw new TypeError("luma data must be a Uint8Array or Uint8ClampedArray");
    }
    /** Width in pixels. */
    this.width = width;
    /** Height in pixels. */
    this.height = height;
    /** Row-major samples, `width * height` bytes. */
    this.data = plane;
  }

  /**
   * Copies a strided plane (for example the Y plane of an NV12/NV21 or I420
   * camera frame). Returns `null` when the plane is too short or
   * `stride < width`.
   */
  static fromStrided(width, height, stride, plane) {
    requireIndex(width, "luma width");
    requireIndex(height, "luma height");
    requireIndex(stride, "luma stride");
    if (!isByteArray(plane)) {
      throw new TypeError("luma plane must be a Uint8Array or Uint8ClampedArray");
    }
    if (height === 0 || stride < width || plane.length < stride * (height - 1) + width) {
      return null;
    }
    const data = new Uint8Array(width * height);
    for (let row = 0; row < height; row += 1) {
      data.set(plane.subarray(row * stride, row * stride + width), row * width);
    }
    return new PetalLuma(width, height, data);
  }

  /**
   * Converts an RGBA plane (canvas `ImageData` layout) to Rec. 601 luma with
   * the reference weights `(299 R + 587 G + 114 B + 500) / 1000`. `stride` is
   * the row pitch in bytes (default `width * 4`). Returns `null` when the
   * buffer is too short.
   */
  static fromRgba(width, height, rgba, stride = width * 4) {
    requirePositive(width, "image width");
    requirePositive(height, "image height");
    requireIndex(stride, "rgba stride");
    if (!isByteArray(rgba)) {
      throw new TypeError("rgba data must be a Uint8Array or Uint8ClampedArray");
    }
    if (stride < width * 4 || rgba.length < stride * (height - 1) + width * 4) {
      return null;
    }
    const data = rgbaToLumaInto(rgba, width, height, stride, new Uint8Array(width * height));
    return new PetalLuma(width, height, data);
  }

  /** Converts a canvas `ImageData` (or any `{width, height, data}` RGBA image). */
  static fromImageData(imageData) {
    if (imageData === null || typeof imageData !== "object") {
      throw new TypeError("imageData must be an ImageData-like object");
    }
    return PetalLuma.fromRgba(imageData.width, imageData.height, imageData.data);
  }

  /** Reads pixel `(x, y)`. */
  at(x, y) {
    return this.data[y * this.width + x];
  }

  /**
   * Bilinear sample at continuous pixel-edge coordinates, clamped to the
   * image border.
   */
  sample(x, y) {
    return sampleLuma(this.data, this.width, this.height, x, y);
  }
}
