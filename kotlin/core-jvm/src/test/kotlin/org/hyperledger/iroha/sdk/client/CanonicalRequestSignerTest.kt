package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CanonicalRequestSignerTest {
    @Test
    fun unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields() {
        val body = linkedMapOf<String, Any?>(
            "z" to "last",
            CanonicalRequestSigner.BODY_SIGNATURE_BASE64 to "remove",
            "nested" to linkedMapOf(CanonicalRequestSigner.BODY_SIGNATURE_BASE64 to "keep"),
            CanonicalRequestSigner.BODY_WITNESS_BASE64 to "remove-too",
            CanonicalRequestSigner.BODY_ACCOUNT_ID to "alice",
            CanonicalRequestSigner.BODY_TIMESTAMP_MS to 7L,
            CanonicalRequestSigner.BODY_NONCE to "n",
        )

        val unsigned = String(CanonicalRequestSigner.unsignedBodyAuthJson(body), StandardCharsets.UTF_8)

        assertEquals(
            """{"account_id":"alice","nested":{"signature_base64":"keep"},"nonce":"n","timestamp_ms":7,"z":"last"}""",
            unsigned,
        )
    }

    @Test
    fun bodySignatureFieldsCarryVerifiableSignature() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val uri = URI.create("https://torii.example/v1/offline/keys/refill?b=2&a=1")
        val timestampMs = 1_717_171_717_000L
        val nonce = "offline-body-nonce"
        val body = linkedMapOf<String, Any?>("operation_id" to "operation-1")

        val signed = CanonicalRequestSigner.withBodySignature(
            "post",
            uri,
            body,
            "alice",
            keyPair.private,
            timestampMs,
            nonce,
        )

        assertEquals("alice", signed[CanonicalRequestSigner.BODY_ACCOUNT_ID])
        assertEquals(timestampMs, signed[CanonicalRequestSigner.BODY_TIMESTAMP_MS])
        assertEquals(nonce, signed[CanonicalRequestSigner.BODY_NONCE])
        assertFalse(signed.containsKey(CanonicalRequestSigner.BODY_WITNESS_BASE64))

        val signatureBytes = Base64.getDecoder()
            .decode(signed[CanonicalRequestSigner.BODY_SIGNATURE_BASE64] as String)
        val message = CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "post",
            uri,
            signed,
            timestampMs,
            nonce,
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(message)
        assertTrue(verifier.verify(signatureBytes))
    }
}
