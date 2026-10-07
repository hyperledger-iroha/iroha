// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.testing.JvmApiInventory
import org.junit.jupiter.api.Test

/** Original/evidence DATA only; no key generation or Native financial admission is faked. */
class KagemushaWalletEnrollmentV1Test {
    private val key=ByteArray(65){2}.also{it[0]=4}
    private val account=ByteArray(385){3}
    private val binding=ByteArray(32){4}
    private val chain=arrayOf(byteArrayOf(5),byteArrayOf(6))
    private val one=byteArrayOf(1)
    private fun originals()=KagemushaWalletEnrollmentOriginalsV1(one,one,one,1000,2000)
    private fun reply(status:Int,bytes:ByteArray=one)=KagemushaWalletEnrollmentReplyV1(status,-1,0,
        if(status==0)key else byteArrayOf(),if(status==0)account else byteArrayOf(),
        if(status==0)binding else byteArrayOf(),if(status==0)chain else emptyArray(),bytes)

    private fun enrolled(keyBytes:ByteArray=key,marker:ByteArray=one,accountFrame:ByteArray=account) =
        KagemushaWalletEnrollmentReplyV1(0,-1,0,keyBytes,accountFrame,binding,chain,marker).progress()

    @Test fun `attestation read is bracketed by unchanged enrollment originals`() {
        val order = ArrayList<String>()
        val der = listOf(byteArrayOf(1, 2), byteArrayOf(3, 4))
        val result = readEnrollmentAttestationChainV1(
            resume = { order += "resume"; enrolled() },
            read = { actualKey ->
                order += "read"; assertContentEquals(key, actualKey)
                KagemushaWalletAndroidAttestationChainV1.Present(der)
            })
        assertEquals(listOf("resume", "read", "resume"), order)
        der.forEach { it.fill(0) }
        assertContentEquals(byteArrayOf(1, 2), result[0])
        assertContentEquals(byteArrayOf(3, 4), result[1])
    }

    @Test fun `changed Native account frame key marker or enrollment phase refuses attestation export`() {
        val pending=reply(1,byteArrayOf()).progress()
        for (after in listOf(
            enrolled(accountFrame=ByteArray(385){9}),
            enrolled(keyBytes = key.copyOf().also { it[1] = 9 }),
            enrolled(marker = byteArrayOf(9)), pending,
        )) {
            var calls = 0
            assertFailsWith<KagemushaWalletExceptionV1> {
                readEnrollmentAttestationChainV1(
                    resume = { if (calls++ == 0) enrolled() else after },
                    read = { KagemushaWalletAndroidAttestationChainV1.Present(listOf(one, one)) })
            }
            assertEquals(2, calls)
        }
        var reads = 0
        assertFailsWith<KagemushaWalletExceptionV1> {
            readEnrollmentAttestationChainV1( { pending }) {
                reads++; KagemushaWalletAndroidAttestationChainV1.Present(listOf(one, one))
            }
        }
        assertEquals(0, reads)
    }

    @Test fun `attestation failure stays unavailable with exact platform reason`() {
        val reason = KagemushaWalletAndroidUnavailableV1.platform(42)
        var resumes = 0
        val failure = assertFailsWith<KagemushaWalletExceptionV1> {
            readEnrollmentAttestationChainV1( { resumes++; enrolled() }) {
                KagemushaWalletAndroidAttestationChainV1.Unavailable(reason)
            }
        }
        assertEquals(2, resumes)
        assertEquals(-5, failure.status); assertEquals(4, failure.reason); assertEquals(42, failure.platformCode)
        assertEquals(-5, assertFailsWith<KagemushaWalletExceptionV1> {
            readEnrollmentAttestationChainV1( { enrolled() }) { KagemushaWalletAndroidAttestationChainV1.Absent }
        }.status)
    }

    @Test fun `attestation originals preserve complete Core bounds without truncation`() {
        val maximum = List(8) { ByteArray(16384) { index -> (index % 251).toByte() } }
        val result = readEnrollmentAttestationChainV1( { enrolled() }) { KagemushaWalletAndroidAttestationChainV1.Present(maximum) }
        maximum.zip(result).forEach { (expected, actual) -> assertContentEquals(expected, actual) }
        for (bad in listOf(emptyList(), listOf(one), List(9) { one }, listOf(one, byteArrayOf()), listOf(one, ByteArray(16385)))) {
            assertFailsWith<KagemushaWalletExceptionV1> {
                readEnrollmentAttestationChainV1( { enrolled() }) { KagemushaWalletAndroidAttestationChainV1.Present(bad) }
            }
        }
    }


    @Test fun `E1 carrier owns all exact frames and original timestamps`() {
        val bytes=byteArrayOf(0,-1,7);val input=KagemushaWalletEnrollmentOriginalsV1(bytes,bytes,bytes,1000,2000)
        bytes.fill(3);input.frames().forEach{it.fill(4)}
        input.frames().forEach{assertContentEquals(byteArrayOf(0,-1,7),it)}
        assertEquals(1000,input.issuedAtMs);assertEquals(2000,input.expiresAtMs)
        for(role in 0..2)for(length in listOf(0,listOf(1024,1024,4096)[role]+1)) {
            val data=MutableList(3){one};data[role]=ByteArray(length)
            assertFailsWith<IllegalArgumentException>{KagemushaWalletEnrollmentOriginalsV1(data[0],data[1],data[2],1000,2000)}
        }
        for((issued,expires) in listOf(0L to 1L,1L to 1L,2L to 1L,1L to 600002L)) {
            assertFailsWith<IllegalArgumentException>{KagemushaWalletEnrollmentOriginalsV1(one,one,one,issued,expires)}
        }
        KagemushaWalletEnrollmentOriginalsV1(one,one,one,Long.MAX_VALUE-600000,Long.MAX_VALUE)
    }
    @Test fun `selector only carries five originals and never an offered slot`() {
        for(selector in 0..3) {
            val input=KagemushaWalletEnrollmentInputV1(selector,originals(),if(selector>=2)one else byteArrayOf(),if(selector==3)one else byteArrayOf())
            assertEquals(5,input.frames().size);input.frames().forEach{it.fill(9)};assertContentEquals(one,input.frames()[0])
            assertEquals(1000,input.issuedAtMs);assertEquals(2000,input.expiresAtMs)
        }
        for(make in listOf<()->KagemushaWalletEnrollmentInputV1>(
            {KagemushaWalletEnrollmentInputV1(0,originals(),one)},
            {KagemushaWalletEnrollmentInputV1(1,originals(),one)},
            {KagemushaWalletEnrollmentInputV1(2,originals(),ByteArray(524289))},
            {KagemushaWalletEnrollmentInputV1(2,originals(),one,one)},
            {KagemushaWalletEnrollmentInputV1(3,originals(),one)},
            {KagemushaWalletEnrollmentInputV1(4,originals())},
        ))assertFailsWith<IllegalArgumentException>{make()}
    }
    @Test fun `progress owns exact public key account frame binding chain and marker`() {
        val payment=key.copyOf();val frame=account.copyOf();val hash=binding.copyOf();val certificates=chain.map{it.copyOf()}.toTypedArray();val marker=byteArrayOf(8)
        val progress=KagemushaWalletEnrollmentReplyV1(0,-1,0,payment,frame,hash,certificates,marker).progress()
        payment.fill(0);frame.fill(0);hash.fill(0);certificates.forEach{it.fill(0)};marker.fill(0)
        progress.paymentKey().fill(0);progress.accountSigningOriginal().fill(0);progress.playIntegrityRequestHash().fill(0)
        progress.attestationCertificatesDer().forEach{it.fill(0)};progress.markerOriginal().fill(0)
        assertEquals(KagemushaWalletEnrollmentStateV1.ENROLLED,progress.state)
        assertContentEquals(key,progress.paymentKey());assertContentEquals(account,progress.accountSigningOriginal())
        assertContentEquals(binding,progress.playIntegrityRequestHash());assertContentEquals(chain[0],progress.attestationCertificatesDer()[0]);assertContentEquals(byteArrayOf(8),progress.markerOriginal())
        assertTrue(progress.toString().contains("[REDACTED]"))
        for((status,state) in listOf(1 to KagemushaWalletEnrollmentStateV1.PENDING,2 to KagemushaWalletEnrollmentStateV1.SLOT_ABANDONED)) {
            val pending=reply(status,byteArrayOf()).progress();assertEquals(state,pending.state)
            assertTrue(pending.paymentKey().isEmpty() && pending.accountSigningOriginal().isEmpty() && pending.playIntegrityRequestHash().isEmpty() && pending.attestationCertificatesDer().isEmpty() && pending.markerOriginal().isEmpty())
        }
    }
    @Test fun `whole request and credential replies require their exact operation`() {
        val request=ByteArray(524288){7};val input=KagemushaWalletEnrollmentInputV1(2,originals(),request);val result=reply(3,request)
        request.fill(8);assertContentEquals(input.frames()[3],result.original(3))
        for(status in listOf(3,4)) {
            val response=reply(status);assertContentEquals(one,response.original(status))
            assertFailsWith<KagemushaWalletExceptionV1>{response.original(if(status==3)4 else 3)}
            assertFailsWith<KagemushaWalletExceptionV1>{response.progress()}
        }
        val error=assertFailsWith<KagemushaWalletExceptionV1>{KagemushaWalletEnrollmentReplyV1(-5,4,42,byteArrayOf(),byteArrayOf(),byteArrayOf(),emptyArray(),byteArrayOf()).checked()}
        assertEquals(-5,error.status);assertEquals(4,error.reason);assertEquals(42,error.platformCode)
    }
    @Test fun `malformed evidence and foreign completion status cannot select progress`() {
        for(make in listOf<()->KagemushaWalletEnrollmentReplyV1>(
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,byteArrayOf(),account,binding,chain,one)},
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,key,ByteArray(384),binding,chain,one)},
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,key,account,ByteArray(32),chain,one)},
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,key,account,binding,arrayOf(one),one)},
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,key,account,binding,arrayOf(one,ByteArray(16385)),one)},
            {KagemushaWalletEnrollmentReplyV1(0,-1,0,key,account,binding,chain,ByteArray(1025))},
            {KagemushaWalletEnrollmentReplyV1(1,-1,0,key,byteArrayOf(),byteArrayOf(),emptyArray(),byteArrayOf())},
            {reply(3,ByteArray(524289))},{reply(4,ByteArray(1025))},
            {KagemushaWalletEnrollmentReplyV1(-5,2,0,key,byteArrayOf(),byteArrayOf(),emptyArray(),byteArrayOf())},
            {reply(5,byteArrayOf())},
        ))assertEquals(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT,assertFailsWith<KagemushaWalletExceptionV1>{make()}.status)
    }
    @Test fun `compiled JNI uses five DATA frames original timestamps and no slot`() {
        val method=JvmApiInventory.read(KagemushaWalletEnrollmentNativeV1::class.java).methods.filter{it.isNative}.single()
        assertEquals("enroll",method.name);assertTrue(method.isStatic)
        assertEquals("(JI[B[B[B[B[BJJ)Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletEnrollmentReplyV1;",method.descriptor)
    }
    @Test fun `compiled enrollment reply constructor matches the actual Native descriptor`() {
        val constructors = JvmApiInventory.read(KagemushaWalletEnrollmentReplyV1::class.java)
            .methods.filter { it.name == "<init>" }
        assertEquals(listOf("(III[B[B[B[[B[B)V"), constructors.map { it.descriptor })
    }

    @Test fun `Native enrolled reply retains the complete maximum DER chain unchanged`() {
        val maximum = Array(8) { certificate -> ByteArray(16384) { index -> ((index + certificate) % 251).toByte() } }
        val progress = KagemushaWalletEnrollmentReplyV1(0, -1, 0, key, account, binding, maximum, one).progress()
        maximum.zip(progress.attestationCertificatesDer()).forEach { (expected, actual) ->
            assertContentEquals(expected, actual)
        }
        maximum.forEach { it.fill(0) }
        val first = progress.attestationCertificatesDer()
        first.forEach { it.fill(0) }
        assertEquals(8, progress.attestationCertificatesDer().size)
        assertEquals(16384, progress.attestationCertificatesDer().first().size)
        assertEquals(1.toByte(), progress.attestationCertificatesDer()[0][1])
        for (bad in listOf(emptyArray(), Array(9) { one }, arrayOf(one, byteArrayOf()))) {
            assertFailsWith<KagemushaWalletExceptionV1> {
                KagemushaWalletEnrollmentReplyV1(0, -1, 0, key, account, binding, bad, one)
            }
        }
    }

    @Test fun `private chain callback deep copies every bounded DER original`() {
        val certificate=byteArrayOf(7);val original=arrayOf(certificate,certificate)
        val response=KagemushaWalletNativeReplyV1(0,chain=original)
        certificate.fill(0);response.certificatesDer().forEach{it.fill(0)}
        response.certificatesDer().forEach{assertContentEquals(byteArrayOf(7),it)}
        assertTrue(response.bytes().isEmpty())
        for(make in listOf<()->KagemushaWalletNativeReplyV1>(
            {KagemushaWalletNativeReplyV1(0,chain=arrayOf(one))},
            {KagemushaWalletNativeReplyV1(0,chain=Array(9){one})},
            {KagemushaWalletNativeReplyV1(0,chain=arrayOf(one,ByteArray(16385)))},
            {KagemushaWalletNativeReplyV1(0,bytes=one,chain=chain)},
            {KagemushaWalletNativeReplyV1(1,chain=chain)},
        ))assertFailsWith<IllegalArgumentException>{make()}
    }
}
