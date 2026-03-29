export interface BrowserConnectKeyPair {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}

export interface BrowserConnectBinaryLikeSource {
  readonly length: number;
  [index: number]: number;
}

export type BrowserConnectBinaryLike =
  | Uint8Array
  | ArrayBuffer
  | ArrayBufferView
  | number[]
  | BrowserConnectBinaryLikeSource
  | string;

export interface BrowserConnectSessionPreviewOptions {
  chainId: string;
  node?: string | null;
  nonce?: BrowserConnectBinaryLike | null;
  appKeyPair?: {
    publicKey: BrowserConnectBinaryLike;
    privateKey: BrowserConnectBinaryLike;
  };
}

export interface BrowserConnectSessionPreview {
  chainId: string;
  node: string | null;
  sidBytes: Uint8Array;
  sidBase64Url: string;
  nonce: Uint8Array;
  appKeyPair: BrowserConnectKeyPair;
  walletUri: string;
  appUri: string;
  wsUrl: string;
  createdAt: number;
}

export interface BrowserConnectSessionResponse {
  sid: string;
  wallet_uri: string;
  app_uri: string;
  token_app: string;
  token_wallet: string;
  extra?: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface BrowserConnectFetchOptions {
  fetchImpl?: typeof fetch;
}

export interface BrowserConnectRegisterOptions extends BrowserConnectFetchOptions {
  node?: string | null;
}

export interface BrowserConnectSocketOptions {
  webSocketImpl?: typeof WebSocket;
  protocols?: string | ReadonlyArray<string>;
}

export function toHex(bytes: Uint8Array): string;
export function toBase64Url(bytes: Uint8Array): string;
export function buildConnectWebSocketUrl(
  baseUrl: string,
  sid: string,
  role?: "app" | "wallet",
): string;
export function createConnectSessionPreview(
  options: BrowserConnectSessionPreviewOptions,
): BrowserConnectSessionPreview;
export function registerConnectSession(
  baseUrl: string,
  sid: string,
  options?: BrowserConnectRegisterOptions,
): Promise<BrowserConnectSessionResponse>;
export function deleteConnectSession(
  baseUrl: string,
  sid: string,
  options?: BrowserConnectFetchOptions,
): Promise<void>;
export function buildConnectTokenProtocol(token: string): string;
export function resolveConnectLaunchUri(
  role: "app" | "wallet",
  preview?: Pick<BrowserConnectSessionPreview, "walletUri" | "appUri"> | null,
  session?: Pick<BrowserConnectSessionResponse, "wallet_uri" | "app_uri"> | null,
): string;
export function openConnectWebSocket(
  baseUrl: string,
  sid: string,
  token: string,
  role?: "app" | "wallet",
  options?: BrowserConnectSocketOptions,
): WebSocket;
