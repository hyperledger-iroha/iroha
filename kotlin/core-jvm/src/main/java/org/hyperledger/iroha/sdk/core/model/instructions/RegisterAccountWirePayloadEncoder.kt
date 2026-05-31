// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Encodes account registration instructions in wire-framed Norito format.
 *
 * Wire format:
 * - Wire name: "iroha.register"
 * - Payload: Norito-framed RegisterBox enum with Account variant (discriminant 2)
 *
 * For self-registration: authority = new account ID, first instruction = Register<Account>
 * for that same account, no linked domains. Iroha allows unregistered authority in this case.
 */
object RegisterAccountWirePayloadEncoder {

    const val WIRE_NAME: String = "iroha.register"

    private const val SCHEMA_PATH = "iroha_data_model::isi::register::RegisterBox"

    /** RegisterBox enum discriminant for Account variant. */
    private const val REGISTER_BOX_ACCOUNT_DISCRIMINANT = 2L

    /**
     * Encodes a self-registration instruction as a wire-framed InstructionBox.
     *
     * @param accountId The I105-encoded account ID to register
     * @return InstructionBox with wire payload ready for transaction encoding
     */
    @JvmStatic
    fun encodeRegisterAccount(accountId: String): InstructionBox {
        val wirePayload = encodeRegisterBox(accountId)
        return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload)
    }

    /** Decodes a Norito-framed `RegisterBox::Account` payload. */
    @JvmStatic
    internal fun decodeRegisterAccountPayload(wirePayload: ByteArray): String =
        NoritoCodec.decode(wirePayload, RegisterBoxAccountAdapter(), SCHEMA_PATH)

    private fun encodeRegisterBox(accountId: String): ByteArray {
        return NoritoCodec.encode(accountId, SCHEMA_PATH, RegisterBoxAccountAdapter())
    }

    private class RegisterBoxAccountAdapter : TypeAdapter<String> {

        override fun encode(encoder: NoritoEncoder, value: String) {
            // RegisterBox enum: u32 discriminant + length-prefixed variant payload
            encoder.writeUInt(REGISTER_BOX_ACCOUNT_DISCRIMINANT, 32)
            val variantChild = encoder.childEncoder()
            encodeRegisterAccountStruct(variantChild, value)
            val variantPayload = variantChild.toByteArray()
            writeLength(encoder, variantPayload.size)
            encoder.writeBytes(variantPayload)
        }

        /** Register<Account> struct has a single field: object: NewAccount */
        private fun encodeRegisterAccountStruct(encoder: NoritoEncoder, accountId: String) {
            val objectChild = encoder.childEncoder()
            encodeNewAccount(objectChild, accountId)
            val objectPayload = objectChild.toByteArray()
            writeLength(encoder, objectPayload.size)
            encoder.writeBytes(objectPayload)
        }

        /**
         * NewAccount struct (5 fields):
         * 1. id: AccountId (transparent → AccountController)
         * 2. metadata: Metadata — empty
         * 3. label: Option<AccountAlias> — None
         * 4. uaid: Option<UniversalAccountId> — None
         * 5. opaque_ids: Vec<OpaqueAccountId> — empty
         */
        private fun encodeNewAccount(encoder: NoritoEncoder, accountId: String) {
            // Field 1: id — reuse AccountId encoding from TransferWirePayloadEncoder
            val accountIdBytes = TransferWirePayloadEncoder.encodeAccountIdPayload(accountId)
            writeFieldWithLength(encoder, accountIdBytes)

            // Field 2: metadata (empty Metadata/BTreeMap) — count = 0
            writeFieldWithLength(encoder, encodeEmptySequence())

            // Field 3: label (None)
            val noneBytes = encodeNone()
            writeFieldWithLength(encoder, noneBytes)

            // Field 4: uaid (None)
            writeFieldWithLength(encoder, noneBytes)

            // Field 5: opaque_ids (empty Vec)
            writeFieldWithLength(encoder, encodeEmptySequence())
        }

        private fun writeFieldWithLength(encoder: NoritoEncoder, payload: ByteArray) {
            writeLength(encoder, payload.size)
            encoder.writeBytes(payload)
        }

        private fun writeLength(encoder: NoritoEncoder, size: Int) {
            encoder.writeLength(size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
        }

        /** Empty sequence/set/map: u64_le(0) — zero element count. */
        private fun encodeEmptySequence(): ByteArray {
            val enc = NoritoEncoder(0)
            enc.writeUInt(0L, 64)
            return enc.toByteArray()
        }

        /** Option::None: u8(0). */
        private fun encodeNone(): ByteArray {
            val enc = NoritoEncoder(0)
            enc.writeByte(0)
            return enc.toByteArray()
        }

        override fun decode(decoder: NoritoDecoder): String =
            decodeRegisterBox(decoder)

        private fun decodeRegisterBox(decoder: NoritoDecoder): String {
            val discriminant = decoder.readUInt(32)
            require(discriminant == REGISTER_BOX_ACCOUNT_DISCRIMINANT) {
                "Unsupported RegisterBox discriminant: $discriminant"
            }
            val payloadLength = checkedLength(
                decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
                "RegisterBox::Account payload",
            )
            val payload = decoder.readBytes(payloadLength)
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val accountId = decodeRegisterAccountStruct(child)
            require(child.remaining() == 0) { "Trailing bytes after RegisterBox::Account payload" }
            return accountId
        }

        private fun decodeRegisterAccountStruct(decoder: NoritoDecoder): String {
            val newAccountPayload = decodeSizedRawField(decoder, "Register<Account>.object")
            val child = NoritoDecoder(newAccountPayload, decoder.flags, decoder.flagsHint)
            val accountId = decodeNewAccount(child)
            require(child.remaining() == 0) { "Trailing bytes after NewAccount payload" }
            return accountId
        }

        private fun decodeNewAccount(decoder: NoritoDecoder): String {
            val accountPayload = decodeSizedRawField(decoder, "NewAccount.id")
            val accountId = TransferWirePayloadEncoder.decodeAccountIdPayload(
                accountPayload,
                decoder.flags,
                decoder.flagsHint,
            )
            requireEmptySequence(decodeSizedRawField(decoder, "NewAccount.metadata"), "NewAccount.metadata")
            requireNone(decodeSizedRawField(decoder, "NewAccount.label"), "NewAccount.label")
            requireNone(decodeSizedRawField(decoder, "NewAccount.uaid"), "NewAccount.uaid")
            requireEmptySequence(decodeSizedRawField(decoder, "NewAccount.opaque_ids"), "NewAccount.opaque_ids")
            return accountId
        }
    }

    private fun decodeSizedRawField(decoder: NoritoDecoder, fieldName: String): ByteArray {
        val payloadLength = checkedLength(
            decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
            "$fieldName payload",
        )
        return decoder.readBytes(payloadLength)
    }

    private fun checkedLength(length: Long, fieldName: String): Int {
        require(length >= 0L) { "$fieldName must be non-negative" }
        require(length <= Int.MAX_VALUE) { "$fieldName too large" }
        return length.toInt()
    }

    private fun requireEmptySequence(payload: ByteArray, fieldName: String) {
        val decoder = NoritoDecoder(payload, 0, NoritoHeader.MINOR_VERSION)
        val count = decoder.readUInt(64)
        require(count == 0L) { "$fieldName must be empty" }
        require(decoder.remaining() == 0) { "Trailing bytes after $fieldName" }
    }

    private fun requireNone(payload: ByteArray, fieldName: String) {
        require(payload.size == 1 && payload[0].toInt() == 0) { "$fieldName must be Option::None" }
    }
}
