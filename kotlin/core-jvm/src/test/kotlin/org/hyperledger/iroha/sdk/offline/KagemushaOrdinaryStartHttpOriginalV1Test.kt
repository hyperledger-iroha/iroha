// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Pure bounded DATA grammar only. No fixture is a Native owner or physical attestation. */
class KagemushaOrdinaryStartHttpOriginalV1Test {
    @Test fun completeNativeChunkBytesRoundtripAndAllMissingOrSubstitutedOriginalsRefuse() {
        val c = fixtureC()
        val credential = byteArrayOf(3,4)
        val scope = ByteArray(32) { 5 }; val nativeDigest = ByteArray(32) { 6 }
        val body = body(c,credential,ByteArray(100000) { 7 })
        val chunks = chunks(body,scope,nativeDigest)
        assertEquals(3,chunks.size)
        assertContentEquals(body, KagemushaOrdinaryIdentityHttpCodecV1.retailStartOriginalChunks(
            chunks,c,credential,scope,nativeDigest,ByteArray(8) { 1 }))
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.retailStartOriginalChunks(
            chunks.dropLast(1),c,credential,scope,nativeDigest,ByteArray(8) { 1 }) }
        for (field in listOf(0,1,2,3,4,5)) {
            val changed=chunks.map { it.map(ByteArray::copyOf).toMutableList() }.toMutableList()
            changed[1][field][0]=(changed[1][field][0].toInt() xor 1).toByte()
            assertFails { KagemushaOrdinaryIdentityHttpCodecV1.retailStartOriginalChunks(
                changed,c,credential,scope,nativeDigest,ByteArray(8) { 1 }) }
        }
        @Suppress("UNCHECKED_CAST") val fields=JsonParser.parse(body.toString(Charsets.UTF_8)) as Map<String,Any?>
        for (key in fields.keys) assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(
            json(fields.toMutableMap().also { it.remove(key) }),c,credential) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(
            json(fields.toMutableMap().also { it["ready"]=true }),c,credential) }
        for ((key,size) in listOf("raw_admission_original_base64" to 315,
            "platform_original_base64" to 131073,"core_possession_original_base64" to 5121,
            "app_certificate_base64" to 16385)) assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(
                json(fields.toMutableMap().also { it[key]=base64(ByteArray(size) { 1 }) }),c,credential) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(
            json(fields.toMutableMap().also { it["selected_integrity"]=mapOf("challenge" to "AA==","lease" to "AA==") }),c,credential) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(body,c,byteArrayOf(3,5)) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(
            json(mapOf("signed_preparation_base64" to base64(c),"app_certificate_base64" to base64(credential))),c,credential) }
    }
    @Test fun fullJsonMaximumRemainsBoundedAndNoDigestReconstructsBody() {
        val c=fixtureC();val credential=byteArrayOf(3,4);val scope=ByteArray(32) { 5 };val digest=ByteArray(32) { 6 }
        val oversized=ByteArray(262145) { 32 }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.requireRetailStartOriginal(oversized,c,credential) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.retailStartOriginalChunks(
            chunks(oversized,scope,digest),c,credential,scope,digest,ByteArray(8) { 1 }) }
        assertFails { KagemushaOrdinaryIdentityHttpCodecV1.retailStartOriginalChunks(
            listOf(),c,credential,scope,digest,ByteArray(8) { 1 }) }
    }
    private fun fixtureC():ByteArray {
        var p=Paths.get("").toAbsolutePath()
        while(!Files.exists(p.resolve("fixtures/offline/kagemusha_ordinary_app_enrollment_v1.json"))) p=checkNotNull(p.parent)
        @Suppress("UNCHECKED_CAST") val f=JsonParser.parse(String(Files.readAllBytes(p.resolve("fixtures/offline/kagemusha_ordinary_app_enrollment_v1.json")), Charsets.UTF_8)) as Map<String,Any?>
        @Suppress("UNCHECKED_CAST") val vectors=f["vectors"] as List<Map<String,Any?>>
        return Base64.getDecoder().decode(vectors.first()["signed_preparation_base64"] as String)
    }
    private fun body(c:ByteArray,credential:ByteArray,platform:ByteArray)=json(linkedMapOf(
        "wallet" to "inert-setup-data-only",
        "signed_preparation_base64" to base64(c),"raw_admission_original_base64" to base64(ByteArray(314) { 2 }),
        "platform_original_base64" to base64(platform),"core_possession_original_base64" to base64(byteArrayOf(8)),
        "app_certificate_base64" to base64(credential),"selected_integrity" to null))
    private fun chunks(body:ByteArray,scope:ByteArray,digest:ByteArray)=body.asList().chunked(65536).mapIndexed { i,raw ->
        listOf(KagemushaCoreCoordinatorFrameV1.u32(i),raw.toByteArray(),MessageDigest.getInstance("SHA-256").digest(body),
            KagemushaCoreCoordinatorFrameV1.u32(body.size),scope.copyOf(),digest.copyOf()) }
    private fun base64(raw:ByteArray)=Base64.getEncoder().encodeToString(raw)
    private fun json(fields:Map<String,Any?>)=JsonEncoder.encode(fields).toByteArray(Charsets.UTF_8)
}
