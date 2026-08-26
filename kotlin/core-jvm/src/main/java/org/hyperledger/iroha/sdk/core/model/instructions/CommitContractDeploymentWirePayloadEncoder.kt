package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import java.util.Locale
import java.util.Optional
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Encodes the atomic `CommitContractDeployment` standard transaction instruction. */
object CommitContractDeploymentWirePayloadEncoder {
    const val WIRE_NAME = "iroha.instruction.v1::smart_contract_code::CommitContractDeployment"
    private const val SCHEMA_NAME =
        "iroha_data_model::isi::smart_contract_code::CommitContractDeployment"
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val stringAdapter = NoritoAdapters.stringAdapter()
    private val optionalU64 = NoritoAdapters.option(U64Adapter)
    private val optionalString = NoritoAdapters.option(stringAdapter)

    /** Builds a wire-framed instruction accepted by the transaction encoder. */
    @JvmStatic
    fun encode(
        expectedDeployNonce: BigInteger,
        contractAddress: String,
        codeHashHex: String,
        contractAlias: String,
        leaseExpiryMs: BigInteger? = null,
        expectedPreviousContractAddress: String? = null,
    ): InstructionBox {
        val payload = Payload(
            requireU64(expectedDeployNonce, "expectedDeployNonce"),
            exact(contractAddress, "contractAddress"),
            hash(codeHashHex),
            exact(contractAlias, "contractAlias"),
            Optional.ofNullable(leaseExpiryMs?.let { requireU64(it, "leaseExpiryMs") }),
            Optional.ofNullable(expectedPreviousContractAddress?.let {
                exact(it, "expectedPreviousContractAddress")
            }),
        )
        return InstructionBox.fromWirePayload(
            WIRE_NAME,
            NoritoCodec.encode(payload, SCHEMA_NAME, PayloadAdapter),
        )
    }

    private fun exact(value: String, field: String): String {
        require(value.isNotEmpty() && value == value.trim()) { "$field must be an exact non-empty string" }
        return value
    }

    private fun requireU64(value: BigInteger, field: String): BigInteger {
        require(value.signum() >= 0 && value <= U64_MAX) { "$field must fit u64" }
        return value
    }

    private fun hash(value: String): ByteArray {
        val normalized = exact(value, "codeHashHex").lowercase(Locale.ROOT)
        require(normalized.length == 64 && normalized.all { it in '0'..'9' || it in 'a'..'f' }) {
            "codeHashHex must contain exactly 64 hexadecimal characters"
        }
        return ByteArray(32) { normalized.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }

    private data class Payload(
        val nonce: BigInteger, val address: String, val hash: ByteArray, val alias: String,
        val expiry: Optional<BigInteger>, val previous: Optional<String>,
    )

    private object U64Adapter : TypeAdapter<BigInteger> {
        override fun encode(encoder: NoritoEncoder, value: BigInteger) =
            encoder.writeUInt(requireU64(value, "u64").toLong(), 64)
        override fun decode(decoder: NoritoDecoder): BigInteger {
            val value = decoder.readUInt(64)
            return if (value >= 0) BigInteger.valueOf(value)
            else BigInteger.valueOf(value and Long.MAX_VALUE).setBit(63)
        }
    }

    private object HashAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
        override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(32)
    }

    private object PayloadAdapter : TypeAdapter<Payload> {
        override fun encode(encoder: NoritoEncoder, value: Payload) {
            field(encoder, U64Adapter, value.nonce)
            field(encoder, stringAdapter, value.address)
            field(encoder, HashAdapter, value.hash)
            field(encoder, stringAdapter, value.alias)
            field(encoder, optionalU64, value.expiry)
            field(encoder, optionalString, value.previous)
        }
        override fun decode(decoder: NoritoDecoder): Payload =
            throw UnsupportedOperationException("deployment instruction decoding is not exposed")
    }

    private fun <T> field(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val bytes = child.toByteArray()
        encoder.writeLength(bytes.size.toLong(), false)
        encoder.writeBytes(bytes)
    }
}
