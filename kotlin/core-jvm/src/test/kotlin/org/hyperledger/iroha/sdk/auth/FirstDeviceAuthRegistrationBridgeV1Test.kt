package org.hyperledger.iroha.sdk.auth

import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.json.Json
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthRegistrationBridgeV1 as Bridge

class FirstDeviceAuthRegistrationBridgeV1Test {
    private val f = RegistrationBridgeFixtureV1
    @Test fun independentBigEndianFrameAcceptsExactOriginalsAndReturnsCopies() {
        val o = f.originals(); val bytes = f.frame(o)
        val frame = Bridge.FrameOriginal.parseResponse(f.response(bytes), o)
        assertArrayEquals(bytes, frame.originalBytes()); assertEquals(f.INCARNATION, frame.declaredIncarnation)
        bytes[0] = 0; frame.originalBytes()[0] = 0
        assertEquals('B'.code.toByte(), frame.originalBytes()[0])
        val dataspaceOffset = "BPNG/FIRST-DEVICE/RETAIL-REGISTRATION/V1\u0000".length + 2 +
            4 + o.networkId.literal.toByteArray().size + 4 + "mibank.bpng".length
        assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5, 6, 7, 8), f.frame(o).copyOfRange(dataspaceOffset, dataspaceOffset + 8))
    }
    @Test fun everyByteMutationTruncationAndTrailingByteRefuse() {
        val o = f.originals(); val bytes = f.frame(o)
        for (i in bytes.indices) {
            // Incarnation is explicitly declared server DATA; an independently valid UUID may vary.
            val changed = bytes.copyOf(); changed[i] = (changed[i].toInt() xor 128).toByte()
            assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(f.response(changed), o) }
        }
        for (size in listOf(1, 32, bytes.size - 1)) assertThrows(Exception::class.java) {
            Bridge.FrameOriginal.parseResponse(f.response(bytes.copyOf(size)), o)
        }
        assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(f.response(bytes + byteArrayOf(0)), o) }
    }
    @Test fun declaredIncarnationDoesNotBecomeAClientChosenNativeCapability() {
        val o = f.originals()
        val frame = Bridge.FrameOriginal.parseResponse(f.response(f.frame(o, "14000000-0000-4000-8000-000000000003")), o)
        assertEquals("14000000-0000-4000-8000-000000000003", frame.declaredIncarnation)
        assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(f.response(f.frame(o, "00000000-0000-0000-0000-000000000000")), o) }
    }
    @Test fun responseClosedFieldsAndCanonicalBase64AreRequired() {
        val o = f.originals(); val b64 = f.b64(f.frame(o))
        assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(Json.obj(mapOf("frame_original_base64" to Json.of(b64), "ready" to Json.of(true))).toJsonBytes(), o) }
        assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(Json.obj(mapOf("frame_original_base64" to Json.of(" " + b64))).toJsonBytes(), o) }
        val exactResponse = ("\n " + f.response(f.frame(o)).toString(Charsets.UTF_8) + "\n").toByteArray()
        val frame = Bridge.FrameOriginal.parseResponse(exactResponse, o)
        assertArrayEquals(exactResponse, frame.responseOriginalBytes())
        frame.responseOriginalBytes()[0] = 0; assertArrayEquals(exactResponse, frame.responseOriginalBytes())
    }
    @Test fun anotherAccountOrChangedOwnerRequestCannotEnterTheExistingAccountBinding() {
        assertThrows(Exception::class.java) { Bridge.RegistrationOriginal.parse(f.registrationBytes(), "invented-account") }
        val changed = f.registrationBytes().toString(Charsets.UTF_8).replace("signup-alias", "changed-alias").toByteArray()
        val o = f.originals(); val changedRegistration = Bridge.RegistrationOriginal.parse(changed, f.account())
        val different = Bridge.Originals.retain(changedRegistration, o.challenge, o.raw, Protocol.possessionMessageBytes(o.challenge, o.raw),
            f.DER, f.INTEGRITY, o.finish, f.GOOGLE, o.networkId, o.physicalDataspaceId, o.laneId)
        assertThrows(Exception::class.java) { Bridge.FrameOriginal.parseResponse(f.response(f.frame(o)), different) }
    }
    @Test fun originalGoogleAndFinishInputsCannotBeRenewed() {
        val o = f.originals()
        for (token in listOf("different.google.original", f.GOOGLE + "x")) assertThrows(Exception::class.java) {
            Bridge.Originals.retain(o.registration, o.challenge, o.raw, Protocol.possessionMessageBytes(o.challenge, o.raw),
                f.DER, f.INTEGRITY, o.finish, token, o.networkId, o.physicalDataspaceId, o.laneId)
        }
        assertThrows(Exception::class.java) { Bridge.Originals.retain(o.registration, o.challenge, o.raw,
            Protocol.possessionMessageBytes(o.challenge, o.raw), f.DER, f.INTEGRITY + "x", o.finish, f.GOOGLE,
            o.networkId, o.physicalDataspaceId, o.laneId) }
    }
    @Test fun purposeRequestsKeepExactOriginalsAndOriginalRegistrationUuid() {
        val o = f.originals(); val frame = Bridge.FrameOriginal.parseResponse(f.response(f.frame(o)), o)
        val request = Bridge.frameRequest(o); val register = Bridge.registerRequest(frame, f.DER)
        assertEquals(Bridge.FRAME_PATH, request.path); assertEquals(Bridge.REGISTER_PATH, register.path)
        assertEquals(f.REQUEST_ID, request.registrationRequestId); assertEquals(request.registrationRequestId, register.registrationRequestId)
        assertTrue(request.bodyBytes().toString(Charsets.UTF_8).contains(f.b64(o.registration.originalBytes())))
        assertFalse(request.bodyBytes().toString(Charsets.UTF_8).contains("authentication_signature"))
        assertTrue(register.bodyBytes().toString(Charsets.UTF_8).contains("authentication_signature_der_base64"))
        request.bodyBytes()[0] = 0; assertEquals('{'.code.toByte(), request.bodyBytes()[0])
    }
    @Test fun pendingResponseCreatesNoSessionAndRejectsMismatchedOriginals() {
        val o = f.originals()
        fun response(status: String) = Json.obj(mapOf("registration_id" to Json.of(f.REQUEST_ID),
            "account_id" to Json.of(o.registration.accountId), "status" to Json.of(status))).toJsonBytes()
        Bridge.requirePendingResponse(response("PENDING"), o.registration)
        assertThrows(Exception::class.java) { Bridge.requirePendingResponse(response("READY"), o.registration) }
        assertThrows(Exception::class.java) { Bridge.requirePendingResponse("{}".toByteArray(), o.registration) }
    }

    @Test fun zeroPhysicalDataspaceAndMalformedNestedAndroidRequestRefuseBeforeFrameSigning() {
        val o = f.originals()
        assertThrows(Exception::class.java) { Bridge.Originals.retain(o.registration, o.challenge, o.raw,
            Protocol.possessionMessageBytes(o.challenge, o.raw), f.DER, f.INTEGRITY, o.finish, f.GOOGLE,
            o.networkId, BigInteger.ZERO, o.laneId) }
        val original = f.registrationBytes().toString(Charsets.UTF_8)
        for (changed in listOf(original.replace("\"address\":{", "\"address\":{\"future\":true,"),
            original.replace("\"first_name\":\"Test\"", "\"first_name\":1"),
            original.replace("\"platform\":\"android\"", "\"platform\":\"ios\""),
            original.replace("\"country\":\"PG\"", "\"country\":\"ZZ\"")))
            assertThrows(Exception::class.java) { Bridge.RegistrationOriginal.parse(changed.toByteArray(), f.account()) }
    }
}
