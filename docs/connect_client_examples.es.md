---
lang: es
direction: ltr
source: docs/connect_client_examples.md
status: complete
translator: manual
source_hash: 5e4b2d737a5f4e676271a905e7aab0a6d7c4e5bbd86a8a7f4f9a6bcf74d0a0f2
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_client_examples.md (Iroha Connect Client Examples) -->

## Ejemplos de Cliente Iroha Connect (TypeScript y Kotlin)

Este documento muestra snippets mínimos del lado cliente que implementan las
reglas v0:
- El AAD de AEAD vincula la cabecera externa (versión, `sid`, `dir`, `seq`,
  `kind=Ciphertext`).
- El nonce se deriva de `seq` (nonce IETF de 12 bytes:
  `0x00000000 || seq_le`).
- Los control frames Post‑Approve (Close/Reject) se envían cifrados.

Son ejemplos ilustrativos; se omiten refuerzos y comprobaciones de producción.

### TypeScript (libsodium + WebCrypto)

Dependencias: `libsodium-wrappers` (X25519, BLAKE2b, ChaCha20‑Poly1305),
WebCrypto (HKDF‑SHA‑256).

```ts
import sodium from 'libsodium-wrappers';

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const len = parts.reduce((a, b) => a + b.length, 0);
  const out = new Uint8Array(len);
  let o = 0; for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

async function hkdfSha256(ikm: Uint8Array, salt: Uint8Array, info: Uint8Array, len: number): Promise<Uint8Array> {
  const ikmKey = await crypto.subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits({ name: 'HKDF', hash: 'SHA-256', salt, info }, ikmKey, len * 8);
  return new Uint8Array(bits);
}

function aadV1(sid: Uint8Array, dir: 'A2W'|'W2A', seq: bigint): Uint8Array {
  const out = new Uint8Array(8 + 32 + 1 + 8 + 1);
  out.set(new TextEncoder().encode('connect:v1'), 0);
  out.set(sid, 8);
  out[40] = dir === 'A2W' ? 0 : 1;
  const dv = new DataView(new ArrayBuffer(8)); dv.setBigUint64(0, seq, true);
  out.set(new Uint8Array(dv.buffer), 41);
  out[49] = 1; // Ciphertext
  return out;
}

function nonceFromSeq(seq: bigint): Uint8Array {
  const n = new Uint8Array(12);
  const dv = new DataView(new ArrayBuffer(8)); dv.setBigUint64(0, seq, true);
  n.set(new Uint8Array(dv.buffer), 4);
  return n;
}

async function sealEnvelope(
  k: Uint8Array, sid: Uint8Array, dir: 'A2W'|'W2A', seq: bigint, payload: Uint8Array,
) {
  const aad = aadV1(sid, dir, seq);
  const nonce = nonceFromSeq(seq);
  const aead = sodium.crypto_aead_chacha20poly1305_ietf_encrypt(payload, aad, null, nonce, k);
  return { aead, aad, nonce };
}

async function openEnvelope(k: Uint8Array, sid: Uint8Array, dir: 'A2W'|'W2A', seq: bigint, aead: Uint8Array) {
  const aad = aadV1(sid, dir, seq);
  const nonce = nonceFromSeq(seq);
  return sodium.crypto_aead_chacha20poly1305_ietf_decrypt(null, aead, aad, nonce, k);
}

(async () => { await sodium.ready;
  const app = sodium.crypto_kx_keypair();
  const chainId = new TextEncoder().encode('testnet');
  const nonce = sodium.randombytes_buf(16);
  const sid = sodium.crypto_generichash(32, concatBytes(new TextEncoder().encode('iroha-connect|sid|'), chainId, app.publicKey, nonce));

  // Crear sesión: POST /v2/connect/session con sid calculado en el cliente
  const sidB64 = sodium.to_base64(sid, sodium.base64_variants.URLSAFE_NO_PADDING);
  const node = 'http://localhost:8080';
  const resp = await fetch(`${node}/v2/connect/session`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sid: sidB64, node })
  });
  if (!resp.ok) throw new Error(`session create failed: ${resp.status}`);
  const { token_app, token_wallet, wallet_uri, app_uri } = await resp.json();

  const shared = sodium.crypto_scalarmult(app.privateKey, /* wallet_pk */ app.publicKey);
  const salt = sodium.crypto_generichash(32, concatBytes(new TextEncoder().encode('iroha-connect|salt|'), sid));
  const kApp = await hkdfSha256(shared, salt, new TextEncoder().encode('iroha-connect|k_app'), 32);

  const payload = new TextEncoder().encode(JSON.stringify({ SignRequestTx: { tx_bytes: '...' } }));
  const { aead } = await sealEnvelope(kApp, sid, 'A2W', 1n, payload);
  const pt = await openEnvelope(kApp, sid, 'A2W', 1n, aead);
  console.log('ok len=', pt.length);
})();
```

### Kotlin (JDK 11 + BouncyCastle)

Dependencias:

```kotlin
dependencies { implementation("org.bouncycastle:bcprov-jdk15on:1.78.1") }
```

```kotlin
import java.security.*
import java.security.spec.NamedParameterSpec
import javax.crypto.*
import javax.crypto.spec.SecretKeySpec
import org.bouncycastle.crypto.digests.Blake2bDigest
import javax.crypto.Mac
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.net.URI
import java.util.Base64

fun blake2b256(vararg parts: ByteArray): ByteArray {
  val d = Blake2bDigest(256); for (p in parts) d.update(p, 0, p.size); val out = ByteArray(32); d.doFinal(out, 0); return out
}

fun hkdfSha256(ikm: ByteArray, salt: ByteArray, info: ByteArray, len: Int): ByteArray {
  val mac = Mac.getInstance("HmacSHA256"); mac.init(SecretKeySpec(salt, "HmacSHA256"))
  val prk = mac.doFinal(ikm)
  val rounds = (len + 31) / 32
  var t = ByteArray(0)
  val okm = ByteArray(len)
  var pos = 0
  for (i in 1..rounds) {
    mac.reset()
    mac.init(SecretKeySpec(prk, "HmacSHA256"))
    val input = t + info + i.toByte()
    t = mac.doFinal(input)
    val toCopy = minOf(32, len - pos)
    System.arraycopy(t, 0, okm, pos, toCopy)
    pos += toCopy
  }
  return okm
}

fun aadV1(sid: ByteArray, dir: String, seq: Long): ByteArray {
  val out = ByteArray(8 + 32 + 1 + 8 + 1)
  val prefix = "connect:v1".toByteArray()
  System.arraycopy(prefix, 0, out, 0, 8)
  System.arraycopy(sid, 0, out, 8, 32)
  out[40] = if (dir == "A2W") 0 else 1
  val bb = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq)
  System.arraycopy(bb.array(), 0, out, 41, 8)
  out[49] = 1 // Ciphertext
  return out
}

fun nonceFromSeq(seq: Long): ByteArray {
  val n = ByteArray(12)
  val bb = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq)
  System.arraycopy(bb.array(), 0, n, 4, 8)
  return n
}

fun sealEnvelope(
  k: ByteArray, sid: ByteArray, dir: String, seq: Long, payload: ByteArray
): Pair<ByteArray, ByteArray> {
  val aad = aadV1(sid, dir, seq)
  val nonce = nonceFromSeq(seq)
  val cipher = Cipher.getInstance("ChaCha20-Poly1305")
  val keySpec = SecretKeySpec(k, "ChaCha20")
  val ivSpec = javax.crypto.spec.IvParameterSpec(nonce)
  cipher.init(Cipher.ENCRYPT_MODE, keySpec, ivSpec)
  cipher.updateAAD(aad)
  val aead = cipher.doFinal(payload)
  return Pair(aead, aad)
}

fun openEnvelope(
  k: ByteArray, sid: ByteArray, dir: String, seq: Long, aead: ByteArray
): ByteArray {
  val aad = aadV1(sid, dir, seq)
  val nonce = nonceFromSeq(seq)
  val cipher = Cipher.getInstance("ChaCha20-Poly1305")
  val keySpec = SecretKeySpec(k, "ChaCha20")
  val ivSpec = javax.crypto.spec.IvParameterSpec(nonce)
  cipher.init(Cipher.DECRYPT_MODE, keySpec, ivSpec)
  cipher.updateAAD(aad)
  return cipher.doFinal(aead)
}

fun main() {
  val kpg = KeyPairGenerator.getInstance("X25519")
  kpg.initialize(NamedParameterSpec("X25519"))
  val appKp = kpg.generateKeyPair()

  val chainId = "testnet".toByteArray()
  val nonce = SecureRandom().generateSeed(16)
  val sid = blake2b256("iroha-connect|sid|".toByteArray(), chainId, appKp.public.encoded, nonce)

  val sidB64 = Base64.getUrlEncoder().withoutPadding().encodeToString(sid)
  val node = "http://localhost:8080"
  val client = HttpClient.newHttpClient()
  val req = HttpRequest.newBuilder()
    .uri(URI.create("$node/v2/connect/session"))
    .POST(HttpRequest.BodyPublishers.ofString("{\"sid\":\"$sidB64\",\"node\":\"$node\"}"))
    .header("content-type", "application/json")
    .build()
  val resp = client.send(req, HttpResponse.BodyHandlers.ofString())
  if (resp.statusCode() != 200) error("session create failed: ${resp.statusCode()}")

  val shared = KeyAgreement.getInstance("X25519").apply {
    init(appKp.private)
    doPhase(appKp.public, true) // substituir por chave pública da wallet
  }.generateSecret()

  val salt = blake2b256("iroha-connect|salt|".toByteArray(), sid)
  val kApp = hkdfSha256(shared, salt, "iroha-connect|k_app".toByteArray(), 32)

  val payload = """{"SignRequestTx":{"tx_bytes":"..."}}""".toByteArray()
  val (aead, _) = sealEnvelope(kApp, sid, "A2W", 1L, payload)
  val pt = openEnvelope(kApp, sid, "A2W", 1L, aead)
  println("ok len=${pt.size}")
}
```

