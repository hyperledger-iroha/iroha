// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * In-memory stand-in for the `@iroha/iroha-js` Connect bindings. Each app
 * session exposes `approve()`, `reject()` and `remoteClose()` so tests can
 * play the wallet's part; every SDK call is recorded.
 */
export function createFakeSdk(options = {}) {
  const calls = { preview: [], register: [], appSession: [], inspector: [], delete: [] };
  const sessions = [];
  let registrations = 0;

  const sdk = {
    NetworkId: {
      parse(literal) {
        if (!String(literal).startsWith("hash:")) {
          throw new Error(`invalid NetworkId ${literal}`);
        }
        return Object.freeze({ literal, toString: () => literal });
      },
    },
    createConnectSessionPreview(previewOptions) {
      calls.preview.push(previewOptions);
      return { sidBase64Url: `sid-${registrations + 1}`, walletUri: "iroha://connect?preview", appUri: "iroha://connect?preview-app" };
    },
    async registerConnectSession(baseUrl, preview, registerOptions) {
      calls.register.push({ baseUrl, preview, options: registerOptions });
      if (options.register) {
        return options.register(baseUrl, preview, registerOptions);
      }
      registrations += 1;
      const sid = `sid-${registrations}`;
      return {
        sid,
        wallet_uri: `iroha://connect?sid=${sid}&role=wallet&token=wallet-${registrations}&relay=relay-${registrations}`,
        app_uri: `iroha://connect?sid=${sid}&role=app`,
        token_app: `app-${registrations}`,
        token_wallet: `wallet-${registrations}`,
        token_management: `management-${registrations}`,
        token_relay: `relay-${registrations}`,
      };
    },
    resolveConnectLaunchUri(role, _preview, session) {
      return role === "wallet" ? session.wallet_uri : session.app_uri;
    },
    resolveConnectLaunchUriForProtocol(role, preview, session, protocol) {
      return sdk.resolveConnectLaunchUri(role, preview, session).replace(/^iroha:/u, `${protocol}:`);
    },
    createAccountInspector(inspectorOptions) {
      calls.inspector.push(inspectorOptions);
      return async () => ({});
    },
    createConnectAppSession(sessionOptions) {
      calls.appSession.push(sessionOptions);
      const socket = new EventTarget();
      let resolveApproval;
      let rejectApproval;
      const approval = new Promise((resolve, reject) => {
        resolveApproval = resolve;
        rejectApproval = reject;
      });
      approval.catch(() => undefined);
      const closeError = (reason) => Object.assign(new Error(reason), { name: "ConnectSessionClosedError" });
      const session = {
        socket,
        closedWith: [],
        signed: [],
        waitForApproval: () => approval,
        async signTransaction(bytes) {
          session.signed.push(["tx", bytes]);
          return Uint8Array.of(0xaa, ...bytes);
        },
        async signRaw(domainTag, bytes) {
          session.signed.push(["raw", domainTag, bytes]);
          return Uint8Array.of(0xbb, ...bytes);
        },
        async accountSigningKey() {
          await approval;
          return { publicKey: new Uint8Array(32).fill(5), generation: 1 };
        },
        close(reason) {
          session.closedWith.push(reason);
          rejectApproval(closeError(reason));
          socket.dispatchEvent(new Event("close"));
        },
        approve(accountId = "sorau7xDemoAccount000000000000000000000000000000000") {
          resolveApproval({
            accountId,
            signingPublicKey: new Uint8Array(32).fill(7),
            walletPublicKey: new Uint8Array(32).fill(9),
            signature: new Uint8Array(64),
          });
        },
        reject(error) {
          rejectApproval(error);
        },
        remoteClose() {
          rejectApproval(closeError("connect socket closed"));
          socket.dispatchEvent(new Event("close"));
        },
      };
      sessions.push(session);
      return session;
    },
    async deleteConnectSession(baseUrl, sid, deleteOptions) {
      calls.delete.push({ baseUrl, sid, options: deleteOptions });
    },
    async createConnectCanonicalRequestAuth(session) {
      // Like the SDK, canonical-request auth signs through the account's signing key.
      const { generation } = await session.accountSigningKey();
      return {
        authAccountId: session === sessions.at(-1) ? "approved" : "other",
        signingKeyGeneration: generation,
        sign: async () => new Uint8Array(64),
      };
    },
  };
  return { sdk, calls, sessions };
}

/** Error shaped like the SDK's `ConnectApprovalRejectedError`. */
export function walletRejection(reason = "user declined") {
  return Object.assign(new Error(reason), { name: "ConnectApprovalRejectedError" });
}
