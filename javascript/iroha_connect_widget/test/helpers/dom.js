// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { JSDOM, VirtualConsole } from "jsdom";

/**
 * Install a jsdom window as the global browser environment. Call before
 * importing widget modules: custom elements extend the global `HTMLElement`
 * and the controller extends the global `EventTarget` at import time.
 */
export function installDom({ html = "<!doctype html><html><body></body></html>", width = 1280, height = 800 } = {}) {
  const virtualConsole = new VirtualConsole();
  virtualConsole.on("jsdomError", (error) => {
    // jsdom has no canvas backend; the sakura field already runs without a 2D context.
    if (!/not implemented/iu.test(error.message)) {
      console.error(error);
    }
  });
  const dom = new JSDOM(html, { pretendToBeVisual: true, url: "https://app.example/", virtualConsole });
  const { window } = dom;
  Object.defineProperty(window, "innerWidth", { configurable: true, value: width, writable: true });
  Object.defineProperty(window, "innerHeight", { configurable: true, value: height, writable: true });
  for (const name of [
    "window",
    "document",
    "navigator",
    "location",
    "HTMLElement",
    "customElements",
    "Event",
    "EventTarget",
    "CustomEvent",
    "MutationObserver",
  ]) {
    Object.defineProperty(globalThis, name, {
      configurable: true,
      writable: true,
      value: name === "window" ? window : window[name],
    });
  }
  return window;
}

export { settle, waitFor } from "./wait.js";
