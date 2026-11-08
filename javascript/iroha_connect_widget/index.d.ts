// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import type {
  AccountSigningKey,
  BrowserConnectAppSession,
  BrowserConnectBinaryLike,
  BrowserConnectCanonicalRequestAuth,
  BrowserConnectPermissions,
} from "@iroha/iroha-js/connect-browser";
import type {
  IrohaConnectApproval,
  IrohaConnectController,
  IrohaConnectDisconnect,
  IrohaConnectError,
  IrohaConnectErrorCode,
  IrohaConnectSdk,
  IrohaConnectState,
} from "./controller.js";

export * from "./controller.js";
export * from "./qr.js";
export * from "./sakura.js";

export const IROHA_CONNECT_TAG: "iroha-connect";

/** DOM events dispatched by `<iroha-connect>`; they bubble and cross shadow roots. */
export const IROHA_CONNECT_EVENTS: Readonly<{
  state: "iroha-connect-state";
  approved: "iroha-connect-approved";
  disconnected: "iroha-connect-disconnected";
  error: "iroha-connect-error";
}>;

/** Iroha red, the fill of the flower mark. */
export const IROHA_RED: "#e4232d";
/** Petal pink of the sakura layer (dark theme). */
export const SAKURA_PINK: "#fcb5d4";
/** Decorative Iroha flower mark (`aria-hidden`). */
export const IROHA_MARK_SVG: string;

/** Inline SVG markup for the Iroha flower mark; pass `title` for a labeled image. */
export function irohaMarkSvg(options?: { title?: string; strokeWidth?: number }): string;

/** Every user-visible string of the widget. */
export interface IrohaConnectStrings {
  trigger: string;
  triggerLabel: string;
  connectedLabel: string;
  eyebrow: string;
  title: string;
  accountTitle: string;
  ledePreparing: string;
  ledeAwaiting: string;
  ledeConnected: string;
  ledeError: string;
  ledeAccount: string;
  ledeConfig: string;
  statusPreparing: string;
  statusAwaiting: string;
  statusConnected: string;
  qrLabel: string;
  openWallet: string;
  copyLink: string;
  copied: string;
  retry: string;
  cancel: string;
  close: string;
  disconnect: string;
  copyAccount: string;
  account: string;
  network: string;
  errorRejected: string;
  errorClosed: string;
  errorNetwork: string;
  errorConfig: string;
  errorFailed: string;
}

/** Built-in English and Japanese copy. */
export const STRINGS: Readonly<{ en: Readonly<IrohaConnectStrings>; ja: Readonly<IrohaConnectStrings> }>;

export interface IrohaConnectErrorDetail {
  readonly code: IrohaConnectErrorCode;
  readonly message: string;
  readonly error: IrohaConnectError;
}

export interface IrohaConnectElementEventMap {
  "iroha-connect-state": CustomEvent<IrohaConnectState>;
  "iroha-connect-approved": CustomEvent<IrohaConnectApproval>;
  "iroha-connect-disconnected": CustomEvent<IrohaConnectDisconnect>;
  "iroha-connect-error": CustomEvent<IrohaConnectErrorDetail>;
}

/**
 * `<iroha-connect>`: the IrohaConnect button, pairing dialog and sakura layer.
 *
 * Attributes: `torii-url`, `network-id` (required), `app-name`, `app-url`,
 * `launch-protocol`, `allow-insecure`, `theme`, `sakura`, `backdrop`, `lang`.
 */
export class IrohaConnectElement extends HTMLElement {
  readonly state: IrohaConnectState;
  readonly accountId: string | null;
  readonly controller: IrohaConnectController | null;
  readonly appSession: BrowserConnectAppSession | null;
  sdk: IrohaConnectSdk | null;
  permissions: BrowserConnectPermissions | null;
  strings: Partial<IrohaConnectStrings> | null;
  /** Open the pairing dialog and resolve with the wallet approval. */
  connect(): Promise<IrohaConnectApproval>;
  /** Close the session (or abandon pairing) and close the dialog. */
  disconnect(): Promise<void>;
  /** Close the dialog; an unfinished pairing is cancelled. */
  close(): void;
  signTransaction(unsignedTxBytes: BrowserConnectBinaryLike): Promise<Uint8Array>;
  /** Signed by the account's off-chain signing key over the off-chain envelope. */
  signRaw(domainTag: string, bytes: BrowserConnectBinaryLike): Promise<Uint8Array>;
  /** The connected account's off-chain signing key; rejects with `SIGNING_KEY_REQUIRED` without one. */
  accountSigningKey(): Promise<AccountSigningKey>;
  createCanonicalRequestAuth(): Promise<BrowserConnectCanonicalRequestAuth>;
  addEventListener<K extends keyof IrohaConnectElementEventMap>(
    type: K,
    listener: (this: IrohaConnectElement, event: IrohaConnectElementEventMap[K]) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener<K extends keyof HTMLElementEventMap>(
    type: K,
    listener: (this: IrohaConnectElement, event: HTMLElementEventMap[K]) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof IrohaConnectElementEventMap>(
    type: K,
    listener: (this: IrohaConnectElement, event: IrohaConnectElementEventMap[K]) => void,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener<K extends keyof HTMLElementEventMap>(
    type: K,
    listener: (this: IrohaConnectElement, event: HTMLElementEventMap[K]) => void,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** Register `<iroha-connect>` and `<iroha-sakura>` once; returns false outside a browser. */
export function defineIrohaConnect(registry?: CustomElementRegistry): boolean;

declare global {
  interface HTMLElementTagNameMap {
    "iroha-connect": IrohaConnectElement;
  }
}
