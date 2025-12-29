<!-- Hebrew translation of docs/connect_ts_wrapper.md -->

---
lang: he
direction: rtl
source: docs/connect_ts_wrapper.md
status: complete
translator: manual
---

<div dir="rtl">

## מעטפת TypeScript ל-Connect (הצטרפות WS + טוקנים)

```ts
type SessionResp = { sid: string; wallet_uri: string; app_uri: string; token_app: string; token_wallet: string };

export async function createSession(node: string): Promise<SessionResp> {
  const res = await fetch(`${node}/v1/connect/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' });
  if (!res.ok) throw new Error(`session: ${res.status}`);
  return res.json();
}

export async function joinWs(node: string, sid: string, role: 'app'|'wallet', token: string): Promise<WebSocket> {
  const wsUrl = `${node.replace('http', 'ws')}/v1/connect/ws?sid=${sid}&role=${role}&token=${token}`;
  const ws = new WebSocket(wsUrl);
  await new Promise<void>((resolve, reject) => { ws.onopen = () => resolve(); ws.onerror = (e) => reject(e); });
  return ws;
}

// שימוש:
// const s = await createSession('http://127.0.0.1:8080');
// const ws = await joinWs('http://127.0.0.1:8080', s.sid, 'app', s.token_app);
// // לאחר גזירת מפתחות ובניית ConnectFrameV1 (מקודד Norito) שלחו:
// ws.send(noritoEncodedBinaryFrame);
```
// הצפנת SignResultOk (מטען בלבד; מסגור Norito לא מוצג):
// const signResult = new TextEncoder().encode(JSON.stringify({ SignResultOk: { signature: { algorithm: 'ed25519', signature_hex: 'deadbeef' } } }));
// const { aead } = await sealEnvelope(kWallet, sidBytes, 'W2A', 1n, signResult);

</div>
</div>
