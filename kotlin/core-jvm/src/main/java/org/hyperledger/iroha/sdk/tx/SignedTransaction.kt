package org.hyperledger.iroha.sdk.tx

import java.util.Optional

class SignedTransaction private constructor(
    encodedPayload: ByteArray,
    signature: ByteArray,
    publicKey: ByteArray,
    private val schemaName: String,
    private val keyAlias: String?,
    exportedKeyBundle: ByteArray?,
    blsPublicKey: ByteArray?,
    private val multisigSignatures: MultisigSignatures?,
) {
    private val _encodedPayload: ByteArray = encodedPayload.copyOf()
    private val _signature: ByteArray = signature.copyOf()
    private val _publicKey: ByteArray = publicKey.copyOf()
    private val _exportedKeyBundle: ByteArray? = exportedKeyBundle?.copyOf()
    private val _blsPublicKey: ByteArray? = blsPublicKey?.copyOf()

    constructor(
        encodedPayload: ByteArray,
        signature: ByteArray,
        publicKey: ByteArray,
        schemaName: String,
    ) : this(encodedPayload, signature, publicKey, schemaName, null, null, null, null)

    constructor(
        encodedPayload: ByteArray,
        signature: ByteArray,
        publicKey: ByteArray,
        schemaName: String,
        keyAlias: String?,
    ) : this(encodedPayload, signature, publicKey, schemaName, keyAlias, null, null, null)

    constructor(
        encodedPayload: ByteArray,
        signature: ByteArray,
        publicKey: ByteArray,
        schemaName: String,
        keyAlias: String?,
        exportedKeyBundle: ByteArray?,
    ) : this(encodedPayload, signature, publicKey, schemaName, keyAlias, exportedKeyBundle, null, null)

    fun encodedPayload(): ByteArray = _encodedPayload.copyOf()

    fun signature(): ByteArray = _signature.copyOf()

    fun publicKey(): ByteArray = _publicKey.copyOf()

    fun blsPublicKey(): Optional<ByteArray> =
        if (_blsPublicKey == null) Optional.empty() else Optional.of(_blsPublicKey.copyOf())

    fun schemaName(): String = schemaName

    fun keyAlias(): Optional<String> = Optional.ofNullable(keyAlias)

    fun exportedKeyBundle(): Optional<ByteArray> =
        if (_exportedKeyBundle == null) Optional.empty() else Optional.of(_exportedKeyBundle.copyOf())

    fun multisigSignatures(): Optional<MultisigSignatures> = Optional.ofNullable(multisigSignatures)

    fun toBuilder(): Builder = builder()
        .setEncodedPayload(encodedPayload())
        .setSignature(signature())
        .setPublicKey(publicKey())
        .setSchemaName(schemaName)
        .setKeyAlias(keyAlias)
        .setExportedKeyBundle(exportedKeyBundle().orElse(null))
        .setBlsPublicKey(blsPublicKey().orElse(null))
        .setMultisigSignatures(multisigSignatures().orElse(null))

    class Builder {
        private var encodedPayload: ByteArray? = null
        private var signature: ByteArray? = null
        private var publicKey: ByteArray? = null
        private var schemaName: String? = null
        private var keyAlias: String? = null
        private var exportedKeyBundle: ByteArray? = null
        private var blsPublicKey: ByteArray? = null
        private var multisigSignatures: MultisigSignatures? = null

        fun setEncodedPayload(encodedPayload: ByteArray?): Builder = apply {
            this.encodedPayload = encodedPayload?.copyOf()
        }

        fun setSignature(signature: ByteArray?): Builder = apply {
            this.signature = signature?.copyOf()
        }

        fun setPublicKey(publicKey: ByteArray?): Builder = apply {
            this.publicKey = publicKey?.copyOf()
        }

        fun setSchemaName(schemaName: String?): Builder = apply {
            this.schemaName = schemaName
        }

        fun setKeyAlias(keyAlias: String?): Builder = apply {
            this.keyAlias = keyAlias
        }

        fun setExportedKeyBundle(exportedKeyBundle: ByteArray?): Builder = apply {
            this.exportedKeyBundle = exportedKeyBundle?.copyOf()
        }

        fun setBlsPublicKey(blsPublicKey: ByteArray?): Builder = apply {
            this.blsPublicKey = blsPublicKey?.copyOf()
        }

        fun setMultisigSignatures(multisigSignatures: MultisigSignatures?): Builder = apply {
            this.multisigSignatures = multisigSignatures
        }

        fun setMultisigSignatures(signatures: List<MultisigSignature>?): Builder = apply {
            this.multisigSignatures = signatures?.let { MultisigSignatures.of(it) }
        }

        fun build(): SignedTransaction {
            check(encodedPayload != null && signature != null && publicKey != null && schemaName != null) {
                "encodedPayload, signature, publicKey, and schemaName must be provided"
            }
            return SignedTransaction(
                encodedPayload!!,
                signature!!,
                publicKey!!,
                schemaName!!,
                keyAlias,
                exportedKeyBundle,
                blsPublicKey,
                multisigSignatures,
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()
    }
}
