---
lang: az
direction: ltr
source: docs/connect_ts_wrapper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d8194c964c8c4d0592db6f119593b79ff768d40f2c55a219ab50e714832d362
source_last_modified: "2026-01-05T18:22:23.397739+00:00"
translation_last_reviewed: 2026-02-07
---

## TypeScript Connect Wrapper (WS join + tokens)

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

// Usage
// const s = await createSession('http://127.0.0.1:8080');
// const ws = await joinWs('http://127.0.0.1:8080', s.sid, 'app', s.token_app);
// // After deriving keys and building a ConnectFrameV1 (Norito-encoded), send:
// ws.send(noritoEncodedBinaryFrame);
```
// Sealing SignResultOk (payload only; Norito framing not shown):
// const signResult = new TextEncoder().encode(JSON.stringify({ SignResultOk: { signature: { algorithm: 'ed25519', signature_hex: 'deadbeef' } } }));
// const { aead } = await sealEnvelope(kWallet, sidBytes, 'W2A', 1n, signResult);
