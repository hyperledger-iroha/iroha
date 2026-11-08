// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * A simulated IrohaConnect wallet for the offline playground.
 *
 * It replaces the SDK bindings of one `<iroha-connect>` element so designers
 * and front-end developers can walk through every state without Torii or a
 * wallet. Production pages never set `element.sdk`.
 */

function randomToken(bytes = 32) {
  const values = crypto.getRandomValues(new Uint8Array(bytes));
  return btoa(String.fromCharCode(...values)).replace(/\+/gu, "-").replace(/\//gu, "_").replace(/=+$/u, "");
}

export function createSimulatedWallet({ onChange = () => undefined } = {}) {
  let current = null;
  let counter = 0;

  const sdk = {
    NetworkId: { parse: (literal) => ({ literal, toString: () => literal }) },
    createConnectSessionPreview: ({ networkId, node }) => ({ networkId, node }),
    async registerConnectSession(_baseUrl, preview) {
      await new Promise((resolve) => setTimeout(resolve, 700));
      counter += 1;
      const query = new URLSearchParams({
        sid: randomToken(),
        network_id: preview.networkId.literal,
        app_pk: randomToken(),
        nonce: randomToken(16),
        node: preview.node,
        v: "1",
        role: "wallet",
        token: randomToken(),
        relay: randomToken(),
      });
      return {
        sid: query.get("sid"),
        wallet_uri: `iroha://connect?${query}`,
        app_uri: `iroha://connect?${query}`.replace("role=wallet", "role=app"),
        token_management: `simulated-${counter}`,
      };
    },
    resolveConnectLaunchUri: (_role, _preview, session) => session.wallet_uri,
    resolveConnectLaunchUriForProtocol: (_role, _preview, session, protocol) =>
      session.wallet_uri.replace(/^iroha:/u, `${protocol}:`),
    createAccountInspector: () => async () => ({}),
    createConnectAppSession() {
      const socket = new EventTarget();
      let resolve;
      let reject;
      const approval = new Promise((onResolve, onReject) => {
        resolve = onResolve;
        reject = onReject;
      });
      approval.catch(() => undefined);
      const closed = (reason) => Object.assign(new Error(reason), { name: "ConnectSessionClosedError" });
      current = {
        socket,
        waitForApproval: () => approval,
        signTransaction: async (bytes) => Uint8Array.from(bytes),
        signRaw: async (_tag, bytes) => Uint8Array.from(bytes),
        accountSigningKey: async () => {
          await approval;
          return { publicKey: new Uint8Array(32), generation: 1 };
        },
        close(reason) {
          reject(closed(reason));
          current = null;
          onChange();
        },
        approve(accountId) {
          resolve({
            accountId,
            signingPublicKey: new Uint8Array(32),
            walletPublicKey: new Uint8Array(32),
            signature: new Uint8Array(64),
          });
        },
        decline() {
          reject(Object.assign(new Error("declined in the wallet"), { name: "ConnectApprovalRejectedError" }));
        },
        closeFromWallet() {
          reject(closed("closed by the wallet"));
          socket.dispatchEvent(new Event("close"));
        },
      };
      onChange();
      return current;
    },
    deleteConnectSession: async () => undefined,
  };

  return {
    sdk,
    get session() {
      return current;
    },
  };
}
