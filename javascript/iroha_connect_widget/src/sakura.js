// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * The SORA Nexus sakura layer: pink petals drifting across the viewport.
 *
 * This is the Iroha wallet's background animation (`SakuraScene`) packaged
 * for any page, so every app that offers IrohaConnect shares one look: the
 * same petal silhouette and colors, the same density per screen area, the
 * same pointer wind and parallax, and the same reduced-motion, low-power and
 * hidden-tab behavior. The only addition is `burst()`, a one-off flurry used
 * when a wallet approves a connection.
 */

import { normalizeThemePreference, watchTheme } from "./theme.js";

/** Element name of the page-level sakura layer. */
export const IROHA_SAKURA_TAG = "iroha-sakura";

/** Petal budget, identical to the wallet scene. */
export const SAKURA_LIMITS = Object.freeze({
  maxPetals: 55,
  minPetals: 16,
  lowPowerMaxPetals: 36,
  reducedMotionPetals: 18,
  areaPerPetal: 26_000,
  lowPowerFrameIntervalMs: 1000 / 30,
  maxBurstPetals: 64,
});

/** Petal fill per resolved theme, identical to the wallet scene. */
export const PETAL_COLORS = Object.freeze({
  dark: "rgba(252, 181, 212, 0.9)",
  light: "rgba(247, 166, 198, 0.92)",
});

/** CSS custom properties that receive the pointer parallax (-0.5 … 0.5). */
export const DEFAULT_PARALLAX_VARIABLES = Object.freeze(["--iroha-parallax-x", "--iroha-parallax-y"]);

const SPRITE_BASE_SIZE = 28;
const SPRITE_WIDTH = 48;
const SPRITE_HEIGHT = 80;
const SPRITE_CENTER_X = 24;
const SPRITE_CENTER_Y = 36;
const SPRITE_PIXEL_RATIO = 2;
const BURST_GRAVITY = 0.06;
const BURST_TERMINAL_FALL = 1.6;
const BURST_DRAG = 0.985;
const BURST_FADE = 0.006;

function normalizeDensity(value) {
  const density = typeof value === "string" ? Number.parseFloat(value) : value;
  if (density === undefined || density === null || Number.isNaN(density)) {
    return 1;
  }
  if (!Number.isFinite(density) || density <= 0 || density > 2) {
    throw new RangeError("sakura density must be greater than 0 and at most 2");
  }
  return density;
}

/** Create one petal at a random position inside `width` x `height`. */
export function createPetal(width, height, random = Math.random) {
  return {
    x: random() * width,
    y: random() * height,
    size: 8 + random() * 8,
    speedY: 0.4 + random() * 1.2,
    speedX: -0.5 + random() * 1.0,
    rotation: random() * Math.PI * 2,
    rotationSpeed: -0.01 + random() * 0.02,
    sway: 0.5 + random() * 0.8,
    flutterPhase: random() * Math.PI * 2,
    stuck: false,
    stickSide: null,
  };
}

/**
 * Number of petals for a viewport: one per 26,000 px², clamped to the
 * wallet's budget; `density` scales the result (1 keeps the wallet look).
 */
export function targetPetalCount(width, height, options = {}) {
  const density = normalizeDensity(options.density ?? 1);
  let count;
  if (options.reducedMotion) {
    count = Math.min(SAKURA_LIMITS.reducedMotionPetals, SAKURA_LIMITS.maxPetals);
  } else {
    const maximum = options.lowPower ? SAKURA_LIMITS.lowPowerMaxPetals : SAKURA_LIMITS.maxPetals;
    count = Math.max(
      SAKURA_LIMITS.minPetals,
      Math.min(maximum, Math.round((width * height) / SAKURA_LIMITS.areaPerPetal)),
    );
  }
  return density === 1 ? count : Math.max(1, Math.round(count * density));
}

/**
 * Advance one petal by `frameScale` 60 Hz frames. Strong sideways pointer
 * wind pins petals to the edge it blows toward until the wind calms.
 */
export function updatePetal(petal, width, height, parallaxX, frameScale, random = Math.random) {
  const wind = parallaxX * 1.5;
  if (petal.stuck) {
    petal.y += petal.speedY * 1.4 * frameScale;
    petal.x = petal.stickSide === "left" ? petal.size : width - petal.size;
    if (Math.abs(parallaxX) < 0.12) {
      petal.stuck = false;
      petal.stickSide = null;
    }
  } else {
    petal.y += petal.speedY * frameScale;
    petal.x += (petal.speedX + wind + Math.sin(petal.y / 40) * petal.sway * 0.3) * frameScale;
    if (Math.abs(parallaxX) > 0.2) {
      if (petal.x < petal.size * 1.5 && parallaxX < 0) {
        petal.stuck = true;
        petal.stickSide = "left";
        petal.x = petal.size;
      } else if (petal.x > width - petal.size * 1.5 && parallaxX > 0) {
        petal.stuck = true;
        petal.stickSide = "right";
        petal.x = width - petal.size;
      }
    }
  }
  petal.rotation += petal.rotationSpeed * frameScale;
  if (petal.y > height + petal.size) {
    petal.y = -petal.size;
    petal.x = random() * width;
    petal.stuck = false;
    petal.stickSide = null;
  }
  if (!petal.stuck) {
    if (petal.x > width + petal.size) {
      petal.x = -petal.size;
    } else if (petal.x < -petal.size) {
      petal.x = width + petal.size;
    }
  }
  return petal;
}

/** Trace one sakura petal (notched top, wide shoulders, tapered tip) and fill it. */
export function tracePetalPath(context, size) {
  const width = size * 0.78;
  const height = size * 1.12;
  context.beginPath();
  context.moveTo(0, -height * 0.76);
  context.bezierCurveTo(width * 0.22, -height * 1.02, width * 0.56, -height * 0.92, width * 0.68, -height * 0.54);
  context.bezierCurveTo(width * 0.9, -height * 0.02, width * 0.62, height * 0.46, width * 0.22, height * 0.95);
  context.bezierCurveTo(width * 0.08, height * 1.11, width * 0.03, height * 1.2, 0, height * 1.23);
  context.bezierCurveTo(-width * 0.06, height * 1.16, -width * 0.16, height * 1.05, -width * 0.3, height * 0.9);
  context.bezierCurveTo(-width * 0.74, height * 0.34, -width * 0.95, -height * 0.06, -width * 0.66, -height * 0.58);
  context.bezierCurveTo(-width * 0.56, -height * 0.92, -width * 0.2, -height * 1.02, 0, -height * 0.76);
  context.fill();
}

function detectLowPowerMode(navigatorLike) {
  const saveData = navigatorLike?.connection?.saveData === true;
  const cores = Number(navigatorLike?.hardwareConcurrency);
  return saveData || (Number.isFinite(cores) && cores > 0 && cores <= 4);
}

function paintPetalSprite(spriteContext, color) {
  spriteContext.setTransform(SPRITE_PIXEL_RATIO, 0, 0, SPRITE_PIXEL_RATIO, 0, 0);
  spriteContext.clearRect(0, 0, SPRITE_WIDTH, SPRITE_HEIGHT);
  spriteContext.translate(SPRITE_CENTER_X, SPRITE_CENTER_Y);
  spriteContext.fillStyle = color;
  tracePetalPath(spriteContext, SPRITE_BASE_SIZE);
}

function createPetalSprite(doc, color) {
  const physicalWidth = SPRITE_WIDTH * SPRITE_PIXEL_RATIO;
  const physicalHeight = SPRITE_HEIGHT * SPRITE_PIXEL_RATIO;
  const Offscreen = doc?.defaultView?.OffscreenCanvas;
  if (typeof Offscreen === "function") {
    try {
      const offscreen = new Offscreen(physicalWidth, physicalHeight);
      const spriteContext = offscreen.getContext("2d", { alpha: true });
      if (spriteContext) {
        paintPetalSprite(spriteContext, color);
        const bitmap = offscreen.transferToImageBitmap();
        return { source: bitmap, close: () => bitmap.close(), renderer: "offscreen-bitmap" };
      }
    } catch {
      // Some sandboxed renderers expose OffscreenCanvas but block bitmap
      // transfer; the cached DOM canvas below keeps the same look.
    }
  }
  const spriteCanvas = doc?.createElement?.("canvas");
  if (!spriteCanvas) {
    return null;
  }
  spriteCanvas.width = physicalWidth;
  spriteCanvas.height = physicalHeight;
  let spriteContext = null;
  try {
    spriteContext = spriteCanvas.getContext("2d", { alpha: true });
  } catch {
    spriteContext = null;
  }
  if (!spriteContext) {
    return null;
  }
  paintPetalSprite(spriteContext, color);
  return { source: spriteCanvas, close: () => undefined, renderer: "cached-canvas" };
}

/**
 * Runs the sakura animation on one canvas sized to the viewport.
 *
 * Call `start()` once the canvas is in the document and `destroy()` when it
 * leaves; the field pauses itself while the tab is hidden and draws a still
 * frame when the user prefers reduced motion.
 */
export class SakuraField {
  #canvas;
  #document;
  #window;
  #context = null;
  #sprite = null;
  #petals = [];
  #bursts = [];
  #frameId = 0;
  #pointerFrameId = 0;
  #lastFrameTime = 0;
  #width = 0;
  #height = 0;
  #parallaxX = 0;
  #parallaxY = 0;
  #pendingPointer = null;
  #reducedMotionQuery = null;
  #reducedMotion = false;
  #lowPower = false;
  #theme;
  #density;
  #random;
  #parallaxVariables;
  #running = false;

  /**
   * @param {HTMLCanvasElement} canvas
   * @param {{ theme?: "dark" | "light", density?: number, random?: () => number, parallaxVariables?: readonly [string, string] | null }} [options]
   */
  constructor(canvas, options = {}) {
    if (!canvas || typeof canvas.getContext !== "function") {
      throw new TypeError("SakuraField needs a canvas element");
    }
    this.#canvas = canvas;
    this.#document = canvas.ownerDocument;
    this.#window = this.#document?.defaultView ?? null;
    this.#theme = options.theme === "light" ? "light" : "dark";
    this.#density = normalizeDensity(options.density ?? 1);
    this.#random = typeof options.random === "function" ? options.random : Math.random;
    const variables = options.parallaxVariables === undefined
      ? DEFAULT_PARALLAX_VARIABLES
      : options.parallaxVariables;
    if (variables !== null && (!Array.isArray(variables) || variables.length !== 2)) {
      throw new TypeError("parallaxVariables must be two CSS custom property names or null");
    }
    this.#parallaxVariables = variables;
  }

  /** True between `start()` and `destroy()`. */
  get running() {
    return this.#running;
  }

  /** Number of drifting petals (bursts excluded). */
  get petalCount() {
    return this.#petals.length;
  }

  /** Number of burst petals still in flight. */
  get burstCount() {
    return this.#bursts.length;
  }

  /** Resolved theme used for the petal color. */
  get theme() {
    return this.#theme;
  }

  /** Attach listeners, size the canvas and begin animating. */
  start() {
    if (this.#running) {
      return this;
    }
    const view = this.#window;
    if (!view) {
      throw new Error("SakuraField needs a canvas that belongs to a window");
    }
    this.#running = true;
    try {
      this.#context = this.#canvas.getContext("2d", { alpha: true, desynchronized: true }) ?? null;
    } catch {
      this.#context = null;
    }
    this.#lowPower = detectLowPowerMode(view.navigator);
    this.#reducedMotionQuery = view.matchMedia?.("(prefers-reduced-motion: reduce)") ?? null;
    this.#reducedMotion = this.#reducedMotionQuery?.matches === true;
    this.#refreshSprite();
    this.#resize();
    view.addEventListener("resize", this.#resize);
    view.addEventListener("pointermove", this.#updatePointer, { passive: true });
    this.#document.addEventListener("visibilitychange", this.#handleVisibilityChange);
    this.#reducedMotionQuery?.addEventListener?.("change", this.#handleReducedMotionChange);
    if (this.#reducedMotion) {
      this.#drawFrame(false, 0);
    } else {
      this.#startAnimation();
    }
    return this;
  }

  /** Stop animating, remove listeners and release the petal sprite. */
  destroy() {
    if (!this.#running) {
      return;
    }
    this.#running = false;
    const view = this.#window;
    view.removeEventListener("resize", this.#resize);
    view.removeEventListener("pointermove", this.#updatePointer);
    this.#document.removeEventListener("visibilitychange", this.#handleVisibilityChange);
    this.#reducedMotionQuery?.removeEventListener?.("change", this.#handleReducedMotionChange);
    this.#stopAnimation();
    if (this.#pointerFrameId) {
      view.cancelAnimationFrame(this.#pointerFrameId);
      this.#pointerFrameId = 0;
    }
    this.#sprite?.close();
    this.#sprite = null;
    this.#context?.clearRect(0, 0, this.#width, this.#height);
    this.#bursts = [];
    this.#pendingPointer = null;
  }

  /** Switch the petal color to the given resolved theme. */
  setTheme(theme) {
    const next = theme === "light" ? "light" : "dark";
    if (next === this.#theme) {
      return;
    }
    this.#theme = next;
    if (!this.#running) {
      return;
    }
    this.#refreshSprite();
    if (this.#reducedMotion) {
      this.#drawFrame(false, 0);
    }
  }

  /** Scale the petal count (1 keeps the wallet density). */
  setDensity(density) {
    this.#density = normalizeDensity(density);
    if (this.#running) {
      this.#syncPetalCount(this.#width, this.#height);
    }
  }

  /**
   * Release a flurry of petals from a viewport point. Returns how many were
   * released; nothing is released while motion is reduced or the tab is hidden.
   *
   * @param {{ x?: number, y?: number, count?: number }} [options]
   */
  burst(options = {}) {
    if (!this.#running || this.#reducedMotion || this.#document.hidden) {
      return 0;
    }
    const x = Number.isFinite(options.x) ? options.x : this.#width / 2;
    const y = Number.isFinite(options.y) ? options.y : this.#height / 2;
    const requested = Number.isFinite(options.count) ? Math.round(options.count) : 24;
    const count = Math.max(0, Math.min(SAKURA_LIMITS.maxBurstPetals, requested));
    const random = this.#random;
    for (let i = 0; i < count; i += 1) {
      const angle = -Math.PI / 2 + (random() - 0.5) * Math.PI * 1.4;
      const speed = 2.2 + random() * 3.4;
      this.#bursts.push({
        x,
        y,
        size: 7 + random() * 9,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        rotation: random() * Math.PI * 2,
        rotationSpeed: -0.08 + random() * 0.16,
        sway: 0.5 + random() * 0.8,
        flutterPhase: random() * Math.PI * 2,
        life: 1,
        stuck: false,
      });
    }
    this.#startAnimation();
    return count;
  }

  #petalColor() {
    return PETAL_COLORS[this.#theme];
  }

  #refreshSprite() {
    this.#sprite?.close();
    this.#sprite = createPetalSprite(this.#document, this.#petalColor());
    this.#canvas.dataset.renderer = this.#sprite?.renderer ?? "canvas-path";
  }

  #syncPetalCount(width, height) {
    const count = targetPetalCount(width, height, {
      reducedMotion: this.#reducedMotion,
      lowPower: this.#lowPower,
      density: this.#density,
    });
    if (this.#petals.length > count) {
      this.#petals = this.#petals.slice(0, count);
      return;
    }
    while (this.#petals.length < count) {
      this.#petals.push(createPetal(width, height, this.#random));
    }
  }

  #resize = () => {
    const view = this.#window;
    if (!view) {
      return;
    }
    this.#width = view.innerWidth;
    this.#height = view.innerHeight;
    const devicePixelRatio = view.devicePixelRatio;
    const pixelRatio = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0
      ? Math.min(Math.max(devicePixelRatio, 1), 2)
      : 1;
    this.#canvas.width = Math.round(this.#width * pixelRatio);
    this.#canvas.height = Math.round(this.#height * pixelRatio);
    this.#canvas.style.width = `${this.#width}px`;
    this.#canvas.style.height = `${this.#height}px`;
    if (this.#context) {
      this.#context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      this.#context.imageSmoothingEnabled = true;
    }
    this.#syncPetalCount(this.#width, this.#height);
    for (const petal of this.#petals) {
      petal.x = Math.min(Math.max(petal.x, -petal.size), this.#width + petal.size);
      petal.y = Math.min(Math.max(petal.y, -petal.size), this.#height + petal.size);
    }
    if (this.#reducedMotion) {
      this.#drawFrame(false, 0);
    }
  };

  #drawPetal(petal, alpha = 1) {
    const context = this.#context;
    if (!context) {
      return;
    }
    context.save();
    context.translate(petal.x, petal.y);
    context.rotate(petal.rotation);
    const flutter = Math.sin(petal.rotation * 2.6 + petal.flutterPhase) * (0.05 + petal.sway * 0.03);
    context.scale(1 + flutter, 1 - flutter * 0.42);
    if (alpha < 1) {
      context.globalAlpha = alpha;
    }
    const sprite = this.#sprite;
    if (sprite) {
      const spriteScale = petal.size / SPRITE_BASE_SIZE;
      context.drawImage(
        sprite.source,
        -SPRITE_CENTER_X * spriteScale,
        -SPRITE_CENTER_Y * spriteScale,
        SPRITE_WIDTH * spriteScale,
        SPRITE_HEIGHT * spriteScale,
      );
    } else {
      context.fillStyle = this.#petalColor();
      tracePetalPath(context, petal.size);
    }
    context.restore();
  }

  #updateBurst(petal, frameScale) {
    petal.vy = Math.min(petal.vy + BURST_GRAVITY * frameScale, BURST_TERMINAL_FALL);
    petal.vx *= BURST_DRAG ** frameScale;
    petal.x += (petal.vx + Math.sin(petal.y / 40) * petal.sway * 0.3) * frameScale;
    petal.y += petal.vy * frameScale;
    petal.rotation += petal.rotationSpeed * frameScale;
    petal.life -= BURST_FADE * frameScale;
  }

  #drawFrame(advance, frameScale) {
    const context = this.#context;
    if (!context) {
      return;
    }
    context.clearRect(0, 0, this.#width, this.#height);
    for (const petal of this.#petals) {
      if (advance) {
        updatePetal(petal, this.#width, this.#height, this.#parallaxX, frameScale, this.#random);
      }
      const offsetX = this.#parallaxX * 20 * (petal.stuck ? 0.2 : 1);
      const offsetY = this.#parallaxY * 15 * (petal.stuck ? 0.3 : 1);
      context.save();
      context.translate(offsetX, offsetY);
      this.#drawPetal(petal);
      context.restore();
    }
    if (this.#bursts.length === 0) {
      return;
    }
    const survivors = [];
    for (const petal of this.#bursts) {
      if (advance) {
        this.#updateBurst(petal, frameScale);
      }
      if (petal.life > 0 && petal.y < this.#height + petal.size) {
        survivors.push(petal);
        this.#drawPetal(petal, Math.min(1, petal.life * 1.6));
      }
    }
    this.#bursts = survivors;
  }

  #render = (timestamp) => {
    if (this.#document.hidden || this.#reducedMotion) {
      this.#frameId = 0;
      this.#lastFrameTime = 0;
      return;
    }
    if (
      this.#lowPower &&
      this.#lastFrameTime &&
      timestamp - this.#lastFrameTime + 0.5 < SAKURA_LIMITS.lowPowerFrameIntervalMs
    ) {
      this.#frameId = this.#window.requestAnimationFrame(this.#render);
      return;
    }
    const delta = this.#lastFrameTime ? timestamp - this.#lastFrameTime : 1000 / 60;
    this.#lastFrameTime = timestamp;
    this.#drawFrame(true, Math.min(Math.max(delta / (1000 / 60), 0.25), 3));
    this.#frameId = this.#window.requestAnimationFrame(this.#render);
  };

  #startAnimation() {
    if (!this.#running || this.#frameId || this.#document.hidden || this.#reducedMotion) {
      return;
    }
    if (typeof this.#window.requestAnimationFrame !== "function") {
      return;
    }
    this.#lastFrameTime = 0;
    this.#frameId = this.#window.requestAnimationFrame(this.#render);
  }

  #stopAnimation() {
    if (this.#frameId) {
      this.#window.cancelAnimationFrame(this.#frameId);
      this.#frameId = 0;
    }
    this.#lastFrameTime = 0;
  }

  #writeParallaxVariables() {
    const variables = this.#parallaxVariables;
    const style = this.#document.documentElement?.style;
    if (!variables || !style) {
      return;
    }
    style.setProperty(variables[0], this.#parallaxX.toFixed(3));
    style.setProperty(variables[1], this.#parallaxY.toFixed(3));
  }

  #updatePointer = (event) => {
    if (this.#reducedMotion) {
      return;
    }
    this.#pendingPointer = { x: event.clientX, y: event.clientY };
    if (this.#pointerFrameId || typeof this.#window.requestAnimationFrame !== "function") {
      return;
    }
    this.#pointerFrameId = this.#window.requestAnimationFrame(() => {
      this.#pointerFrameId = 0;
      const pointer = this.#pendingPointer;
      if (!pointer || !this.#running) {
        return;
      }
      this.#parallaxX = pointer.x / Math.max(this.#width, 1) - 0.5;
      this.#parallaxY = pointer.y / Math.max(this.#height, 1) - 0.5;
      this.#pendingPointer = null;
      this.#writeParallaxVariables();
    });
  };

  #handleVisibilityChange = () => {
    if (this.#document.hidden) {
      this.#stopAnimation();
      return;
    }
    if (this.#reducedMotion) {
      this.#drawFrame(false, 0);
      return;
    }
    this.#startAnimation();
  };

  #handleReducedMotionChange = (event) => {
    this.#reducedMotion = event.matches === true;
    this.#parallaxX = 0;
    this.#parallaxY = 0;
    this.#writeParallaxVariables();
    this.#syncPetalCount(this.#width, this.#height);
    if (this.#reducedMotion) {
      this.#bursts = [];
      this.#stopAnimation();
      this.#drawFrame(false, 0);
      return;
    }
    this.#startAnimation();
  };
}

const SAKURA_STYLES = `
:host {
  position: fixed;
  inset: 0;
  display: block;
  pointer-events: none;
  z-index: -1;
  contain: strict;
}
:host([mode="overlay"]) {
  z-index: 2147483000;
}
:host([hidden]) {
  display: none;
}
.layer,
.backdrop,
canvas {
  position: absolute;
  inset: 0;
}
.backdrop {
  display: none;
}
:host([backdrop]) .backdrop {
  display: block;
}
.layer[data-theme="dark"] .backdrop {
  background:
    radial-gradient(circle at -6% -10%, rgba(255, 255, 255, 0.105), transparent 38%),
    radial-gradient(circle at 96% 2%, rgba(255, 75, 75, 0.16), transparent 34%),
    radial-gradient(circle at 52% 116%, rgba(118, 133, 174, 0.1), transparent 45%),
    #05060b;
}
.layer[data-theme="light"] .backdrop {
  background:
    radial-gradient(circle at -6% -10%, rgba(255, 255, 255, 0.98), transparent 39%),
    radial-gradient(circle at 96% 2%, rgba(255, 75, 75, 0.15), transparent 34%),
    radial-gradient(circle at 52% 116%, rgba(222, 214, 218, 0.68), transparent 46%),
    #f4f1ed;
}
canvas {
  display: block;
  transform: translateZ(0);
  backface-visibility: hidden;
}
`;

const HTMLElementBase = globalThis.HTMLElement ?? class {};

/**
 * `<iroha-sakura>`: a fixed, click-through sakura layer.
 *
 * Attributes: `theme` (`auto` | `dark` | `light`), `mode` (`background`,
 * behind page content, or `overlay`, above it), `backdrop` (also paint the
 * SORA Nexus illuminated background) and `density` (0 < n <= 2).
 */
export class IrohaSakuraElement extends HTMLElementBase {
  static get observedAttributes() {
    return ["theme", "density"];
  }

  #layer;
  #canvas;
  #field = null;
  #stopThemeWatch = null;

  constructor() {
    super();
    const shadow = this.attachShadow({ mode: "open" });
    shadow.innerHTML =
      `<style>${SAKURA_STYLES}</style>` +
      `<div class="layer" data-theme="dark"><div class="backdrop" part="backdrop"></div>` +
      `<canvas part="canvas" aria-hidden="true"></canvas></div>`;
    this.#layer = shadow.querySelector(".layer");
    this.#canvas = shadow.querySelector("canvas");
  }

  connectedCallback() {
    if (!this.hasAttribute("aria-hidden")) {
      this.setAttribute("aria-hidden", "true");
    }
    this.#start();
  }

  disconnectedCallback() {
    this.#stop();
  }

  attributeChangedCallback(name, previous, next) {
    if (previous === next || !this.#field) {
      return;
    }
    if (name === "theme") {
      this.#watchTheme();
      return;
    }
    try {
      this.#field.setDensity(next ?? 1);
    } catch {
      // An invalid density attribute keeps the current density.
    }
  }

  /** The running animation, or null while disconnected. */
  get field() {
    return this.#field;
  }

  /** Release a flurry of petals from a viewport point. */
  burst(x, y, count) {
    return this.#field?.burst({ x, y, count }) ?? 0;
  }

  #start() {
    if (this.#field || !this.isConnected) {
      return;
    }
    let density = 1;
    try {
      density = normalizeDensity(this.getAttribute("density") ?? 1);
    } catch {
      density = 1;
    }
    this.#field = new SakuraField(this.#canvas, { density });
    this.#watchTheme();
    this.#field.start();
  }

  #watchTheme() {
    this.#stopThemeWatch?.();
    this.#stopThemeWatch = watchTheme(
      normalizeThemePreference(this.getAttribute("theme")),
      (theme) => {
        this.#layer.dataset.theme = theme;
        this.#field?.setTheme(theme);
      },
      this.ownerDocument,
    );
  }

  #stop() {
    this.#stopThemeWatch?.();
    this.#stopThemeWatch = null;
    this.#field?.destroy();
    this.#field = null;
  }
}

/** Register `<iroha-sakura>` once; returns false outside a browser. */
export function defineIrohaSakura(registry = globalThis.customElements) {
  if (!registry || typeof registry.define !== "function") {
    return false;
  }
  if (!registry.get(IROHA_SAKURA_TAG)) {
    registry.define(IROHA_SAKURA_TAG, IrohaSakuraElement);
  }
  return true;
}

const layerState = new WeakMap();

const NOOP_LEASE = Object.freeze({
  element: null,
  burst: () => 0,
  release: () => undefined,
});

function setFlag(element, name, value, enabled) {
  if (enabled) {
    if (element.getAttribute(name) !== value) {
      element.setAttribute(name, value);
    }
  } else if (element.hasAttribute(name)) {
    element.removeAttribute(name);
  }
}

// An auto-created layer reflects every live lease: it paints the backdrop
// when any widget asks for it, floats above content when any widget asks for
// `overlay`, and follows the theme of the oldest lease.
function applyLeaseOptions(state) {
  if (!state.owned || state.leases.size === 0) {
    return;
  }
  const leases = [...state.leases];
  setFlag(state.element, "backdrop", "", leases.some((lease) => lease.backdrop));
  setFlag(state.element, "mode", "overlay", leases.some((lease) => lease.mode === "overlay"));
  setFlag(state.element, "theme", leases[0].theme, true);
}

/**
 * Share one page-level sakura layer between every widget on the page.
 *
 * The first lease adds `<iroha-sakura>` as the first child of `<body>`
 * (unless the page already declares one, which is reused as declared and
 * never removed); the layer leaves the page when the last lease is released.
 *
 * @param {{ mode?: "background" | "overlay", theme?: string, backdrop?: boolean }} [options]
 */
export function acquireSakura(options = {}) {
  const doc = globalThis.document;
  if (!doc?.body || !defineIrohaSakura()) {
    return NOOP_LEASE;
  }
  let state = layerState.get(doc);
  if (!state || !state.element.isConnected) {
    const declared = doc.querySelector(IROHA_SAKURA_TAG);
    if (declared) {
      state = { element: declared, owned: false, leases: new Set() };
    } else {
      const element = doc.createElement(IROHA_SAKURA_TAG);
      element.setAttribute("data-iroha-auto", "");
      // Host rules such as `body > * { max-width: … }` must not reshape the layer.
      element.style.cssText =
        "position:fixed;inset:0;width:auto;height:auto;max-width:none;max-height:none;" +
        "min-width:0;min-height:0;margin:0;padding:0;border:0;transform:none;";
      state = { element, owned: true, leases: new Set() };
    }
    layerState.set(doc, state);
  }
  const record = Object.freeze({
    mode: options.mode === "overlay" ? "overlay" : "background",
    backdrop: options.backdrop === true,
    theme: normalizeThemePreference(options.theme),
  });
  state.leases.add(record);
  applyLeaseOptions(state);
  if (state.owned && !state.element.isConnected) {
    doc.body.prepend(state.element);
  }
  return Object.freeze({
    element: state.element,
    burst(x, y, count) {
      return state.leases.has(record) ? state.element.burst?.(x, y, count) ?? 0 : 0;
    },
    release() {
      if (!state.leases.delete(record)) {
        return;
      }
      if (state.leases.size > 0) {
        applyLeaseOptions(state);
        return;
      }
      if (state.owned) {
        state.element.remove();
      }
      if (layerState.get(doc) === state) {
        layerState.delete(doc);
      }
    },
  });
}
