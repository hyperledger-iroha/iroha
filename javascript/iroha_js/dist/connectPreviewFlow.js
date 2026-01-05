import { createConnectSessionPreview } from "./connectSession.js";

function requireObject(value, name) {
  if (!value || typeof value !== "object") {
    throw new TypeError(`${name} must be an object`);
  }
  return value;
}

function requireToriiClient(client) {
  if (!client || typeof client.createConnectSession !== "function") {
    throw new TypeError("toriiClient must expose createConnectSession()");
  }
  return client;
}

function normalizeSessionOptions(options) {
  if (options === undefined || options === null) {
    return {};
  }
  if (typeof options !== "object") {
    throw new TypeError("sessionOptions must be an object");
  }
  const normalized = {};
  if (options.node !== undefined && options.node !== null) {
    normalized.node = requireNonEmptyString(options.node, "sessionOptions.node");
  }
  return normalized;
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new RangeError(`${name} must not be empty`);
  }
  return trimmed;
}

/**
 * Generate a Connect session preview and (optionally) register it with Torii.
 * @param {{ createConnectSession(input: { sid: string; node?: string | null }): Promise<import("./toriiClient.js").ConnectSessionResponse> }} toriiClient
 * @param {import("./connectSession.js").ConnectSessionPreviewOptions & {
 *   register?: boolean;
 *   sessionOptions?: { node?: string | null } | null;
 * }} options
 * @returns {Promise<{
 *   preview: import("./connectSession.js").ConnectSessionPreview;
 *   session: import("./toriiClient.js").ConnectSessionResponse | null;
 *   tokens: { wallet: string; app: string } | null;
 * }>}
 */
export async function bootstrapConnectPreviewSession(toriiClient, options = {}) {
  requireToriiClient(toriiClient);
  requireObject(options, "options");
  const preview = createConnectSessionPreview({
    chainId: options.chainId,
    node: options.node ?? null,
    nonce: options.nonce ?? null,
    appKeyPair: options.appKeyPair ?? null,
  });
  const shouldRegister = options.register !== false;
  if (!shouldRegister) {
    return {
      preview,
      session: null,
      tokens: null,
    };
  }
  const sessionOverrides = normalizeSessionOptions(options.sessionOptions ?? null);
  const sessionInput = {
    ...sessionOverrides,
    sid: preview.sidBase64Url,
  };
  if (
    (sessionInput.node === undefined || sessionInput.node === null) &&
    preview.node
  ) {
    sessionInput.node = preview.node;
  }
  const session = await toriiClient.createConnectSession(sessionInput);
  return {
    preview,
    session,
    tokens: {
      wallet: session.token_wallet,
      app: session.token_app,
    },
  };
}
