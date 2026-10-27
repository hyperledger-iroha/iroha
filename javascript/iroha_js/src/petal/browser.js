// Browser glue: a Canvas 2D frame player and a camera scanner.
//
// Nothing here touches DOM globals at import time; `document`,
// `OffscreenCanvas`, `requestAnimationFrame` and `performance` are looked up
// only when a player or scanner runs, and every one of them can be injected.

import { PetalLuma, rgbaToLumaInto } from "./image.js";
import { CANVAS, CARD_HEIGHT } from "./layout.js";
import { petalDrawList, resolvePalette } from "./render.js";
import { PetalScanSession } from "./session.js";

const TAU = 2 * Math.PI;

function css(color) {
  return `rgb(${color[0]}, ${color[1]}, ${color[2]})`;
}

function addCircle(context, shape) {
  context.moveTo(shape.x + shape.r, shape.y);
  context.arc(shape.x, shape.y, shape.r, 0, TAU);
}

/**
 * Where the outline of a petal leaves its ellipse for the notch, in the
 * petal's own frame (`x` along its axis): `t` is the ellipse parameter of the
 * intersection (`x = a cos t`, `y = b sin t`) and `phi` its angle seen from the
 * notch centre. `null` when the notch does not cut the ellipse's outline.
 */
function notchOutline(shape) {
  const a = shape.halfLength;
  const b = shape.halfWidth;
  const r = shape.notchRadius;
  const d = shape.notchDistance;
  // (a cos t - d)^2 + (b sin t)^2 = r^2 is a quadratic in cos t
  const qa = a * a - b * b;
  const qb = -2 * a * d;
  const qc = d * d + b * b - r * r;
  const roots = [];
  if (Math.abs(qa) < 1e-12) {
    roots.push(-qc / qb);
  } else {
    const discriminant = qb * qb - 4 * qa * qc;
    if (discriminant >= 0) {
      const root = Math.sqrt(discriminant);
      roots.push((-qb - root) / (2 * qa), (-qb + root) / (2 * qa));
    }
  }
  const cos = roots.find((value) => value > -1 && value < 1);
  if (!(r > 0) || cos === undefined) {
    return null;
  }
  const t = Math.acos(cos);
  return { t, phi: Math.atan2(b * Math.sin(t), a * cos - d) };
}

/**
 * Adds one petal to the current path, along its axis `petal.angle` (the
 * outward direction plus its turn): the ellipse the long way round from one
 * edge of the notch to the other, then the notch's inner arc back.
 */
function addPetal(context, petal, shape, outline) {
  const { x, y, angle } = petal;
  const a = shape.halfLength;
  const b = shape.halfWidth;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  if (outline === null) {
    context.moveTo(x + a * cos, y + a * sin);
    context.ellipse(x, y, a, b, angle, 0, TAU);
    return;
  }
  const { t, phi } = outline;
  const ux = a * Math.cos(t);
  const uy = b * Math.sin(t);
  context.moveTo(x + ux * cos - uy * sin, y + ux * sin + uy * cos);
  context.ellipse(x, y, a, b, angle, t, TAU - t);
  const d = shape.notchDistance;
  context.arc(x + d * cos, y + d * sin, shape.notchRadius, angle - phi, angle + phi, true);
  context.closePath();
}

/**
 * Adds a square of side `side` at `(x, y)` with corners rounded to radius `corner` to the current
 * path (clockwise, from the end of its top edge's left corner).
 */
function addRoundedSquare(context, x, y, side, corner) {
  context.moveTo(x + corner, y);
  context.arc(x + side - corner, y + corner, corner, -Math.PI / 2, 0);
  context.arc(x + side - corner, y + side - corner, corner, 0, Math.PI / 2);
  context.arc(x + corner, y + side - corner, corner, Math.PI / 2, Math.PI);
  context.arc(x + corner, y + corner, corner, Math.PI, (3 * Math.PI) / 2);
  context.closePath();
}

/**
 * Draws the katakana tiles of a draw list: the light tiles filled, then the glyph strokes, ink on
 * light tiles and pink on dark ones (round caps and joins; every stroke lies inside its tile).
 */
function drawTiles(context, drawList) {
  const { palette, tile: shape } = drawList;
  context.fillStyle = css(palette.light);
  context.beginPath();
  for (const tile of drawList.kanaTiles) {
    if (tile.light) addRoundedSquare(context, tile.x, tile.y, shape.side, shape.corner);
  }
  context.fill();
  context.lineWidth = shape.strokeWidth;
  context.lineCap = "round";
  context.lineJoin = "round";
  for (const light of [true, false]) {
    context.strokeStyle = css(light ? palette.ink : palette.pink);
    context.beginPath();
    for (const tile of drawList.kanaTiles) {
      if (tile.light !== light) continue;
      for (const stroke of shape.glyphs[tile.glyph]) {
        context.moveTo(tile.x + stroke[0][0], tile.y + stroke[0][1]);
        for (let point = 1; point < stroke.length; point += 1) {
          context.lineTo(tile.x + stroke[point][0], tile.y + stroke[point][1]);
        }
      }
    }
    context.stroke();
  }
}

/**
 * Draws a `petalDrawList` result onto a Canvas 2D context (or any object
 * with the same path API): the whole card, its 1024-unit width scaled to
 * `size` pixels (it is `size * 1668 / 1024` pixels tall) with its top-left
 * corner at `(x, y)`, the square starting 322 units below the card's top
 * edge. Petals are drawn turned (half-length 6 and half-width 3.73 along their
 * axis), with the notch of radius 1.12 at 6.24 along the axis that tells a
 * petal from one turned by half a turn; the katakana tiles above and below the
 * square are rounded squares of side 25 (corner radius 3), light tiles filled,
 * with their glyphs stroked (width 6.5 * 23 / 32, round caps and joins) in ink
 * on light tiles and pink on dark ones. Everything is antialiased (Canvas 2D
 * fills and strokes paths antialiased; keep the card at 768 pixels wide or
 * more so the turns and glyphs show).
 *
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} context
 * @param {ReturnType<typeof petalDrawList>} drawList
 * @param {{x?: number, y?: number, size?: number}} [options] `size` (the card's width) defaults
 *   to the widest card the canvas holds
 */
export function drawPetalFrame(context, drawList, options = {}) {
  const x = options.x ?? 0;
  const y = options.y ?? 0;
  const size = options.size ?? defaultSize(context);
  if (typeof size !== "number" || !(size > 0)) {
    throw new TypeError("draw size must be a positive number");
  }
  const { palette } = drawList;
  const scale = size / drawList.canvas;
  context.save();
  context.translate(x, y);
  context.scale(scale, scale);
  context.fillStyle = css(palette.background);
  context.fillRect(0, 0, drawList.canvas, drawList.cardHeight);
  // square coordinates from here on: the bands lie above and below the square
  context.translate(0, drawList.squareTop);
  // the faint 天 under the petals, clipped to the backdrop disc
  const { backdrop } = drawList;
  context.save();
  context.beginPath();
  addCircle(context, backdrop);
  context.clip();
  context.fillStyle = css(palette.backdrop);
  context.beginPath();
  for (const cell of backdrop.cells) context.rect(cell.x, cell.y, cell.size, cell.size);
  context.fill();
  context.restore();
  // lit petals: the light ones inside the 天, then the pink ones
  const shape = drawList.petal;
  const outline = notchOutline(shape);
  for (const light of [true, false]) {
    context.fillStyle = css(light ? palette.light : palette.pink);
    context.beginPath();
    for (const petal of drawList.petals) {
      if (petal.light === light) addPetal(context, petal, shape, outline);
    }
    context.fill();
  }
  // lit ring dots
  context.fillStyle = css(palette.pink);
  context.beginPath();
  for (const dot of drawList.dots) addCircle(context, dot);
  context.fill();
  // finders: core and petals in the light colour, then the petal notches
  context.fillStyle = css(palette.light);
  context.beginPath();
  for (const finder of drawList.finders) {
    addCircle(context, finder.core);
    for (const petal of finder.petals) addCircle(context, petal);
  }
  context.fill();
  context.fillStyle = css(palette.background);
  context.beginPath();
  for (const finder of drawList.finders) {
    for (const notch of finder.notches) addCircle(context, notch);
  }
  context.fill();
  // the katakana bands above and below the square
  drawTiles(context, drawList);
  context.restore();
}

/** The width of the widest card a context's canvas holds (1024 without a canvas). */
function defaultSize(context) {
  const canvas = context && context.canvas;
  if (canvas && typeof canvas.width === "number" && typeof canvas.height === "number") {
    return Math.min(canvas.width, (canvas.height * CANVAS) / CARD_HEIGHT);
  }
  return CANVAS;
}

/**
 * Frame shown `elapsedMs` after `startFrame` was displayed at `fps` frames
 * per second (the counter wraps at 65536).
 */
export function petalPlayerFrame(startFrame, elapsedMs, fps) {
  if (!Number.isInteger(startFrame) || startFrame < 0) {
    throw new TypeError("startFrame must be a non-negative integer");
  }
  if (typeof fps !== "number" || !(fps > 0) || !Number.isFinite(fps)) {
    throw new TypeError("fps must be a positive number");
  }
  const elapsed = typeof elapsedMs === "number" && elapsedMs > 0 ? elapsedMs : 0;
  return (startFrame + Math.floor((elapsed * fps) / 1000)) % 65536;
}

/**
 * Size of the downscaled analysis image: the camera frame scaled so its long
 * side is at most `maxSide` pixels (never upscaled).
 *
 * @returns {{width: number, height: number}}
 */
export function petalScanSize(width, height, maxSide = 1280) {
  if (!Number.isInteger(width) || width <= 0 || !Number.isInteger(height) || height <= 0) {
    throw new TypeError("frame width and height must be positive integers");
  }
  if (!Number.isInteger(maxSide) || maxSide <= 0) {
    throw new TypeError("maxSide must be a positive integer");
  }
  const longSide = Math.max(width, height);
  if (longSide <= maxSide) {
    return { width, height };
  }
  const scale = maxSide / longSide;
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

function defaultNow() {
  const performance = globalThis.performance;
  return performance && typeof performance.now === "function" ? performance.now() : Date.now();
}

function frameScheduler(requestFrame, cancelFrame) {
  const request = requestFrame ?? globalThis.requestAnimationFrame;
  const cancel = cancelFrame ?? globalThis.cancelAnimationFrame;
  if (typeof request !== "function") {
    throw new TypeError("requestAnimationFrame is not available; pass one in the options");
  }
  return {
    request: (callback) => request.call(globalThis, callback),
    cancel: (handle) => {
      if (typeof cancel === "function") cancel.call(globalThis, handle);
    },
  };
}

/**
 * Plays a Petal stream on a Canvas 2D context, advancing frames at `fps`
 * with `requestAnimationFrame`, by default 12 frames per second, of which a
 * 30 fps camera reads practically every frame; a frame must stay up for at
 * least two camera frames, so keep `fps` at or below half the camera frame
 * rate. Every frame is the whole card, the square with its katakana bands
 * (`size` is the card's width; by default the widest card the canvas holds):
 * on a phone, let the card's width fill the screen's width.
 */
export class PetalStreamPlayer {
  /**
   * @param {{encoder: {cells(frame: number): import("./lanes.js").PetalFrameCells},
   *   context: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
   *   fps?: number, size?: number, x?: number, y?: number, palette?: object,
   *   startFrame?: number, onFrame?: (frame: number) => void,
   *   requestAnimationFrame?: (callback: (time: number) => void) => unknown,
   *   cancelAnimationFrame?: (handle: unknown) => void}} options
   */
  constructor(options) {
    if (options === null || typeof options !== "object") {
      throw new TypeError("player options must be an object");
    }
    const { encoder, context } = options;
    if (encoder === null || typeof encoder !== "object" || typeof encoder.cells !== "function") {
      throw new TypeError("player encoder must provide cells(frame)");
    }
    if (context === null || typeof context !== "object") {
      throw new TypeError("player context must be a Canvas 2D context");
    }
    this._encoder = encoder;
    this._context = context;
    this._fps = options.fps ?? 12;
    petalPlayerFrame(0, 0, this._fps);
    this._size = options.size;
    this._x = options.x ?? 0;
    this._y = options.y ?? 0;
    this._palette = resolvePalette(options.palette);
    this._frame = petalPlayerFrame(options.startFrame ?? 0, 0, this._fps);
    this._drawn = false;
    this._onFrame = options.onFrame;
    this._requestFrame = options.requestAnimationFrame;
    this._cancelFrame = options.cancelAnimationFrame;
    this._scheduler = null;
    this._handle = null;
    this._playing = false;
    this._base = this._frame;
    this._origin = null;
  }

  /** The frame number currently on screen. */
  get frame() {
    return this._frame;
  }

  /** Whether the player is animating. */
  get playing() {
    return this._playing;
  }

  /** Draws `frame` immediately. */
  drawFrame(frame) {
    const number = petalPlayerFrame(frame, 0, this._fps);
    const drawList = petalDrawList(this._encoder.cells(number), { palette: this._palette });
    drawPetalFrame(this._context, drawList, { x: this._x, y: this._y, size: this._size });
    this._frame = number;
    this._drawn = true;
    if (typeof this._onFrame === "function") {
      this._onFrame(number);
    }
  }

  /** Starts (or resumes) the animation from the frame on screen. */
  start() {
    if (this._playing) {
      return;
    }
    this._scheduler = frameScheduler(this._requestFrame, this._cancelFrame);
    this._playing = true;
    this._base = this._frame;
    this._origin = null;
    this._handle = this._scheduler.request((time) => this._tick(time));
  }

  /** Stops the animation; the last frame stays on screen. */
  stop() {
    this._playing = false;
    if (this._handle !== null && this._scheduler !== null) {
      this._scheduler.cancel(this._handle);
    }
    this._handle = null;
  }

  _tick(time) {
    if (!this._playing) {
      return;
    }
    const now = typeof time === "number" ? time : defaultNow();
    if (this._origin === null) {
      this._origin = now;
    }
    const frame = petalPlayerFrame(this._base, now - this._origin, this._fps);
    if (!this._drawn || frame !== this._frame) {
      this.drawFrame(frame);
    }
    if (this._playing) {
      this._handle = this._scheduler.request((next) => this._tick(next));
    }
  }
}

function defaultCanvas(width, height) {
  if (typeof OffscreenCanvas === "function") {
    return new OffscreenCanvas(width, height);
  }
  if (typeof document === "object" && document !== null && typeof document.createElement === "function") {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    return canvas;
  }
  throw new TypeError("no canvas implementation is available; pass createCanvas in the options");
}

/**
 * Reads Petal frames from a camera.
 *
 * Takes a `MediaStream` or an `HTMLVideoElement`, grabs frames with
 * `requestVideoFrameCallback` (falling back to `requestAnimationFrame`),
 * downsizes each to at most `maxSide` pixels on the long side on an offscreen
 * canvas, converts it to Rec. 601 luma and feeds a {@link PetalScanSession},
 * which follows the code from frame to frame once it decoded and reads it
 * with one corner blossom hidden. `onProgress` receives each frame's outcome
 * and the session's counters: `stats.tracked` counts frames read by tracking,
 * and `stats.inferred` grows while a corner blossom is hidden (a hint such as
 * "one corner blossom is hidden" helps the user uncover it). A phone held
 * upright sees the card's katakana bands as well, which the session reads
 * (`outcome.kana`, `stats.kanaLanes`).
 *
 * The scanner does not change the camera's settings; set the stream up like
 * this (evidence: `specs/petal_stream.md` section 8):
 *
 * - Resolution: ask `getUserMedia` for about 1280x720
 *   (`video: {facingMode: "environment", width: {ideal: 1280}, height: {ideal: 720}}`).
 *   The default `maxSide` of 1280 already matches it. Only drop to 640 when
 *   the device cannot sustain about 5 decoded frames per second, because at
 *   480p a little defocus already costs the petal field (lane `D` carries on).
 * - Exposure: automatic exposure over-exposes a mostly black screen. Where the
 *   browser supports it, apply an exposure compensation of about -1 EV to the
 *   video track: feature-detect `track.getCapabilities().exposureCompensation`,
 *   then call `track.applyConstraints({advanced: [{exposureCompensation: -1}]})`
 *   (Chrome on Android exposes it). The decoder tolerates the over-exposure
 *   that is left, but it should not be asked to.
 */
export class PetalCameraScanner {
  /**
   * @param {{video?: HTMLVideoElement, stream?: MediaStream, session?: PetalScanSession,
   *   limits?: object, maxSide?: number, stopOnComplete?: boolean,
   *   onProgress?: (outcome: object, stats: object) => void, onComplete?: (completed: object) => void,
   *   onError?: (error: unknown) => void,
   *   createCanvas?: (width: number, height: number) => {width: number, height: number, getContext: Function},
   *   requestAnimationFrame?: (callback: (time: number) => void) => unknown,
   *   cancelAnimationFrame?: (handle: unknown) => void, now?: () => number}} options
   */
  constructor(options) {
    if (options === null || typeof options !== "object") {
      throw new TypeError("scanner options must be an object");
    }
    if (options.video === undefined && options.stream === undefined) {
      throw new TypeError("scanner needs a video element or a media stream");
    }
    this._video = options.video ?? null;
    this._stream = options.stream ?? null;
    this._ownsVideo = false;
    this._session = options.session ?? new PetalScanSession(options.limits);
    if (!(this._session instanceof PetalScanSession)) {
      throw new TypeError("scanner session must be a PetalScanSession");
    }
    this._maxSide = options.maxSide ?? 1280;
    petalScanSize(1, 1, this._maxSide);
    this._stopOnComplete = options.stopOnComplete ?? true;
    this._onProgress = options.onProgress;
    this._onComplete = options.onComplete;
    this._onError = options.onError;
    this._createCanvas = options.createCanvas ?? defaultCanvas;
    this._requestFrame = options.requestAnimationFrame;
    this._cancelFrame = options.cancelAnimationFrame;
    this._now = options.now ?? defaultNow;
    this._canvas = null;
    this._context = null;
    this._luma = null;
    this._scanning = false;
    this._starting = null;
    this._generation = 0;
    this._pending = null;
  }

  /** The scan session receiving the frames. */
  get session() {
    return this._session;
  }

  /** The video element frames are read from (created from the stream on `start`). */
  get video() {
    return this._video;
  }

  /** Whether the scanner is reading frames. */
  get scanning() {
    return this._scanning;
  }

  /** Attaches the stream (when given), starts playback and begins scanning. */
  start() {
    if (this._scanning) {
      return Promise.resolve();
    }
    if (this._starting !== null) {
      return this._starting;
    }
    const generation = this._generation;
    const starting = this._attach().then(
      () => {
        if (this._starting === starting) this._starting = null;
        // a stop() while playback was starting wins
        if (generation === this._generation && !this._scanning) {
          this._scanning = true;
          this._schedule();
        }
      },
      (error) => {
        if (this._starting === starting) this._starting = null;
        throw error;
      },
    );
    this._starting = starting;
    return starting;
  }

  async _attach() {
    if (this._video === null) {
      if (typeof document !== "object" || document === null) {
        throw new TypeError("no document is available to create a video element; pass video in the options");
      }
      const video = document.createElement("video");
      video.muted = true;
      video.playsInline = true;
      video.setAttribute("playsinline", "");
      this._video = video;
      this._ownsVideo = true;
    }
    if (this._stream !== null && this._video.srcObject !== this._stream) {
      this._video.srcObject = this._stream;
    }
    const attached = this._ownsVideo || this._stream !== null;
    if (attached && this._video.paused !== false && typeof this._video.play === "function") {
      await this._video.play();
    }
  }

  /** Stops scanning; a video element created by the scanner is released. */
  stop() {
    this._generation += 1;
    this._starting = null;
    this._scanning = false;
    const pending = this._pending;
    this._pending = null;
    if (pending !== null) {
      if (pending.video && typeof this._video.cancelVideoFrameCallback === "function") {
        this._video.cancelVideoFrameCallback(pending.handle);
      } else if (!pending.video) {
        pending.scheduler.cancel(pending.handle);
      }
    }
    if (this._ownsVideo && this._video !== null) {
      if (typeof this._video.pause === "function") this._video.pause();
      this._video.srcObject = null;
    }
  }

  _schedule() {
    if (!this._scanning) {
      return;
    }
    const video = this._video;
    if (typeof video.requestVideoFrameCallback === "function") {
      const handle = video.requestVideoFrameCallback(() => this._onVideoFrame());
      this._pending = { video: true, handle };
    } else {
      const scheduler = frameScheduler(this._requestFrame, this._cancelFrame);
      const handle = scheduler.request(() => this._onVideoFrame());
      this._pending = { video: false, handle, scheduler };
    }
  }

  _onVideoFrame() {
    this._pending = null;
    if (!this._scanning) {
      return;
    }
    try {
      this.scanFrame();
    } catch (error) {
      if (typeof this._onError === "function") {
        this._onError(error);
      } else {
        this.stop();
        throw error;
      }
    }
    this._schedule();
  }

  /**
   * Grabs the current video frame and offers it to the session. Returns the
   * session outcome, or `null` while the video has no frame yet.
   */
  scanFrame() {
    const video = this._video;
    if (video === null) {
      return null;
    }
    const sourceWidth = video.videoWidth;
    const sourceHeight = video.videoHeight;
    if (!Number.isInteger(sourceWidth) || !Number.isInteger(sourceHeight) || sourceWidth <= 0 || sourceHeight <= 0) {
      return null;
    }
    const { width, height } = petalScanSize(sourceWidth, sourceHeight, this._maxSide);
    const context = this._contextFor(width, height);
    context.drawImage(video, 0, 0, width, height);
    const pixels = context.getImageData(0, 0, width, height);
    if (this._luma === null || this._luma.width !== width || this._luma.height !== height) {
      this._luma = new PetalLuma(width, height);
    }
    rgbaToLumaInto(pixels.data, width, height, width * 4, this._luma.data);
    const outcome = this._session.push(this._luma, this._now());
    if (typeof this._onProgress === "function") {
      this._onProgress(outcome, this._session.stats());
    }
    if (outcome.completed !== null) {
      if (this._stopOnComplete) {
        this.stop();
      }
      if (typeof this._onComplete === "function") {
        this._onComplete(outcome.completed);
      }
    }
    return outcome;
  }

  _contextFor(width, height) {
    if (this._canvas === null) {
      this._canvas = this._createCanvas(width, height);
    }
    const canvas = this._canvas;
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      this._context = null;
    }
    if (this._context === null) {
      this._context = canvas.getContext("2d", { willReadFrequently: true });
      if (this._context === null) {
        throw new TypeError("canvas does not provide a 2D context");
      }
    }
    return this._context;
  }
}
