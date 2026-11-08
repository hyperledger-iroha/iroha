// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import type * as ConnectBrowser from "@iroha/iroha-js/connect-browser";
import type {
  AccountSigningKey,
  BrowserConnectAppSession,
  BrowserConnectAppSessionOptions,
  BrowserConnectBinaryLike,
  BrowserConnectCanonicalRequestAuth,
  BrowserConnectPermissions,
  NetworkId,
} from "@iroha/iroha-js/connect-browser";

/** Pairing states, in the order a successful connection visits them. */
export type IrohaConnectStatus = "idle" | "preparing" | "awaiting" | "connected" | "error";

export type IrohaConnectErrorCode =
  | "config"
  | "cancelled"
  | "rejected"
  | "closed"
  | "network"
  | "not_connected"
  | "failed";

export const CONNECT_STATUSES: readonly IrohaConnectStatus[];

/** Launch scheme of IrohaConnect wallets for same-device deep links. */
export const DEFAULT_LAUNCH_PROTOCOL: "irohaconnect";

/** Error raised by the controller; `code` is stable for UI decisions. */
export class IrohaConnectError extends Error {
  constructor(code: IrohaConnectErrorCode, message: string, options?: { cause?: unknown });
  readonly code: IrohaConnectErrorCode;
}

/** The wallet's verified approval. */
export interface IrohaConnectApproval {
  /** Canonical I105 account id approved by the wallet. */
  readonly accountId: string;
  /** Ed25519 key bound to `accountId` by the approval proof. */
  readonly signingPublicKey: Uint8Array;
  /** Ephemeral X25519 key of the encrypted Connect transport. */
  readonly walletPublicKey: Uint8Array;
  readonly sid: string;
  readonly toriiUrl: string;
}

export type IrohaConnectState =
  | { readonly status: "idle" }
  | { readonly status: "preparing" }
  | {
      readonly status: "awaiting";
      /** Canonical `iroha://connect?...&role=wallet` URI shown as the QR code. */
      readonly walletUri: string;
      /** Same-device deep link using the launch protocol. */
      readonly launchUri: string;
      readonly sid: string;
      readonly toriiUrl: string;
    }
  | {
      readonly status: "connected";
      readonly accountId: string;
      readonly sid: string;
      readonly toriiUrl: string;
      readonly approval: IrohaConnectApproval;
    }
  | {
      readonly status: "error";
      readonly code: IrohaConnectErrorCode;
      readonly message: string;
      readonly toriiUrl: string;
    };

export interface IrohaConnectAppMeta {
  name: string;
  url?: string | null;
  iconHash?: string | null;
}

/** SDK bindings the controller uses; defaults to `@iroha/iroha-js`. */
export interface IrohaConnectSdk {
  NetworkId: { parse(literal: string): NetworkId };
  createConnectSessionPreview: typeof ConnectBrowser.createConnectSessionPreview;
  registerConnectSession: typeof ConnectBrowser.registerConnectSession;
  createConnectAppSession: typeof ConnectBrowser.createConnectAppSession;
  deleteConnectSession: typeof ConnectBrowser.deleteConnectSession;
  resolveConnectLaunchUri: typeof ConnectBrowser.resolveConnectLaunchUri;
  resolveConnectLaunchUriForProtocol: typeof ConnectBrowser.resolveConnectLaunchUriForProtocol;
  createConnectCanonicalRequestAuth?: typeof ConnectBrowser.createConnectCanonicalRequestAuth;
  /** Native account inspection used to verify wallet approval in browsers. */
  createAccountInspector(options: {
    baseUrl: string;
    networkId: NetworkId;
  }): NonNullable<BrowserConnectAppSessionOptions["inspectAccountIdentity"]>;
}

export interface IrohaConnectControllerOptions {
  /** Torii origin, for example `https://taira.sora.org`. */
  toriiUrl: string;
  /** Exact deployed NetworkId literal; never guessed from a chain label. */
  networkId: string;
  appMeta?: IrohaConnectAppMeta | null;
  permissions?: BrowserConnectPermissions | null;
  launchProtocol?: string;
  /** Permit `http://` Torii and `ws://` relay for local development only. */
  allowInsecure?: boolean;
  sdk?: IrohaConnectSdk;
  loadSdk?: () => Promise<IrohaConnectSdk>;
}

export interface IrohaConnectConfig {
  readonly toriiUrl: string;
  readonly networkId: string;
  readonly appMeta: Readonly<{ name: string; url: string | null; iconHash: string | null }> | null;
  readonly permissions: Readonly<BrowserConnectPermissions> | null;
  readonly launchProtocol: string;
  readonly allowInsecure: boolean;
}

export interface IrohaConnectDisconnect {
  readonly reason: string;
  /** `app` after `disconnect()`, `remote` when the wallet or relay closed the session. */
  readonly initiator: "app" | "remote";
}

export interface IrohaConnectControllerEventMap {
  statechange: CustomEvent<IrohaConnectState>;
  approved: CustomEvent<IrohaConnectApproval>;
  disconnected: CustomEvent<IrohaConnectDisconnect>;
  error: CustomEvent<IrohaConnectError>;
}

/** Headless app-side IrohaConnect pairing over `@iroha/iroha-js/connect-browser`. */
export class IrohaConnectController extends EventTarget {
  constructor(options: IrohaConnectControllerOptions);
  readonly config: IrohaConnectConfig;
  readonly state: IrohaConnectState;
  readonly accountId: string | null;
  readonly appSession: BrowserConnectAppSession | null;
  /** Register a session and wait for wallet approval. */
  connect(): Promise<IrohaConnectApproval>;
  /** Abandon a pairing in progress (or disconnect a connected session). */
  cancel(reason?: string): Promise<void>;
  /** Close an approved session and delete it on Torii. */
  disconnect(reason?: string): Promise<void>;
  signTransaction(unsignedTxBytes: BrowserConnectBinaryLike): Promise<Uint8Array>;
  /**
   * Requires the corresponding `sign_raw` permission to be granted by the wallet.
   * Resolves with a signature by the account's off-chain signing key over the
   * off-chain envelope of `domainTag` and `bytes`.
   */
  signRaw(domainTag: string, bytes: BrowserConnectBinaryLike): Promise<Uint8Array>;
  /** The approved account's off-chain signing key; rejects with `SIGNING_KEY_REQUIRED` without one. */
  accountSigningKey(): Promise<AccountSigningKey>;
  createCanonicalRequestAuth(): Promise<BrowserConnectCanonicalRequestAuth>;
  addEventListener<K extends keyof IrohaConnectControllerEventMap>(
    type: K,
    listener: (event: IrohaConnectControllerEventMap[K]) => void,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof IrohaConnectControllerEventMap>(
    type: K,
    listener: (event: IrohaConnectControllerEventMap[K]) => void,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | EventListenerOptions,
  ): void;
}
