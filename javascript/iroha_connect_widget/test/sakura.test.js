// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { installDom, settle, waitFor } from "./helpers/dom.js";

const window = installDom({ width: 1280, height: 800 });
const {
  IROHA_SAKURA_TAG,
  PETAL_COLORS,
  SAKURA_LIMITS,
  SakuraField,
  acquireSakura,
  createPetal,
  targetPetalCount,
  tracePetalPath,
  updatePetal,
} = await import("../src/sakura-element.js");
const { resolveTheme } = await import("../src/theme.js");

function stubMatchMedia(matches) {
  window.matchMedia = (query) => ({
    matches: matches(query),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  });
}

test("petal budget matches the wallet scene", () => {
  assert.equal(SAKURA_LIMITS.maxPetals, 55);
  assert.equal(targetPetalCount(1280, 800), 39);
  assert.equal(targetPetalCount(320, 480), 16);
  assert.equal(targetPetalCount(3840, 2160), 55);
  assert.equal(targetPetalCount(3840, 2160, { lowPower: true }), 36);
  assert.equal(targetPetalCount(3840, 2160, { reducedMotion: true }), 18);
  assert.equal(targetPetalCount(1280, 800, { density: 0.5 }), 20);
  assert.equal(targetPetalCount(1280, 800, { density: 2 }), 78);
  assert.throws(() => targetPetalCount(1280, 800, { density: 0 }), RangeError);
  assert.throws(() => targetPetalCount(1280, 800, { density: 3 }), RangeError);
});

test("petals are created inside the wallet's ranges", () => {
  const low = createPetal(1000, 500, () => 0);
  assert.deepEqual(
    [low.x, low.y, low.size, low.speedY, low.speedX, low.rotationSpeed, low.sway, low.stuck, low.stickSide],
    [0, 0, 8, 0.4, -0.5, -0.01, 0.5, false, null],
  );
  const high = createPetal(1000, 500, () => 0.999999);
  assert.ok(high.x < 1000 && high.y < 500 && high.size < 16 && high.speedY < 1.6 && high.speedX < 0.5);
});

test("petals fall, wrap and stick to the edge the pointer wind blows toward", () => {
  const petal = { ...createPetal(800, 600, () => 0.5), x: 400, y: 100 };
  const fallBy = petal.speedY * 2;
  updatePetal(petal, 800, 600, 0, 2);
  assert.ok(Math.abs(petal.y - (100 + fallBy)) < 1e-9);

  petal.y = 600 + petal.size + 1;
  updatePetal(petal, 800, 600, 0, 1, () => 0.25);
  assert.equal(petal.y, -petal.size);
  assert.equal(petal.x, 200);

  const windy = { ...createPetal(800, 600, () => 0.5), x: 10, y: 50 };
  updatePetal(windy, 800, 600, -0.4, 1);
  assert.equal(windy.stuck, true);
  assert.equal(windy.stickSide, "left");
  assert.equal(windy.x, windy.size);
  updatePetal(windy, 800, 600, 0.05, 1);
  assert.equal(windy.stuck, false);

  const drifting = { ...createPetal(800, 600, () => 0.5), x: 800 + 30, y: 50, size: 10 };
  updatePetal(drifting, 800, 600, 0, 1);
  assert.equal(drifting.x, -10);
});

test("the petal path is one closed six-curve silhouette", () => {
  const calls = [];
  const recorder = new Proxy({}, { get: (_target, name) => (...args) => calls.push([name, args.length]) });
  tracePetalPath(recorder, 28);
  assert.deepEqual(calls.map(([name]) => name), [
    "beginPath",
    "moveTo",
    "bezierCurveTo",
    "bezierCurveTo",
    "bezierCurveTo",
    "bezierCurveTo",
    "bezierCurveTo",
    "bezierCurveTo",
    "fill",
  ]);
});

test("a field sizes its canvas to the viewport and stops cleanly", async () => {
  window.devicePixelRatio = 3;
  const canvas = window.document.createElement("canvas");
  window.document.body.append(canvas);
  const field = new SakuraField(canvas, { theme: "light" }).start();
  assert.equal(field.running, true);
  assert.equal(field.theme, "light");
  assert.equal(canvas.width, 2560, "device pixel ratio is clamped to 2");
  assert.equal(canvas.height, 1600);
  assert.equal(canvas.style.width, "1280px");
  assert.equal(field.petalCount, targetPetalCount(1280, 800, { lowPower: (window.navigator.hardwareConcurrency ?? 8) <= 4 }));

  window.dispatchEvent(new window.PointerEvent("pointermove", { clientX: 1280, clientY: 0 }));
  await waitFor(() => window.document.documentElement.style.getPropertyValue("--iroha-parallax-x") === "0.500");
  assert.equal(window.document.documentElement.style.getPropertyValue("--iroha-parallax-y"), "-0.500");

  assert.equal(field.burst({ x: 10, y: 10, count: 500 }), SAKURA_LIMITS.maxBurstPetals);
  assert.equal(field.burstCount, SAKURA_LIMITS.maxBurstPetals);

  window.innerWidth = 400;
  window.dispatchEvent(new window.Event("resize"));
  assert.equal(canvas.style.width, "400px");

  field.destroy();
  assert.equal(field.running, false);
  assert.equal(field.burst(), 0);
  window.document.documentElement.style.removeProperty("--iroha-parallax-x");
  window.dispatchEvent(new window.PointerEvent("pointermove", { clientX: 0, clientY: 0 }));
  await settle(40);
  assert.equal(window.document.documentElement.style.getPropertyValue("--iroha-parallax-x"), "");
  window.innerWidth = 1280;
  canvas.remove();
});

test("reduced motion keeps a still field and releases no bursts", () => {
  stubMatchMedia((query) => query.includes("reduced-motion"));
  try {
    const canvas = window.document.createElement("canvas");
    const field = new SakuraField(canvas).start();
    assert.equal(field.petalCount, SAKURA_LIMITS.reducedMotionPetals);
    assert.equal(field.burst({ count: 10 }), 0);
    field.destroy();
  } finally {
    delete window.matchMedia;
  }
});

test("every widget shares one page-level layer that leaves with the last lease", () => {
  const first = acquireSakura({ backdrop: true, theme: "dark" });
  const second = acquireSakura({ mode: "overlay" });
  const layers = window.document.querySelectorAll(IROHA_SAKURA_TAG);
  assert.equal(layers.length, 1);
  assert.equal(first.element, second.element);
  assert.equal(window.document.body.firstElementChild, first.element);
  assert.ok(first.element.hasAttribute("backdrop"), "any lease can ask for the backdrop");
  assert.equal(first.element.getAttribute("mode"), "overlay", "any lease can ask for overlay");
  assert.equal(first.element.getAttribute("theme"), "dark", "the oldest lease sets the theme");
  assert.equal(first.element.getAttribute("aria-hidden"), "true");
  assert.equal(first.element.style.position, "fixed");
  assert.equal(first.element.style.maxWidth, "none", "host layout rules cannot shrink the layer");
  assert.ok(first.element.field?.running);
  first.release();
  first.release();
  assert.equal(window.document.querySelectorAll(IROHA_SAKURA_TAG).length, 1);
  assert.equal(second.element.hasAttribute("backdrop"), false, "options follow the remaining leases");
  assert.equal(second.element.getAttribute("theme"), "auto");
  assert.equal(first.burst(0, 0, 3), 0, "a released lease cannot burst");
  second.release();
  assert.equal(window.document.querySelectorAll(IROHA_SAKURA_TAG).length, 0);
});

test("toggling options on every widget updates the shared layer", () => {
  let one = acquireSakura({});
  let two = acquireSakura({});
  const layer = one.element;
  one.release();
  one = acquireSakura({ backdrop: true });
  two.release();
  two = acquireSakura({ backdrop: true });
  assert.equal(one.element, layer, "the layer survives the round trip");
  assert.ok(layer.hasAttribute("backdrop"));
  one.release();
  two.release();
  assert.equal(layer.isConnected, false);
});

test("a layer the page declares is reused and never removed", () => {
  const declared = window.document.createElement(IROHA_SAKURA_TAG);
  window.document.body.append(declared);
  const lease = acquireSakura();
  assert.equal(lease.element, declared);
  lease.release();
  assert.ok(declared.isConnected);
  declared.remove();
  assert.equal(declared.field, null, "a removed layer stops animating");
});

test("auto theme follows data-theme, then a dark/light class, then the OS", async () => {
  const root = window.document.documentElement;
  stubMatchMedia(() => false);
  try {
    assert.equal(resolveTheme("auto", window.document), "light");
    stubMatchMedia((query) => query.includes("dark"));
    assert.equal(resolveTheme("auto", window.document), "dark");
    root.classList.add("light");
    assert.equal(resolveTheme("auto", window.document), "light");
    root.setAttribute("data-theme", "dark");
    assert.equal(resolveTheme("auto", window.document), "dark");
    assert.equal(resolveTheme("light", window.document), "light");
    assert.equal(resolveTheme("nonsense", window.document), "dark");

    const layer = window.document.createElement(IROHA_SAKURA_TAG);
    window.document.body.append(layer);
    const inner = layer.shadowRoot.querySelector(".layer");
    assert.equal(inner.dataset.theme, "dark");
    assert.equal(layer.field.theme, "dark");
    root.setAttribute("data-theme", "light");
    await waitFor(() => inner.dataset.theme === "light", { label: "theme switch" });
    assert.equal(layer.field.theme, "light");
    layer.setAttribute("theme", "dark");
    assert.equal(inner.dataset.theme, "dark");
    layer.remove();
  } finally {
    root.removeAttribute("data-theme");
    root.classList.remove("light");
    delete window.matchMedia;
  }
  assert.equal(PETAL_COLORS.dark, "rgba(252, 181, 212, 0.9)");
});
