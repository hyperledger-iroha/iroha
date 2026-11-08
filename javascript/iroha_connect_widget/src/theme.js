// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Light/dark resolution shared by the sakura layer and the IrohaConnect widget.
 *
 * `auto` follows the host page: an explicit `data-theme="dark|light"` on the
 * root element wins (the Iroha wallet convention), then a `dark`/`light`
 * class (the Tailwind convention), then the user's color-scheme preference.
 */

/** Concrete themes the widget renders. */
export const THEMES = Object.freeze(["dark", "light"]);

/** Theme preferences accepted by the `theme` attribute. */
export const THEME_PREFERENCES = Object.freeze(["auto", "dark", "light"]);

/** Normalize a `theme` attribute value; unknown values mean `auto`. */
export function normalizeThemePreference(value) {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  return normalized === "dark" || normalized === "light" ? normalized : "auto";
}

/**
 * Resolve a theme preference against a document.
 *
 * @param {string | null | undefined} preference
 * @param {Document | undefined} [doc]
 * @returns {"dark" | "light"}
 */
export function resolveTheme(preference, doc = globalThis.document) {
  const normalized = normalizeThemePreference(preference);
  if (normalized !== "auto") {
    return normalized;
  }
  const root = doc?.documentElement;
  const declared = root?.getAttribute?.("data-theme");
  if (declared === "dark" || declared === "light") {
    return declared;
  }
  if (root?.classList?.contains("dark")) {
    return "dark";
  }
  if (root?.classList?.contains("light")) {
    return "light";
  }
  const query = doc?.defaultView?.matchMedia?.("(prefers-color-scheme: dark)");
  return query?.matches ? "dark" : "light";
}

/**
 * Call `onTheme` with the resolved theme now and whenever it changes.
 *
 * @param {string | null | undefined} preference
 * @param {(theme: "dark" | "light") => void} onTheme
 * @param {Document | undefined} [doc]
 * @returns {() => void} stops watching
 */
export function watchTheme(preference, onTheme, doc = globalThis.document) {
  let current = resolveTheme(preference, doc);
  onTheme(current);
  if (normalizeThemePreference(preference) !== "auto" || !doc?.documentElement) {
    return () => {};
  }
  const update = () => {
    const next = resolveTheme(preference, doc);
    if (next !== current) {
      current = next;
      onTheme(next);
    }
  };
  const view = doc.defaultView;
  const query = view?.matchMedia?.("(prefers-color-scheme: dark)") ?? null;
  query?.addEventListener?.("change", update);
  const Observer = view?.MutationObserver;
  const observer = typeof Observer === "function" ? new Observer(update) : null;
  observer?.observe(doc.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme", "class"],
  });
  return () => {
    query?.removeEventListener?.("change", update);
    observer?.disconnect();
  };
}
