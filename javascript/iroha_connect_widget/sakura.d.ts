// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/** Resolved color theme. */
export type SakuraTheme = "dark" | "light";

/** Theme preference; `auto` follows the page (`data-theme`, a `dark`/`light` class, then the OS). */
export type SakuraThemePreference = "auto" | SakuraTheme;

/** `background` sits behind page content; `overlay` floats above it (click-through). */
export type SakuraMode = "background" | "overlay";

export const IROHA_SAKURA_TAG: "iroha-sakura";

/** Petal budget shared with the Iroha wallet scene. */
export const SAKURA_LIMITS: Readonly<{
  maxPetals: number;
  minPetals: number;
  lowPowerMaxPetals: number;
  reducedMotionPetals: number;
  areaPerPetal: number;
  lowPowerFrameIntervalMs: number;
  maxBurstPetals: number;
}>;

/** Petal fill per resolved theme. */
export const PETAL_COLORS: Readonly<Record<SakuraTheme, string>>;

/** CSS custom properties that receive the pointer parallax (-0.5 … 0.5). */
export const DEFAULT_PARALLAX_VARIABLES: readonly [string, string];

export interface SakuraPetal {
  x: number;
  y: number;
  size: number;
  speedY: number;
  speedX: number;
  rotation: number;
  rotationSpeed: number;
  sway: number;
  flutterPhase: number;
  stuck: boolean;
  stickSide: "left" | "right" | null;
}

export interface SakuraCountOptions {
  reducedMotion?: boolean;
  lowPower?: boolean;
  /** Multiplier for the petal count, greater than 0 and at most 2. */
  density?: number;
}

/** Create one petal at a random position inside the viewport. */
export function createPetal(width: number, height: number, random?: () => number): SakuraPetal;

/** Petals for a viewport: one per 26,000 px², clamped to the wallet's budget. */
export function targetPetalCount(width: number, height: number, options?: SakuraCountOptions): number;

/** Advance one petal by `frameScale` 60 Hz frames under pointer wind `parallaxX`. */
export function updatePetal(
  petal: SakuraPetal,
  width: number,
  height: number,
  parallaxX: number,
  frameScale: number,
  random?: () => number,
): SakuraPetal;

/** Trace and fill one sakura petal of `size` pixels at the origin. */
export function tracePetalPath(
  context: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
  size: number,
): void;

export interface SakuraFieldOptions {
  theme?: SakuraTheme;
  density?: number;
  random?: () => number;
  /** Two CSS custom property names for the pointer parallax, or null to write none. */
  parallaxVariables?: readonly [string, string] | null;
}

export interface SakuraBurstOptions {
  /** Viewport x; defaults to the center. */
  x?: number;
  /** Viewport y; defaults to the center. */
  y?: number;
  /** Petals to release (0–64); defaults to 24. */
  count?: number;
}

/** Runs the sakura animation on one viewport-sized canvas. */
export class SakuraField {
  constructor(canvas: HTMLCanvasElement, options?: SakuraFieldOptions);
  readonly running: boolean;
  readonly petalCount: number;
  readonly burstCount: number;
  readonly theme: SakuraTheme;
  start(): this;
  destroy(): void;
  setTheme(theme: SakuraTheme): void;
  setDensity(density: number): void;
  /** Release a flurry of petals; returns how many were released. */
  burst(options?: SakuraBurstOptions): number;
}

/**
 * `<iroha-sakura>`: a fixed, click-through sakura layer. Attributes: `theme`,
 * `mode`, `backdrop`, `density`.
 */
export class IrohaSakuraElement extends HTMLElement {
  readonly field: SakuraField | null;
  burst(x?: number, y?: number, count?: number): number;
}

/** Register `<iroha-sakura>` once; returns false outside a browser. */
export function defineIrohaSakura(registry?: CustomElementRegistry): boolean;

/** A share of the page-level sakura layer. */
export interface SakuraLease {
  readonly element: IrohaSakuraElement | null;
  burst(x?: number, y?: number, count?: number): number;
  release(): void;
}

export interface AcquireSakuraOptions {
  mode?: SakuraMode;
  theme?: SakuraThemePreference | null;
  /** Also paint the SORA Nexus illuminated background. */
  backdrop?: boolean;
}

/** Share one page-level `<iroha-sakura>` between every widget on the page. */
export function acquireSakura(options?: AcquireSakuraOptions): SakuraLease;

/** Resolve a theme preference against a document. */
export function resolveTheme(preference?: string | null, doc?: Document): SakuraTheme;

declare global {
  interface HTMLElementTagNameMap {
    "iroha-sakura": IrohaSakuraElement;
  }
}
