---
lang: pt
direction: ltr
source: docs/connect_kotlin_ws.md
status: complete
translator: manual
source_hash: f41226862739179daa422b783af0c5df3367f52c804d2b22b57f94a0972a4b1a
source_last_modified: "2025-11-02T04:40:28.809023+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/connect_kotlin_ws.md (Kotlin WS Join with Tokens) -->

## Conexão WebSocket em Kotlin com Tokens (OkHttp)

Dependências (Gradle):
```kotlin
dependencies {
  implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
```

```kotlin
import okhttp3.*

fun joinWs(node: String, sid: String, role: String, token: String, listener: WebSocketListener): WebSocket {
  val http = node.replace("http", "ws")
  val url = "$http/v2/connect/ws?sid=$sid&role=$role"
  val req = Request.Builder()
    .url(url)
    .addHeader("Authorization", "Bearer $token")
    .build()
  val client = OkHttpClient()
  return client.newWebSocket(req, listener)
}

// Exemplo de uso:
// val ws = joinWs("http://127.0.0.1:8080", sid, "app", tokenApp, object: WebSocketListener() {
//   override fun onOpen(webSocket: WebSocket, response: Response) { println("open") }
//   override fun onMessage(webSocket: WebSocket, bytes: ByteString) { println("binary ${'$'}{bytes.size}") }
// })
```

### Selando/abrindo payloads (ChaCha20‑Poly1305 + AAD)

```kotlin
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
import javax.crypto.Mac
import org.bouncycastle.crypto.digests.Blake2bDigest

fun blake2b256(vararg parts: ByteArray): ByteArray { val d = Blake2bDigest(256); for (p in parts) d.update(p, 0, p.size); val o = ByteArray(32); d.doFinal(o, 0); return o }
fun hkdfSha256(ikm: ByteArray, salt: ByteArray, info: ByteArray, len: Int): ByteArray {
  val mac = Mac.getInstance("HmacSHA256"); mac.init(SecretKeySpec(salt, "HmacSHA256"))
  val prk = mac.doFinal(ikm)
  val out = ByteArray(len); var t = ByteArray(0); var pos = 0
  val rounds = (len + 31) / 32
  for (i in 1..rounds) { mac.reset(); mac.init(SecretKeySpec(prk, "HmacSHA256")); mac.update(t); mac.update(info); mac.update(byteArrayOf(i.toByte())); t = mac.doFinal(); val take = minOf(32, len - pos); System.arraycopy(t, 0, out, pos, take); pos += take }
  return out
}
fun aadV1(sid: ByteArray, dir: Byte, seq: Long): ByteArray {
  val out = ByteArray(8 + 32 + 1 + 8 + 1)
  System.arraycopy("connect:v1".toByteArray(), 0, out, 0, 8); System.arraycopy(sid, 0, out, 8, 32); out[40] = dir
  val le = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq).array()
  System.arraycopy(le, 0, out, 41, 8); out[49] = 1; return out
}
fun nonceFromSeq(seq: Long): ByteArray { val n = ByteArray(12); val le = java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(seq).array(); System.arraycopy(le, 0, n, 4, 8); return n }
fun sealChaCha20Poly1305(key: ByteArray, nonce: ByteArray, aad: ByteArray, pt: ByteArray): ByteArray { val c = Cipher.getInstance("ChaCha20-Poly1305"); c.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "ChaCha20"), javax.crypto.spec.IvParameterSpec(nonce)); c.updateAAD(aad); return c.doFinal(pt) }
fun openChaCha20Poly1305(key: ByteArray, nonce: ByteArray, aad: ByteArray, ct: ByteArray): ByteArray { val c = Cipher.getInstance("ChaCha20-Poly1305"); c.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "ChaCha20"), javax.crypto.spec.IvParameterSpec(nonce)); c.updateAAD(aad); return c.doFinal(ct) }

// Selando SignResultOk wallet → app em seq=1 (apenas payload; o framing Norito não é mostrado)
fun sealSignResultOk(kWallet: ByteArray, sid: ByteArray): ByteArray {
  val payload = "{\"SignResultOk\":{\"signature\":{\"algorithm\":\"ed25519\",\"signature_hex\":\"deadbeef\"}}}".toByteArray()
  val aad = aadV1(sid, 1, 1L) // dir=WalletToApp
  val nonce = nonceFromSeq(1L)
  return sealChaCha20Poly1305(kWallet, nonce, aad, payload)
}
```

