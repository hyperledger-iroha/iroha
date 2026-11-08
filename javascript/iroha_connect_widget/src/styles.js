// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Shadow-DOM styles for `<iroha-connect>`.
 *
 * Tokens mirror the Iroha wallet (`tokens.css`): Sora type, porcelain and
 * night glass surfaces, directional "global illumination" with a coral
 * bounce, and the Iroha red accent. Host pages cannot restyle the internals
 * by accident; deliberate layout tweaks go through `::part()`.
 *
 * The pairing dialog gives most of its area to the QR code. On wide or
 * landscape screens the code sits beside the copy and actions; on narrow
 * screens it sits above them. Its size follows the viewport so every state
 * fits without scrolling.
 */
export const CONNECT_STYLES = `
:host {
  display: inline-block;
  vertical-align: middle;
  -webkit-tap-highlight-color: transparent;
}
:host([hidden]) {
  display: none;
}
.root {
  --ic-font: "Sora", -apple-system, BlinkMacSystemFont, "Segoe UI", "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif;
  --ic-mono: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
  --ic-ease: cubic-bezier(0.2, 0.75, 0.25, 1);
  --ic-red: #e4232d;
  font-family: var(--ic-font);
}
.theme-dark {
  color-scheme: dark;
  --ic-text: #e7ebf6;
  --ic-text-strong: #f8f5f3;
  --ic-text-muted: #a9acb8;
  --ic-accent: #ff6468;
  --ic-accent-fill: linear-gradient(135deg, #e4232d 0%, #b92735 100%);
  --ic-accent-glow: rgba(228, 35, 45, 0.36);
  --ic-chip: rgba(16, 18, 25, 0.74);
  --ic-panel: rgba(13, 15, 22, 0.8);
  --ic-panel-border: rgba(255, 255, 255, 0.15);
  --ic-control: rgba(255, 255, 255, 0.06);
  --ic-control-hover: rgba(255, 255, 255, 0.11);
  --ic-border: rgba(255, 255, 255, 0.1);
  --ic-border-strong: rgba(255, 255, 255, 0.17);
  --ic-inset: rgba(8, 10, 15, 0.72);
  --ic-success: #70d6aa;
  --ic-success-soft: rgba(112, 214, 170, 0.16);
  --ic-danger: #ff928d;
  --ic-danger-soft: rgba(255, 146, 141, 0.14);
  --ic-focus: #9bbcff;
  --ic-overlay: rgba(3, 4, 8, 0.58);
  --ic-shadow: 0 18px 60px rgba(0, 0, 0, 0.66);
  --ic-rim: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  --ic-qr-ring: 0 0 0 1px rgba(255, 255, 255, 0.14), 0 0 0 7px rgba(252, 181, 212, 0.14), 0 18px 40px rgba(0, 0, 0, 0.36);
  --ic-illumination:
    radial-gradient(circle at -6% -10%, rgba(255, 255, 255, 0.08), transparent 38%),
    radial-gradient(circle at 96% 2%, rgba(255, 75, 75, 0.16), transparent 34%),
    radial-gradient(circle at 52% 116%, rgba(118, 133, 174, 0.1), transparent 45%);
}
.theme-light {
  color-scheme: light;
  --ic-text: #202126;
  --ic-text-strong: #15161a;
  --ic-text-muted: #565962;
  --ic-accent: #c41b27;
  --ic-accent-fill: linear-gradient(135deg, #d4202b 0%, #a91523 100%);
  --ic-accent-glow: rgba(196, 27, 39, 0.26);
  --ic-chip: rgba(255, 253, 250, 0.84);
  --ic-panel: rgba(255, 252, 249, 0.88);
  --ic-panel-border: rgba(255, 255, 255, 0.86);
  --ic-control: rgba(255, 255, 255, 0.74);
  --ic-control-hover: #ffffff;
  --ic-border: rgba(42, 35, 39, 0.1);
  --ic-border-strong: rgba(42, 35, 39, 0.17);
  --ic-inset: rgba(235, 229, 225, 0.82);
  --ic-success: #12643f;
  --ic-success-soft: #e6eee8;
  --ic-danger: #a93239;
  --ic-danger-soft: #f4e4e3;
  --ic-focus: #24579f;
  --ic-overlay: rgba(31, 26, 29, 0.32);
  --ic-shadow: 0 18px 60px rgba(73, 55, 64, 0.23);
  --ic-rim: inset 0 1px 0 rgba(255, 255, 255, 0.96);
  --ic-qr-ring: 0 0 0 1px rgba(42, 35, 39, 0.08), 0 0 0 7px rgba(247, 166, 198, 0.24), 0 16px 36px rgba(73, 55, 64, 0.18);
  --ic-illumination:
    radial-gradient(circle at -6% -10%, rgba(255, 255, 255, 0.72), transparent 39%),
    radial-gradient(circle at 96% 2%, rgba(255, 75, 75, 0.15), transparent 34%),
    radial-gradient(circle at 52% 116%, rgba(222, 214, 218, 0.5), transparent 46%);
}

button,
a {
  -webkit-tap-highlight-color: transparent;
}

.trigger {
  all: unset;
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  gap: 9px;
  min-height: 40px;
  padding: 3px 13px 3px 3px;
  /* Same border width as the connected chip, so connecting never shifts layout. */
  border: 1px solid transparent;
  border-radius: 11px;
  color: #ffffff;
  background: var(--ic-accent-fill);
  box-shadow: 0 8px 22px var(--ic-accent-glow), inset 0 1px 0 rgba(255, 255, 255, 0.24);
  font: 700 0.85rem/1 var(--ic-font);
  letter-spacing: 0.01em;
  white-space: nowrap;
  cursor: pointer;
  transition:
    transform 150ms var(--ic-ease),
    box-shadow 150ms var(--ic-ease),
    filter 150ms var(--ic-ease),
    background 150ms var(--ic-ease);
}
.trigger:hover {
  transform: translateY(-1px);
  filter: brightness(1.06);
  box-shadow: 0 12px 28px var(--ic-accent-glow), inset 0 1px 0 rgba(255, 255, 255, 0.24);
}
.trigger:active {
  transform: translateY(0);
  filter: brightness(0.98);
}
.trigger:focus-visible,
.button:focus-visible,
.close:focus-visible {
  outline: 2px solid var(--ic-focus);
  outline-offset: 3px;
}
.trigger[data-connected] {
  padding-inline-end: 11px;
  color: var(--ic-text-strong);
  background: var(--ic-chip);
  border: 1px solid var(--ic-border-strong);
  box-shadow: var(--ic-rim), 0 8px 24px rgba(0, 0, 0, 0.16);
  backdrop-filter: blur(12px) saturate(150%);
  -webkit-backdrop-filter: blur(12px) saturate(150%);
}
.trigger[data-connected]:hover {
  filter: none;
  background: var(--ic-control-hover);
  box-shadow: var(--ic-rim), 0 12px 30px rgba(0, 0, 0, 0.2);
}
.badge {
  flex: 0 0 auto;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  background: #ffffff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.16);
}
.badge svg {
  width: 22px;
  height: 21px;
  display: block;
}
.trigger-account {
  font: 600 0.8rem/1 var(--ic-mono);
  letter-spacing: 0;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ic-success);
  box-shadow: 0 0 0 3px var(--ic-success-soft);
}
[hidden] {
  display: none !important;
}

.dialog {
  box-sizing: border-box;
  width: 100vw;
  height: 100vh;
  height: 100dvh;
  max-width: none;
  max-height: none;
  margin: 0;
  padding: 12px;
  border: 0;
  overflow: auto;
  color: var(--ic-text);
  background: transparent;
  font-family: var(--ic-font);
  overscroll-behavior: contain;
}
.dialog[open] {
  display: grid;
  place-items: center;
}
.dialog::backdrop {
  background: rgba(3, 4, 8, 0.58);
  backdrop-filter: blur(5px) saturate(120%);
  -webkit-backdrop-filter: blur(5px) saturate(120%);
}
.theme-light .dialog::backdrop {
  background: rgba(31, 26, 29, 0.32);
}
.sky,
.dialog-sakura {
  position: fixed;
  inset: 0;
  pointer-events: none;
}
.sky {
  background: var(--ic-illumination);
}

/* Stacked layout: header, code, then copy and actions. The code keeps
   about 300px of height free for the rest so the actions stay on screen. */
.panel {
  --ic-vh: 100vh;
  --ic-pad: 20px;
  --ic-qr: min(100%, 300px, max(200px, calc(var(--ic-vh) - 300px)));
  position: relative;
  z-index: 1;
  box-sizing: border-box;
  width: min(380px, 100%);
  padding: var(--ic-pad);
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-areas:
    "head"
    "stage"
    "body";
  gap: 16px;
  border: 1px solid var(--ic-panel-border);
  border-radius: 18px;
  background: var(--ic-panel);
  box-shadow: var(--ic-shadow), var(--ic-rim);
  backdrop-filter: blur(28px) saturate(150%);
  -webkit-backdrop-filter: blur(28px) saturate(150%);
  animation: ic-rise 260ms var(--ic-ease) both;
}
@supports (height: 100dvh) {
  .panel {
    --ic-vh: 100dvh;
  }
}
.head {
  grid-area: head;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
}
.mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 11px;
  background: #ffffff;
  box-shadow: 0 6px 18px rgba(228, 35, 45, 0.24);
}
.mark svg {
  width: 26px;
  height: 25px;
  display: block;
}
.heading {
  min-width: 0;
}
.title {
  margin: 0;
  color: var(--ic-text-strong);
  font-size: 1.02rem;
  font-weight: 700;
  line-height: 1.25;
  overflow-wrap: anywhere;
}
.meta {
  margin: 3px 0 0;
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
  color: var(--ic-text-muted);
  font-size: 0.72rem;
  line-height: 1.3;
}
.eyebrow {
  flex: 0 0 auto;
  color: var(--ic-accent);
  font-weight: 700;
}
.network {
  min-width: 0;
  overflow: hidden;
  font-family: var(--ic-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.eyebrow:not(:empty) + .network::before {
  content: "·";
  margin-inline-end: 6px;
  font-family: var(--ic-font);
}
.close {
  all: unset;
  box-sizing: border-box;
  align-self: start;
  width: 32px;
  height: 32px;
  margin: -4px -6px 0 0;
  display: grid;
  place-items: center;
  border-radius: 9px;
  color: var(--ic-text-muted);
  cursor: pointer;
  transition: background 150ms var(--ic-ease), color 150ms var(--ic-ease);
}
.close:hover {
  color: var(--ic-text-strong);
  background: var(--ic-control-hover);
}
.close svg {
  width: 16px;
  height: 16px;
}

.stage {
  grid-area: stage;
  display: grid;
  place-items: center;
}
.dialog[data-view="account"] .stage {
  display: none;
}
/* The code's own four-module quiet zone is the white margin. */
.qr-frame {
  position: relative;
  box-sizing: border-box;
  width: var(--ic-qr);
  aspect-ratio: 1;
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: 12px;
  background: #ffffff;
  box-shadow: var(--ic-qr-ring);
}
.qr {
  width: 100%;
  height: 100%;
  transition: opacity 200ms var(--ic-ease), filter 200ms var(--ic-ease);
}
.qr svg {
  display: block;
  width: 100%;
  height: 100%;
}
.dialog:not([data-status="awaiting"]):not([data-status="connected"]) .qr {
  visibility: hidden;
}
.dialog[data-status="connected"] .qr {
  opacity: 0.12;
  filter: blur(1px);
}
.spinner,
.result {
  position: absolute;
  inset: 0;
  margin: auto;
}
.spinner {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  border: 3px solid rgba(21, 22, 26, 0.08);
  border-top-color: var(--ic-red);
  animation: ic-spin 900ms linear infinite;
  display: none;
}
.dialog[data-status="preparing"] .spinner {
  display: block;
}
.result {
  width: 72px;
  height: 72px;
  display: none;
  place-items: center;
  border-radius: 50%;
  color: #ffffff;
}
.result svg {
  width: 34px;
  height: 34px;
}
.dialog[data-status="connected"] .result {
  display: grid;
  background: #12643f;
  box-shadow: 0 0 0 10px rgba(18, 100, 63, 0.14);
  animation: ic-pop 360ms var(--ic-ease) both;
}
.dialog[data-status="error"] .result,
.dialog[data-status="config"] .result {
  display: grid;
  background: #a93239;
  box-shadow: 0 0 0 10px rgba(169, 50, 57, 0.12);
}
.result .icon-error,
.dialog[data-status="error"] .result .icon-check,
.dialog[data-status="config"] .result .icon-check {
  display: none;
}
.dialog[data-status="error"] .result .icon-error,
.dialog[data-status="config"] .result .icon-error {
  display: block;
}
/* An error needs its message and a retry, not the code's full footprint. */
.dialog[data-status="error"] .qr-frame,
.dialog[data-status="config"] .qr-frame {
  width: auto;
  aspect-ratio: auto;
  overflow: visible;
  background: none;
  box-shadow: none;
}
.dialog[data-status="error"] .qr,
.dialog[data-status="config"] .qr {
  display: none;
}
.dialog[data-status="error"] .result,
.dialog[data-status="config"] .result {
  position: static;
  width: 56px;
  height: 56px;
  margin: 10px;
}
.dialog[data-status="error"] .result svg,
.dialog[data-status="config"] .result svg {
  width: 28px;
  height: 28px;
}

.body {
  grid-area: body;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.lede {
  margin: 0;
  color: var(--ic-text-muted);
  font-size: 0.84rem;
  line-height: 1.5;
  text-align: center;
}
.dialog[data-view="account"] .lede {
  text-align: start;
}
.status {
  margin: 0;
  min-height: 1.4em;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--ic-text);
  font-size: 0.84rem;
  font-weight: 600;
  line-height: 1.4;
  text-align: center;
}
.dialog[data-view="account"] .status {
  display: none;
}
.status[data-tone="error"] {
  color: var(--ic-danger);
}
.status[data-tone="success"] {
  color: var(--ic-success);
}
.pulse {
  flex: 0 0 auto;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ic-accent);
  animation: ic-pulse 1400ms ease-in-out infinite;
}
.status:not([data-tone="info"]) .pulse {
  display: none;
}
.account-card {
  display: none;
  gap: 4px;
  padding: 12px;
  border: 1px solid var(--ic-border);
  border-radius: 12px;
  background: var(--ic-inset);
}
.dialog[data-view="account"] .account-card {
  display: grid;
}
.account-label {
  color: var(--ic-text-muted);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}
.account-id {
  color: var(--ic-text-strong);
  font: 600 0.8rem/1.5 var(--ic-mono);
  word-break: break-all;
}
.actions {
  margin-top: auto;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.button {
  all: unset;
  box-sizing: border-box;
  flex: 1 1 112px;
  min-width: 0;
  min-height: 40px;
  padding: 0 14px;
  display: none;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border-radius: 10px;
  font: 700 0.8rem/1.2 var(--ic-font);
  text-align: center;
  cursor: pointer;
  transition:
    transform 150ms var(--ic-ease),
    background 150ms var(--ic-ease),
    border-color 150ms var(--ic-ease),
    filter 150ms var(--ic-ease);
}
.button:hover {
  transform: translateY(-1px);
}
.button.primary {
  color: #ffffff;
  background: var(--ic-accent-fill);
  box-shadow: 0 8px 20px var(--ic-accent-glow), inset 0 1px 0 rgba(255, 255, 255, 0.22);
}
.button.primary:hover {
  filter: brightness(1.06);
}
.button.secondary {
  color: var(--ic-text-strong);
  background: var(--ic-control);
  border: 1px solid var(--ic-border-strong);
}
.button.secondary:hover {
  background: var(--ic-control-hover);
}
.button.danger {
  color: var(--ic-danger);
  background: var(--ic-danger-soft);
  border: 1px solid transparent;
}
.dialog[data-view="pairing"][data-status="awaiting"] [data-action="open"],
.dialog[data-view="pairing"][data-status="awaiting"] [data-action="copy"],
.dialog[data-view="pairing"][data-status="preparing"] [data-action="cancel"],
.dialog[data-view="pairing"][data-status="error"] [data-action="retry"],
.dialog[data-view="pairing"][data-status="error"] [data-action="cancel"],
.dialog[data-view="pairing"][data-status="config"] [data-action="cancel"],
.dialog[data-view="account"] [data-action="copy-account"],
.dialog[data-view="account"] [data-action="disconnect"] {
  display: inline-flex;
}

/* Wide or landscape screens: the code beside the copy and actions, sized
   to the viewport height and leaving at least 400px of width for the rest. */
@media (min-width: 600px), (min-width: 480px) and (orientation: landscape) {
  .dialog[data-view="pairing"]:not([data-status="error"]):not([data-status="config"]) .panel {
    --ic-qr: clamp(200px, min(calc(var(--ic-vh) - 72px), calc(100vw - 400px)), 320px);
    width: auto;
    max-width: 100%;
    grid-template-columns: var(--ic-qr) minmax(0, 300px);
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-areas:
      "stage head"
      "stage body";
    column-gap: 24px;
    row-gap: 12px;
  }
  .dialog[data-view="pairing"]:not([data-status="error"]):not([data-status="config"]) .lede {
    text-align: start;
  }
  .dialog[data-view="pairing"]:not([data-status="error"]):not([data-status="config"]) .status {
    justify-content: flex-start;
    text-align: start;
  }
}
@media (max-width: 380px) {
  .dialog {
    padding: 8px;
  }
  .panel {
    --ic-pad: 16px;
    gap: 14px;
  }
}
@media (pointer: coarse) {
  .trigger {
    min-height: 44px;
    padding-block: 5px;
    padding-inline-start: 5px;
  }
  .button {
    min-height: 44px;
  }
  .close {
    width: 40px;
    height: 40px;
  }
}

@keyframes ic-rise {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.98);
  }
}
@keyframes ic-spin {
  to {
    transform: rotate(360deg);
  }
}
@keyframes ic-pulse {
  0%,
  100% {
    opacity: 0.35;
    transform: scale(0.85);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
}
@keyframes ic-pop {
  from {
    opacity: 0;
    transform: scale(0.6);
  }
}
@media (prefers-reduced-motion: reduce) {
  .panel,
  .spinner,
  .pulse,
  .dialog[data-status="connected"] .result {
    animation: none;
  }
  .trigger,
  .button,
  .close,
  .qr {
    transition: none;
  }
  .trigger:hover,
  .button:hover {
    transform: none;
  }
}
`;
