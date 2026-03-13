---
lang: es
direction: ltr
source: docs/connect_ts_wrapper.md
status: complete
translator: manual
source_hash: 57524bc163f78ed08b304d4226616aeb0694ba89ad68b5bb3ce6fa18e868c691
source_last_modified: "2025-11-02T04:40:28.811884+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_ts_wrapper.md (TypeScript Connect Wrapper) -->

## Wrapper TypeScript para Connect (WS join + tokens)

```ts
type SessionResp = { sid: string; wallet_uri: string; app_uri: string; token_app: string; token_wallet: string };

export async function createSession(node: string): Promise<SessionResp> {
  const res = await fetch(`${node}/v2/connect/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' });
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
  const wsUrl = `${node.replace('http', 'ws')}/v2/connect/ws?sid=${sid}&role=${role}`;
  const protocol = `iroha-connect.token.v1.${base64UrlToken(token)}`;
  const ws = new WebSocket(wsUrl, protocol);
  await new Promise<void>((resolve, reject) => { ws.onopen = () => resolve(); ws.onerror = (e) => reject(e); });
  return ws;
}

// Uso:
// const s = await createSession('http://127.0.0.1:8080');
// const ws = await joinWs('http://127.0.0.1:8080', s.sid, 'app', s.token_app);
// // Tras derivar las claves y construir un ConnectFrameV1 (codificado con Norito), enviar:
// ws.send(noritoEncodedBinaryFrame);
```

```ts
// Sellar SignResultOk (sólo la payload; el framing Norito no se muestra):
// const signResult = new TextEncoder().encode(JSON.stringify({ SignResultOk: { signature: { algorithm: 'ed25519', signature_hex: 'deadbeef' } } }));
// const { aead } = await sealEnvelope(kWallet, sidBytes, 'W2A', 1n, signResult);
```

