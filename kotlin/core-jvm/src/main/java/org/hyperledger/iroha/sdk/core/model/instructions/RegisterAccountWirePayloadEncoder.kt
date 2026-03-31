// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
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
            encoder.writeUInt(variantPayload.size.toLong(), 64)
            encoder.writeBytes(variantPayload)
        }

        /** Register<Account> struct has a single field: object: NewAccount */
        private fun encodeRegisterAccountStruct(encoder: NoritoEncoder, accountId: String) {
            val objectChild = encoder.childEncoder()
            encodeNewAccount(objectChild, accountId)
            val objectPayload = objectChild.toByteArray()
            encoder.writeUInt(objectPayload.size.toLong(), 64)
            encoder.writeBytes(objectPayload)
        }

        /**
         * NewAccount struct (6 fields):
         * 1. id: AccountId (transparent → AccountController)
         * 2. linked_domains: BTreeSet<DomainId> — empty
         * 3. metadata: Metadata — empty
         * 4. label: Option<AccountLabel> — None
         * 5. uaid: Option<UniversalAccountId> — None
         * 6. opaque_ids: Vec<OpaqueAccountId> — empty
         */
        private fun encodeNewAccount(encoder: NoritoEncoder, accountId: String) {
            // Field 1: id — reuse AccountId encoding from TransferWirePayloadEncoder
            val accountIdBytes = TransferWirePayloadEncoder.encodeAccountIdPayload(accountId)
            writeFieldWithLength(encoder, accountIdBytes)

            // Field 2: linked_domains (empty BTreeSet) — count = 0
            writeFieldWithLength(encoder, encodeEmptySequence())

            // Field 3: metadata (empty Metadata/BTreeMap) — count = 0
            writeFieldWithLength(encoder, encodeEmptySequence())

            // Field 4: label (None)
            val noneBytes = encodeNone()
            writeFieldWithLength(encoder, noneBytes)

            // Field 5: uaid (None)
            writeFieldWithLength(encoder, noneBytes)

            // Field 6: opaque_ids (empty Vec)
            writeFieldWithLength(encoder, encodeEmptySequence())
        }

        private fun writeFieldWithLength(encoder: NoritoEncoder, payload: ByteArray) {
            encoder.writeUInt(payload.size.toLong(), 64)
            encoder.writeBytes(payload)
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
            throw UnsupportedOperationException("Decoding RegisterBox payloads is not supported")
    }
}
