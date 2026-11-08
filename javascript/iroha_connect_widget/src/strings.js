// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/** User-visible IrohaConnect copy. `{app}`, `{account}` and `{message}` are filled in at render time. */
export const STRINGS = Object.freeze({
  en: Object.freeze({
    trigger: "IrohaConnect",
    triggerLabel: "Connect a wallet with IrohaConnect",
    connectedLabel: "IrohaConnect: connected as {account}",
    eyebrow: "IrohaConnect",
    title: "Connect to {app}",
    accountTitle: "Wallet connected",
    ledePreparing: "Creating a secure pairing session…",
    ledeAwaiting: "Scan this code with your Iroha wallet, or open the wallet on this device.",
    ledeConnected: "Your wallet approved the connection.",
    ledeError: "Pairing did not finish. Start again with a new code.",
    ledeAccount: "Requests from this app are signed in your wallet, never here.",
    ledeConfig: "This IrohaConnect button is not configured.",
    statusPreparing: "Preparing session",
    statusAwaiting: "Waiting for wallet approval",
    statusConnected: "Connected",
    qrLabel: "IrohaConnect pairing code",
    openWallet: "Open wallet",
    copyLink: "Copy link",
    copied: "Copied",
    retry: "New code",
    cancel: "Cancel",
    close: "Close",
    disconnect: "Disconnect",
    copyAccount: "Copy account",
    account: "Account",
    network: "Network",
    errorRejected: "The wallet declined the connection.",
    errorClosed: "The session closed before the wallet approved it.",
    errorNetwork: "Torii could not be reached: {message}",
    errorConfig: "{message}",
    errorFailed: "IrohaConnect could not continue: {message}",
  }),
  ja: Object.freeze({
    trigger: "IrohaConnect",
    triggerLabel: "IrohaConnect でウォレットを接続",
    connectedLabel: "IrohaConnect: {account} として接続中",
    eyebrow: "IrohaConnect",
    title: "{app} に接続",
    accountTitle: "ウォレット接続中",
    ledePreparing: "安全なペアリングセッションを作成しています…",
    ledeAwaiting: "Iroha ウォレットでこのコードを読み取るか、この端末でウォレットを開いてください。",
    ledeConnected: "ウォレットが接続を承認しました。",
    ledeError: "ペアリングが完了しませんでした。新しいコードでやり直してください。",
    ledeAccount: "このアプリからのリクエストはウォレットで署名され、ここでは署名されません。",
    ledeConfig: "この IrohaConnect ボタンは設定されていません。",
    statusPreparing: "セッションを準備中",
    statusAwaiting: "ウォレットの承認を待っています",
    statusConnected: "接続しました",
    qrLabel: "IrohaConnect ペアリングコード",
    openWallet: "ウォレットを開く",
    copyLink: "リンクをコピー",
    copied: "コピーしました",
    retry: "新しいコード",
    cancel: "キャンセル",
    close: "閉じる",
    disconnect: "切断",
    copyAccount: "アカウントをコピー",
    account: "アカウント",
    network: "ネットワーク",
    errorRejected: "ウォレットが接続を拒否しました。",
    errorClosed: "ウォレットの承認前にセッションが終了しました。",
    errorNetwork: "Torii に接続できませんでした: {message}",
    errorConfig: "{message}",
    errorFailed: "IrohaConnect を続行できませんでした: {message}",
  }),
});

/** Pick the built-in language for a BCP 47 tag (Japanese or English). */
export function languageFor(tag) {
  return typeof tag === "string" && /^ja(?:-|$)/iu.test(tag.trim()) ? "ja" : "en";
}

/** Fill `{name}` placeholders from `values`; unknown placeholders stay empty. */
export function formatString(template, values = {}) {
  return String(template).replace(/\{(\w+)\}/gu, (_match, key) =>
    values[key] === undefined || values[key] === null ? "" : String(values[key]),
  );
}
