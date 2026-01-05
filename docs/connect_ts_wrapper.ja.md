<!-- Japanese translation of docs/connect_ts_wrapper.md -->

---
lang: ja
direction: ltr
source: docs/connect_ts_wrapper.md
status: complete
translator: manual
---

## TypeScript Connect ラッパー（WS 参加 + トークン）

```ts
type SessionResp = { sid: string; wallet_uri: string; app_uri: string; token_app: string; token_wallet: string };

export async function createSession(node: string): Promise<SessionResp> {
  const res = await fetch(`${node}/v1/connect/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' });
  if (!res.ok) throw new Error(`session: ${res.status}`);
  return res.json();
}

function base64UrlToken(token: string): string {
  const raw = typeof btoa === 'function'
    ? btoa(token)
    : Buffer.from(token, 'utf8').toString('base64');
  return raw.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

export async function joinWs(node: string, sid: string, role: 'app'|'wallet', token: string): Promise<WebSocket> {
  const wsUrl = `${node.replace('http', 'ws')}/v1/connect/ws?sid=${sid}&role=${role}`;
  const protocol = `iroha-connect.token.v1.${base64UrlToken(token)}`;
  const ws = new WebSocket(wsUrl, protocol);
  await new Promise<void>((resolve, reject) => { ws.onopen = () => resolve(); ws.onerror = (e) => reject(e); });
  return ws;
}

// 用法
// const s = await createSession('http://127.0.0.1:8080');
// const ws = await joinWs('http://127.0.0.1:8080', s.sid, 'app', s.token_app);
// // キー導出と ConnectFrameV1（Norito エンコード）を完了したら送信:
// ws.send(noritoEncodedBinaryFrame);
```
// SignResultOk を暗号化（ペイロードのみ。Norito フレーミングは未提示）:
// const signResult = new TextEncoder().encode(JSON.stringify({ SignResultOk: { signature: { algorithm: 'ed25519', signature_hex: 'deadbeef' } } }));
// const { aead } = await sealEnvelope(kWallet, sidBytes, 'W2A', 1n, signResult);
