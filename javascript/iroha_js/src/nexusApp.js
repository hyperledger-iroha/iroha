import { AccountAddress } from "./address.js";
import {
  createConnectAppSession,
  createConnectSessionPreview,
  registerConnectSession,
} from "./connect.browser.js";
import { buildTransferAssetInstruction } from "./instructionBuilders.js";
import { getNativeBinding } from "./native.js";
import { ToriiClient } from "./toriiClient.js";
import { blake2b256 } from "./blake2b.js";
import { verifyEd25519 } from "./crypto.js";
import { hashSignedTransaction } from "./transaction.js";

const ALGORITHM_ED25519 = "ed25519";
const ALGORITHM_ED25519_TAG = 0;

function toBuffer(value, context) {
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (Array.isArray(value)) {
    return Buffer.from(value);
  }
  if (typeof value === "string") {
    const trimmed = value.startsWith("0x") ? value.slice(2) : value;
    if (/^[0-9a-fA-F]*$/.test(trimmed) && trimmed.length % 2 === 0) {
      return Buffer.from(trimmed, "hex");
    }
  }
  throw new TypeError(`${context} must be bytes or a hex string`);
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

function irohaPrehash(payloadBytes) {
  const digest = Buffer.from(blake2b256(payloadBytes));
  digest[digest.length - 1] |= 1;
  return digest;
}

function irohaPrehashHex(payloadBytes) {
  const digest = irohaPrehash(payloadBytes);
  return digest.toString("hex");
}

function resolveNativeBinding() {
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function accountEd25519PublicKey(accountId) {
  let address;
  try {
    address = AccountAddress.fromI105(accountId);
  } catch (error) {
    throw new NexusAppError(
      "missing_signing_public_key",
      "approved account must be a canonical single-key Ed25519 I105 account",
      error,
    );
  }
  const controller = address._controller;
  if (
    !controller ||
    controller.tag !== 0 ||
    controller.curve !== 1 ||
    controller.publicKey.length !== 32
  ) {
    throw new NexusAppError(
      "missing_signing_public_key",
      "approved account must be a canonical single-key Ed25519 I105 account",
    );
  }
  return Buffer.from(controller.publicKey);
}

function validateEd25519PublicKey(publicKey, context) {
  if (publicKey.length !== 32) {
    throw new NexusAppError(
      "invalid_signing_public_key",
      `${context} must be a 32-byte Ed25519 public key`,
    );
  }
  return publicKey;
}

function validateEd25519SignatureForPayload(publicKey, payloadBytes, signature) {
  let verified = false;
  try {
    verified = verifyEd25519(irohaPrehash(payloadBytes), signature, publicKey);
  } catch {
    verified = false;
  }
  if (!verified) {
    throw new NexusAppError(
      "invalid_signature",
      "Ed25519 signature does not verify for the signable payload",
    );
  }
}

function normalizeAlgorithm(algorithm) {
  if (algorithm === undefined || algorithm === null) {
    return ALGORITHM_ED25519;
  }
  const normalized = String(algorithm).toLowerCase();
  if (/^[\x20-\x7e]+$/.test(normalized) && (normalized === ALGORITHM_ED25519 || normalized === "0")) {
    return ALGORITHM_ED25519;
  }
  throw new NexusAppError(
    "unsupported_signature_algorithm",
    `unsupported signature algorithm ${String(algorithm)}`,
  );
}

function normalizeConnectSession(session) {
  if (!session || typeof session !== "object") {
    throw new TypeError("connect session must be an object");
  }
  return {
    ...session,
    sid: requireNonEmptyString(session.sid, "session.sid"),
    walletLaunchUri:
      session.walletLaunchUri ??
      session.wallet_launch_uri ??
      session.wallet_uri ??
      null,
    appLaunchUri: session.appLaunchUri ?? session.app_launch_uri ?? session.app_uri ?? null,
    tokenApp: session.tokenApp ?? session.token_app ?? null,
    tokenWallet: session.tokenWallet ?? session.token_wallet ?? null,
    tokenManagement:
      session.tokenManagement ?? session.token_management ?? null,
    tokenRelay: session.tokenRelay ?? session.token_relay ?? null,
    approvedAccountId:
      session.approvedAccountId ??
      session.approvedAccount ??
      session.approved_account ??
      null,
    approvedAccount:
      session.approvedAccount ??
      session.approvedAccountId ??
      session.approved_account ??
      null,
    signingPublicKey:
      session.signingPublicKey ??
      session.signing_public_key ??
      null,
  };
}

function isRawByteSignature(value) {
  return (
    Buffer.isBuffer(value) ||
    value instanceof Uint8Array ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer ||
    Array.isArray(value)
  );
}

function normalizeSignature(signature) {
  if (isRawByteSignature(signature) || !signature || typeof signature !== "object") {
    return {
      algorithm: ALGORITHM_ED25519,
      signature: toBuffer(signature, "signature"),
    };
  }
  const algorithm = normalizeAlgorithm(signature.algorithm ?? signature.alg);
  const bytes = toBuffer(
    signature.signature ?? signature.bytes ?? signature.payload,
    "signature.signature",
  );
  if (algorithm === ALGORITHM_ED25519 && bytes.length !== 64) {
    throw new NexusAppError(
      "invalid_signature",
      `Ed25519 signature must be 64 bytes, got ${bytes.length}`,
    );
  }
  return { algorithm, signature: bytes };
}

function defaultBuildTransferPayload(input) {
  const native = resolveNativeBinding();
  const metadata =
    input.metadata === undefined || input.metadata === null
      ? null
      : typeof input.metadata === "string"
        ? input.metadata
        : JSON.stringify(input.metadata);
  if (native && typeof native.buildTransferAssetPayload === "function") {
    const result = native.buildTransferAssetPayload(
      input.chainId,
      input.authority,
      input.sourceAssetHoldingId ?? input.sourceAssetId,
      String(input.quantity),
      input.destinationAccountId,
      metadata,
      input.creationTimeMs ?? null,
      input.ttlMs ?? null,
      input.nonce ?? null,
    );
    return toBuffer(
      result?.payloadBytes ?? result?.payload_bytes ?? result,
      "buildTransferAssetPayload result",
    );
  }
  if (native && typeof native.buildTransactionPayload === "function") {
    const instruction = buildTransferAssetInstruction({
      sourceAssetHoldingId: input.sourceAssetHoldingId ?? input.sourceAssetId,
      quantity: input.quantity,
      destinationAccountId: input.destinationAccountId,
    });
    const result = native.buildTransactionPayload(
      input.chainId,
      input.authority,
      [JSON.stringify(instruction)],
      metadata,
      input.creationTimeMs ?? null,
      input.ttlMs ?? null,
      input.nonce ?? null,
    );
    return toBuffer(
      result?.payloadBytes ?? result?.payload_bytes ?? result,
      "buildTransactionPayload result",
    );
  }
  throw new NexusAppError(
    "transaction_codec_unavailable",
    "native transaction payload builder is unavailable; provide config.transactionCodec.buildTransferPayload",
  );
}

function defaultFinalizeSignedTransaction(signable, signature, publicKey) {
  const native = resolveNativeBinding();
  const finalize =
    native?.finalizeSignedTransaction ??
    native?.finalizeTransactionWithSignature ??
    null;
  if (typeof finalize !== "function") {
    throw new NexusAppError(
      "transaction_codec_unavailable",
      "native signed transaction finalizer is unavailable; provide config.transactionCodec.finalizeSignedTransaction",
    );
  }
  return finalize({
    payloadBytes: Buffer.from(signable.payloadBytes),
    payload_hash_hex: signable.payloadHashHex,
    payloadHashHex: signable.payloadHashHex,
    signature: Buffer.from(signature.signature),
    signatureBytes: Buffer.from(signature.signature),
    publicKey: Buffer.from(publicKey),
    signingPublicKey: Buffer.from(publicKey),
    authority: signable.authority,
  });
}

function normalizeFinalizedTransaction(result) {
  const signedTransaction = toBuffer(
    result?.signedTransaction ??
      result?.signed_transaction ??
      result?.bytes ??
      result,
    "signed transaction",
  );
  let hashHex =
    result?.hashHex ??
    result?.hash_hex ??
    (result?.hash ? toBuffer(result.hash, "hash").toString("hex") : null);
  if (!hashHex) {
    hashHex = hashSignedTransaction(signedTransaction);
  }
  return { signedTransaction, hashHex: String(hashHex).toLowerCase() };
}

function submissionHashHex(submission) {
  const candidate =
    submission?.hashHex ??
    submission?.hash_hex ??
    submission?.hash ??
    submission?.txHash ??
    submission?.tx_hash ??
    submission?.transactionHashHex ??
    submission?.transaction_hash_hex ??
    submission?.signedTransactionHash ??
    submission?.signed_transaction_hash ??
    submission?.payload?.txHash ??
    submission?.payload?.tx_hash ??
    submission?.payload?.hash ??
    submission?.payload?.signedTransactionHash ??
    submission?.payload?.signed_transaction_hash ??
    null;
  if (!candidate) {
    return null;
  }
  return toBuffer(candidate, "submission.hash").toString("hex");
}

function maybeInvoke(method, receiver, ...args) {
  if (typeof method === "function") {
    return method.apply(receiver, args);
  }
  return undefined;
}

export class NexusAppError extends Error {
  constructor(code, message, cause = null) {
    super(message);
    this.name = "NexusAppError";
    this.code = code;
    if (cause) {
      this.cause = cause;
    }
  }
}

export class NexusAppClient {
  constructor(config = {}) {
    if (!config || typeof config !== "object") {
      throw new TypeError("NexusAppClient config must be an object");
    }
    this.config = { ...config };
    this.connect = config.connectTransport ?? config.connect ?? null;
    this.transactionCodec = config.transactionCodec ?? null;
    this.toriiClient =
      config.toriiClient ??
      (config.toriiBaseUrl || config.baseUrl
        ? new ToriiClient(config.toriiBaseUrl ?? config.baseUrl, {
            fetchImpl: config.fetchImpl,
          })
        : null);
  }

  async startConnect(options = {}) {
    const injected = await maybeInvoke(
      this.connect?.startConnect,
      this.connect,
      options,
      this.config,
    );
    if (injected !== undefined) {
      return normalizeConnectSession(injected);
    }
    const baseUrl = requireNonEmptyString(
      this.config.connectBaseUrl ?? this.config.toriiBaseUrl ?? this.config.baseUrl,
      "config.baseUrl",
    );
    const chainId = requireNonEmptyString(
      options.chainId ?? this.config.chainId,
      "chainId",
    );
    const node = options.node ?? this.config.node ?? null;
    const preview = createConnectSessionPreview({
      chainId,
      node,
      appKeyPair: options.appKeyPair,
      nonce: options.nonce,
      protocol: options.protocol,
    });
    const registered = await registerConnectSession(
      baseUrl,
      preview.sidBase64Url,
      { node, fetchImpl: this.config.fetchImpl },
    );
    return normalizeConnectSession({
      ...registered,
      preview,
      walletLaunchUri: registered.wallet_uri ?? preview.walletUri,
      appLaunchUri: registered.app_uri ?? preview.appUri,
    });
  }

  async awaitApproval(session) {
    const normalized = normalizeConnectSession(session);
    const injected = await maybeInvoke(
      this.connect?.awaitApproval,
      this.connect,
      normalized,
      this.config,
    );
    let approved = injected;
    if (approved === undefined) {
      const appSession =
        normalized.appSession ??
        createConnectAppSession({
          baseUrl: this.config.connectBaseUrl ?? this.config.toriiBaseUrl ?? this.config.baseUrl,
          preview: normalized.preview,
          session: {
            sid: normalized.sid,
            token_app: normalized.tokenApp,
            token_relay: normalized.tokenRelay,
          },
          appMeta: this.config.appMeta ?? this.config.appMetadata ?? null,
          permissions: this.config.permissions ?? null,
          webSocketImpl: this.config.webSocketImpl,
          allowInsecure: this.config.allowInsecure,
        });
      normalized.appSession = appSession;
      approved = await appSession.waitForApproval();
    }
    const accountIdRaw = approved.accountId ?? approved.account_id;
    if (typeof accountIdRaw !== "string" || accountIdRaw.trim() === "") {
      throw new NexusAppError(
        "approval_missing_account",
        "wallet approval did not include an account",
      );
    }
    const accountId = accountIdRaw.trim();
    const signingPublicKey = this.config.signingPublicKey
      ? validateEd25519PublicKey(
          toBuffer(this.config.signingPublicKey, "config.signingPublicKey"),
          "config.signingPublicKey",
        )
      : approved.signingPublicKey || approved.signing_public_key
        ? validateEd25519PublicKey(
            toBuffer(
              approved.signingPublicKey ?? approved.signing_public_key,
              "approved.signingPublicKey",
            ),
            "approved.signingPublicKey",
          )
        : accountEd25519PublicKey(accountId);
    normalized.approvedAccountId = accountId;
    normalized.approvedAccount = accountId;
    normalized.signingPublicKey = Buffer.from(signingPublicKey);
    return {
      accountId,
      signingPublicKey: Buffer.from(signingPublicKey),
      session: normalized,
    };
  }

  buildTransferDraft(input = {}) {
    const authority =
      input.authority ??
      input.accountId ??
      input.sourceAccountId ??
      this.config.authority ??
      this.config.accountId;
    if (!authority) {
      throw new NexusAppError(
        "missing_authority",
        "transfer authority is required",
      );
    }
    const chainId = requireNonEmptyString(
      input.chainId ?? this.config.chainId,
      "chainId",
    );
    const payloadInput = {
      chainId,
      authority,
      sourceAssetHoldingId:
        input.sourceAssetHoldingId ?? input.sourceAssetId ?? input.assetId,
      quantity: input.quantity,
      destinationAccountId:
        input.destinationAccountId ?? input.destination ?? input.to,
      metadata: input.metadata ?? null,
      creationTimeMs: input.creationTimeMs ?? null,
      ttlMs: input.ttlMs ?? null,
      nonce: input.nonce ?? null,
    };
    const payloadBytes = toBuffer(
      this.transactionCodec?.buildTransferPayload
        ? this.transactionCodec.buildTransferPayload(payloadInput)
        : defaultBuildTransferPayload(payloadInput),
      "payloadBytes",
    );
    const payloadHashHex =
      this.transactionCodec?.payloadHashHex?.(payloadBytes) ??
      irohaPrehashHex(payloadBytes);
    const signingPublicKey =
      input.signingPublicKey ??
      this.config.signingPublicKey ??
      accountEd25519PublicKey(authority);
    return {
      input: { ...payloadInput, signingPublicKey },
      signable: {
        payloadBytes,
        payloadHashHex,
        authority,
        signingPublicKey: signingPublicKey
          ? validateEd25519PublicKey(
              toBuffer(signingPublicKey, "signingPublicKey"),
              "signingPublicKey",
            )
          : null,
        signatureAlgorithm: ALGORITHM_ED25519,
      },
    };
  }

  async requestSignature(session, signable) {
    normalizeAlgorithm(signable.signatureAlgorithm);
    const injected = await maybeInvoke(
      this.connect?.requestSignature,
      this.connect,
      session,
      signable,
      this.config,
    );
    if (injected !== undefined) {
      return normalizeSignature(injected);
    }
    const appSession = session?.appSession;
    if (!appSession || typeof appSession.signTransaction !== "function") {
      throw new NexusAppError(
        "connect_session_unapproved",
        "Connect app session is not approved or cannot sign transactions",
      );
    }
    const signature = await appSession.signTransaction(signable.payloadBytes);
    return normalizeSignature({ algorithm: ALGORITHM_ED25519, signature });
  }

  async finalizeAndSubmit(signable, signature, options = {}) {
    normalizeAlgorithm(signable.signatureAlgorithm);
    const normalizedSignature = normalizeSignature(signature);
    const signingPublicKey =
      signable.signingPublicKey ??
      options.signingPublicKey ??
      this.config.signingPublicKey;
    if (!signingPublicKey) {
      throw new NexusAppError(
        "missing_signing_public_key",
        "signing public key is required to finalize a wallet-signed transaction",
      );
    }
    const publicKey = validateEd25519PublicKey(
      toBuffer(signingPublicKey, "signingPublicKey"),
      "signingPublicKey",
    );
    validateEd25519SignatureForPayload(
      publicKey,
      signable.payloadBytes,
      normalizedSignature.signature,
    );
    const finalized = normalizeFinalizedTransaction(
      this.transactionCodec?.finalizeSignedTransaction
        ? this.transactionCodec.finalizeSignedTransaction(
            signable,
            normalizedSignature,
            publicKey,
          )
        : defaultFinalizeSignedTransaction(
            signable,
            normalizedSignature,
            publicKey,
          ),
    );
    const toriiClient = options.toriiClient ?? this.toriiClient;
    if (!toriiClient || typeof toriiClient.submitTransaction !== "function") {
      throw new NexusAppError(
        "torii_client_unavailable",
        "Torii client is required to submit the signed transaction",
      );
    }
    let submission;
    try {
      submission = await toriiClient.submitTransaction(finalized.signedTransaction);
    } catch (error) {
      throw new NexusAppError(
        "submit_failed",
        `failed to submit signed transfer to Torii: ${error?.message ?? String(error)}`,
        error,
      );
    }
    const submittedHashHex = submissionHashHex(submission);
    if (submittedHashHex && submittedHashHex !== finalized.hashHex) {
      throw new NexusAppError(
        "transaction_hash_mismatch",
        `Torii returned transaction hash ${submittedHashHex} but local hash is ${finalized.hashHex}`,
      );
    }
    let status = null;
    if (options.wait !== false && typeof toriiClient.waitForTransactionStatus === "function") {
      try {
        status = await toriiClient.waitForTransactionStatus(finalized.hashHex, {
          intervalMs: options.intervalMs,
          timeoutMs: options.timeoutMs,
          maxAttempts: options.maxAttempts,
          scope: options.scope,
          successStatuses: options.successStatuses,
          failureStatuses: options.failureStatuses,
          onStatus: options.onStatus,
        });
      } catch (error) {
        throw new NexusAppError(
          "status_wait_failed",
          `failed while waiting for Torii pipeline status: ${error?.message ?? String(error)}`,
          error,
        );
      }
    }
    return {
      signedTransaction: finalized.signedTransaction,
      signedTransactionHashHex: finalized.hashHex,
      submission,
      status,
    };
  }

  async transferWithWallet(session, input, options = {}) {
    const normalizedSession = session ? normalizeConnectSession(session) : {};
    const approvedAccount =
      normalizedSession.approvedAccountId ??
      normalizedSession.approvedAccount ??
      null;
    const authority =
      input.authority ??
      input.accountId ??
      approvedAccount ??
      this.config.authority ??
      this.config.accountId;
    if (approvedAccount && input.authority && approvedAccount !== input.authority) {
      throw new NexusAppError(
        "approval_account_mismatch",
        "transfer authority does not match the approved wallet account",
      );
    }
    const signingPublicKey =
      input.signingPublicKey ??
      normalizedSession.signingPublicKey ??
      this.config.signingPublicKey;
    const draft = this.buildTransferDraft({
      ...input,
      authority,
      signingPublicKey,
    });
    const signature = await this.requestSignature(normalizedSession, draft.signable);
    return this.finalizeAndSubmit(draft.signable, signature, options);
  }
}

export {
  ALGORITHM_ED25519 as NexusSignatureAlgorithmEd25519,
  irohaPrehashHex as nexusPayloadHashHex,
};
