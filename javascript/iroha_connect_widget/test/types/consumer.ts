// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Compile-only consumer of the published declarations (`npm run typecheck`).

import {
  IROHA_CONNECT_EVENTS,
  IrohaConnectController,
  IrohaConnectError,
  SakuraField,
  acquireSakura,
  encodeQr,
  qrToSvg,
  type IrohaConnectApproval,
  type IrohaConnectState,
} from "@iroha/connect-widget";
import { IROHA_SAKURA_TAG, targetPetalCount } from "@iroha/connect-widget/sakura";
import { CONNECT_STATUSES } from "@iroha/connect-widget/controller";
import { QR_QUIET_ZONE, type QrSymbol } from "@iroha/connect-widget/qr";

const button = document.createElement("iroha-connect");
button.setAttribute("torii-url", "https://taira.sora.org");
button.addEventListener("iroha-connect-approved", (event) => {
  const approval: IrohaConnectApproval = event.detail;
  const accountId: string = approval.accountId;
  void accountId;
});
button.addEventListener("iroha-connect-error", (event) => {
  const code: IrohaConnectError["code"] = event.detail.code;
  void code;
});
button.addEventListener(IROHA_CONNECT_EVENTS.state, (event) => {
  const state: IrohaConnectState = event.detail;
  if (state.status === "awaiting") {
    const uri: string = state.walletUri;
    void uri;
  }
});
button.strings = { trigger: "Connect" };
button.permissions = { methods: ["sign_raw"], resources: ["iroha:torii:canonical-request:v1"] };

async function sign(bytes: Uint8Array): Promise<Uint8Array> {
  await button.connect();
  return button.signTransaction(bytes);
}
void sign;

async function signingKeyGeneration(): Promise<number> {
  const key = await button.accountSigningKey();
  const publicKey: Uint8Array = key.publicKey;
  void publicKey;
  return key.generation;
}
void signingKeyGeneration;

const controller = new IrohaConnectController({
  toriiUrl: "https://taira.sora.org",
  networkId: "hash:00#0000",
  appMeta: { name: "Example" },
});
controller.addEventListener("statechange", (event) => {
  const status: (typeof CONNECT_STATUSES)[number] = event.detail.status;
  void status;
});

const layer: HTMLElementTagNameMap[typeof IROHA_SAKURA_TAG] = document.createElement("iroha-sakura");
const field: SakuraField | null = layer.field;
void field;
const lease = acquireSakura({ mode: "background", backdrop: true, theme: "auto" });
lease.release();
const count: number = targetPetalCount(1280, 800, { density: 0.6 });
void count;

const qr: QrSymbol = encodeQr("iroha://connect?sid=x", { level: "M" });
const svg: string = qrToSvg(qr, { margin: QR_QUIET_ZONE, title: "Pairing code" });
void svg;
