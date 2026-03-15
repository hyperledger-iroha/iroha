---
lang: uz
direction: ltr
source: docs/connect_client_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ecdf23dc61024ae4c509806700773d9b34ddd36076c1182cbeccd3654b29144
source_last_modified: "2026-01-05T18:22:23.392202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha Connect Client misollari (TypeScript va Kotlin)

Ushbu hujjat v0 qoidalarini amalga oshiradigan minimal mijoz snippetlarini ko'rsatadi:
- AEAD AAD tashqi sarlavhani bog'laydi (versiya, sid, dir, seq, kind = shifrlangan matn).
- Nonce `seq` dan olingan (12 bayt IETF: 0x00000000 || seq_le).
- Tasdiqdan keyin boshqaruv ramkalari (yopish/rad etish) shifrlangan holda yuboriladi.

Bular illyustrativ; qattiqlashuv/ishlab chiqarish tekshiruvlari o'tkazib yuborilgan.

### TypeScript (libsodium + WebCrypto)

Bog'liqlar: `libsodium-wrappers` (X25519, BLAKE2b, ChaCha20‑Poly1305), WebCrypto (HKDF‑SHA‑256).

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

  // Create session: POST /v1/connect/session with client-computed sid
  const sidB64 = sodium.to_base64(sid, sodium.base64_variants.URLSAFE_NO_PADDING);
  const node = 'http://localhost:8080';
  const resp = await fetch(`${node}/v1/connect/session`, {
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

Bog'liqliklar:

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
    mac.reset(); mac.init(SecretKeySpec(prk, "HmacSHA256"))
    mac.update(t); mac.update(info); mac.update(byteArrayOf(i.toByte()))
    t = mac.doFinal(); val take = minOf(32, len - pos); System.arraycopy(t, 0, okm, pos, take); pos += take
  }
  return okm
}

fun aadV1(sid: ByteArray, dir: Byte, seq: Long): ByteArray {
  val out = ByteArray(8 + 32 + 1 + 8 + 1)
  System.arraycopy("connect:v1".toByteArray(), 0, out, 0, 8)
  System.arraycopy(sid, 0, out, 8, 32); out[40] = dir
  val bb = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq).array()
  System.arraycopy(bb, 0, out, 41, 8); out[49] = 1; return out
}

fun nonceFromSeq(seq: Long): ByteArray {
  val n = ByteArray(12)
  val le = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq).array()
  System.arraycopy(le, 0, n, 4, 8); return n
}

fun sealChaCha20Poly1305(key: ByteArray, nonce: ByteArray, aad: ByteArray, pt: ByteArray): ByteArray {
  val cipher = Cipher.getInstance("ChaCha20-Poly1305")
  cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "ChaCha20"), javax.crypto.spec.IvParameterSpec(nonce))
  cipher.updateAAD(aad); return cipher.doFinal(pt)
}

fun openChaCha20Poly1305(key: ByteArray, nonce: ByteArray, aad: ByteArray, ct: ByteArray): ByteArray {
  val cipher = Cipher.getInstance("ChaCha20-Poly1305")
  cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "ChaCha20"), javax.crypto.spec.IvParameterSpec(nonce))
  cipher.updateAAD(aad); return cipher.doFinal(ct)
}

fun main() {
  val kpg = KeyPairGenerator.getInstance("XDH"); kpg.initialize(NamedParameterSpec("X25519"))
  val appKp = kpg.generateKeyPair()
  val chainId = "testnet".toByteArray(); val nonce = SecureRandom().generateSeed(16)
  val sid = blake2b256("iroha-connect|sid|".toByteArray(), chainId, appKp.public.encoded, nonce)
  // Create session: POST /v1/connect/session with client-computed sid
  val sidB64 = Base64.getUrlEncoder().withoutPadding().encodeToString(sid)
  val node = "http://localhost:8080"
  val client = HttpClient.newHttpClient()
  val json = "{" + "\"sid\":\"" + sidB64 + "\",\"node\":\"" + node + "\"}"
  val req = HttpRequest.newBuilder()
      .uri(URI.create("$node/v1/connect/session"))
      .header("Content-Type", "application/json")
      .POST(HttpRequest.BodyPublishers.ofString(json))
      .build()
  val resp = client.send(req, HttpResponse.BodyHandlers.ofString())
  require(resp.statusCode() == 200) { "session create failed: ${resp.statusCode()}" }
  val body = resp.body()
  val tokenApp = Regex("\"token_app\"\\s*:\\s*\"([^\"]+)\"").find(body)?.groupValues?.get(1)
      ?: error("token_app missing")
  val tokenWallet = Regex("\"token_wallet\"\\s*:\\s*\"([^\"]+)\"").find(body)?.groupValues?.get(1)
      ?: error("token_wallet missing")
  val ka = KeyAgreement.getInstance("XDH"); ka.init(appKp.private); ka.doPhase(appKp.public, true) // replace with wallet pk
  val shared = ka.generateSecret()
  val salt = blake2b256("iroha-connect|salt|".toByteArray(), sid)
  val kApp = hkdfSha256(shared, salt, "iroha-connect|k_app".toByteArray(), 32)
  val aad = aadV1(sid, 0, 1L); val n12 = nonceFromSeq(1L)
  val pt = "{\"SignRequestTx\":{\"tx_bytes\":\"...\"}}".toByteArray()
  val ct = sealChaCha20Poly1305(kApp, n12, aad, pt)
  val opened = openChaCha20Poly1305(kApp, n12, aad, ct)
  require(java.util.Arrays.equals(opened, pt))
}
```

Eslatmalar:
- Mijoz `sid` (32 bayt; base64url/hex) hisoblaydi va bir martalik tokenlarni olish uchun uni `/v1/connect/session` ga POST qiladi; server `sid` aks sadolari. WS ga `Authorization: Bearer <token>` yoki `Sec-WebSocket-Protocol: iroha-connect.token.v1.<base64url(token)>` bilan qo'shiling.
- Kalitlar mavjud bo'lgandan keyin (Tasdiqlash), shifrlangan foydali yuklarda Yopish/Rad qilish yuboring.
- Dedupe kalitlari va `seq` ilova/hamyon ramkalari uchun har bir yo'nalishda monoton bo'lishi kerak; `Envelope.seq == frame.seq`. Server hodisalari alohida server tomoni ketma-ketligidan foydalanadi va AEAD/dedupe dan chiqarib tashlanadi.