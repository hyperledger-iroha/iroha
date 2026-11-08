// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * SORA Nexus brand primitives shared by every IrohaConnect surface.
 *
 * The mark is the Iroha flower from the wallet logo: the red five-petal
 * silhouette with its full white construction. Colors match the wallet's
 * design tokens so a connected app reads as part of the same product.
 */

/** Iroha red, the fill of the flower mark. */
export const IROHA_RED = "#e4232d";

/** Petal pink used by the sakura layer (dark theme). */
export const SAKURA_PINK = "#fcb5d4";

const FLOWER_PATH =
  "M189,74.86l-16.35-3.56,11-12.85c-9.61-9.54-30.25-12-51.46-5.15a82.91,82.91,0,0,0-8.87,3.45," +
  "82.94,82.94,0,0,0,.53-9.41C123.9,25,115.14,6.19,103.1,0L94.29,14.47,85.86,0C73.84,6.24,65.1," +
  "25.08,65.1,47.35a82.94,82.94,0,0,0,.53,9.41,82.91,82.91,0,0,0-8.87-3.45c-21.21-6.89-41.85-4.39-" +
  "51.46,5.15l11,12.85L0,74.86c2.2,13.35,17.41,27.48,38.6,34.36a83,83,0,0,0,9.06,2.39,83,83,0,0,0-" +
  "5.91,7.25c-13.11,18-17.11,38.44-11,50.53l15.63-6.53L44.7,179.51c13.38,2,31.52-8.07,44.61-26.09a82.88," +
  "82.88,0,0,0,5.19-8.08,82.88,82.88,0,0,0,5.19,8.08c13.09,18,31.23,28.12,44.61,26.09l-1.67-16.64,15.63," +
  "6.53c6.1-12.09,2.1-32.49-11-50.53a83,83,0,0,0-5.91-7.25,83,83,0,0,0,9.06-2.39C171.59,102.34,186.8," +
  "88.21,189,74.86Z";

// Construction of the wallet logo (iroha-demo-javascript `src/assets/iroha_logo.svg`),
// drawn in white over the flower in the source's order.
const LINES = [
  // Outer star.
  [123.3, 58.03, 172.48, 72.55],
  [172.48, 72.55, 141.24, 112.77],
  [141.24, 112.77, 142.52, 163.9],
  [142.52, 163.9, 94.5, 146.41],
  [94.5, 146.41, 46.48, 163.9],
  [46.48, 163.9, 47.76, 112.77],
  [47.76, 112.77, 16.52, 72.55],
  [16.52, 72.55, 65.7, 58.03],
  [65.7, 58.03, 47.76, 112.77],
  [47.76, 112.77, 94.5, 146.41],
  [94.5, 146.41, 141.24, 112.77],
  [141.24, 112.77, 123.3, 58.03],
  [123.3, 58.03, 65.7, 58.03],
  [65.7, 58.06, 94.29, 15.87],
  [123.3, 58.06, 94.29, 15.87],
  // Inner hexagon lattice.
  [94.23, 117.81, 121.96, 115.36],
  [66.5, 115.36, 94.23, 117.81],
  [121.96, 83.26, 94.23, 80.83],
  [66.5, 83.34, 94.23, 80.83],
  [121.96, 83.34, 110.24, 108.56],
  [94.23, 131.37, 110.24, 108.56],
  [66.5, 83.34, 78.22, 108.56],
  [94.23, 131.37, 78.22, 108.56],
  [66.5, 83.34, 78.22, 89.87],
  [110.24, 89.87, 121.96, 83.34],
  [94.23, 117.6, 94.23, 131.37],
  [121.96, 115.36, 110.24, 89.87],
  [94.23, 67.33, 110.24, 89.87],
  [66.5, 115.36, 78.22, 89.87],
  [94.23, 67.33, 78.22, 89.87],
  // Spokes from the lattice to the star.
  [117.34, 129.97, 94.23, 131.37],
  [132.36, 85.67, 121.96, 83.34],
  [66.5, 115.36, 47.76, 112.77],
  [121.96, 115.36, 141.24, 112.77],
  [65.63, 58.01, 94.23, 67.33],
  [123.37, 58.01, 94.23, 67.33],
  [46.48, 163.9, 71.13, 129.95],
  [94.23, 131.37, 71.13, 129.95],
  [142.52, 163.9, 117.34, 129.97],
  [172.48, 72.55, 132.36, 85.67],
  [94.29, 15.87, 94.29, 58.03],
  [16.52, 72.55, 57.13, 85.64],
  [57.13, 85.64, 68.43, 83.33],
];

const POLYGONS = [
  "141.24 112.77 94.5 146.59 47.76 112.77 65.63 58.01 123.37 58.01",
  "121.96 115.36 94.23 131.37 66.5 115.36 66.5 83.34 94.23 67.33 121.96 83.34",
  "110.24 108.56 94.23 80.83 78.22 108.56",
  "78.22 89.87 94.23 117.6 110.24 89.87",
];

const NODES = [
  [94.23, 131.37],
  [66.5, 83.33],
  [78.22, 108.56],
  [110.24, 108.56],
  [94.23, 80.83],
  [121.96, 83.34],
];

/**
 * Inline SVG markup for the Iroha flower mark: the red five-petal flower
 * with the wallet logo's complete white construction (star, hexagon
 * lattice, spokes and nodes).
 *
 * @param {{ title?: string, strokeWidth?: number }} [options] `strokeWidth`
 *   is in the mark's 189-unit viewBox; the source artwork uses 0.94, which
 *   vanishes at icon sizes, so the default is heavier.
 */
export function irohaMarkSvg(options = {}) {
  const strokeWidth = Number.isFinite(options.strokeWidth) ? options.strokeWidth : 1.6;
  const lines = LINES.map(
    ([x1, y1, x2, y2]) => `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`,
  ).join("");
  const polygons = POLYGONS.map((points) => `<polygon points="${points}"/>`).join("");
  const nodes = NODES.map(([cx, cy]) => `<circle cx="${cx}" cy="${cy}" r="1.93"/>`).join("");
  const label = typeof options.title === "string" && options.title
    ? ` role="img" aria-label="${options.title.replace(/[&<>"']/gu, (c) => `&#${c.charCodeAt(0)};`)}"`
    : ` aria-hidden="true" focusable="false"`;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 189 181"${label}>` +
    `<path fill="${IROHA_RED}" d="${FLOWER_PATH}"/>` +
    `<g fill="none" stroke="#ffffff" stroke-width="${strokeWidth}" stroke-linejoin="round" stroke-linecap="round">` +
    `${polygons}${lines}<g fill="#ffffff">${nodes}</g></g></svg>`
  );
}

/** Decorative Iroha flower mark (`aria-hidden`), ready to inline in custom UI. */
export const IROHA_MARK_SVG = irohaMarkSvg();
