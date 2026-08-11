package org.hyperledger.iroha.sdk.alias

import java.math.BigInteger
import java.util.Optional
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/** Canonical V1 Norito codecs used by alias planners and local apply. */
object AliasNoritoCodec {
    private val decodeChainDiscriminant = ThreadLocal<Int?>()
    private const val ENSURE_SCHEMA = "iroha_data_model::isi::alias_setup::EnsureAlias"
    private const val RENEW_SCHEMA = "iroha_data_model::isi::alias_setup::RenewAliasLease"
    private const val AUTO_RENEW_SCHEMA = "iroha_data_model::isi::alias_setup::ConfigureAliasAutoRenew"
    private const val REBIND_SCHEMA = "iroha_data_model::isi::alias_setup::RebindAccountAlias"
    private const val PRIMARY_SCHEMA =
        "iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias"

    /** Encodes the exact bare setup-plan body committed by the planner hash. */
    @JvmStatic
    fun encodePlanBody(body: AliasTransactionPlanBodyV1): ByteArray =
        NoritoCodec.encodeAdaptive(body, PLAN_BODY_ADAPTER).payload()

    /** Decodes an exact bare setup-plan body. */
    @JvmStatic
    fun decodePlanBody(
        payload: ByteArray,
        chainDiscriminant: Int,
    ): AliasTransactionPlanBodyV1 = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decodeAdaptive(payload, PLAN_BODY_ADAPTER)
    }

    /** Encodes the exact bare lifecycle-plan body committed by the planner hash. */
    @JvmStatic
    fun encodeLifecyclePlanBody(body: AliasLifecycleTransactionPlanBodyV1): ByteArray =
        NoritoCodec.encodeAdaptive(body, LIFECYCLE_PLAN_BODY_ADAPTER).payload()

    /** Decodes an exact bare lifecycle-plan body. */
    @JvmStatic
    fun decodeLifecyclePlanBody(
        payload: ByteArray,
        chainDiscriminant: Int,
    ): AliasLifecycleTransactionPlanBodyV1 = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decodeAdaptive(payload, LIFECYCLE_PLAN_BODY_ADAPTER)
    }

    /** Encodes the exact bare sponsored-onboarding receipt body signed by Torii. */
    @JvmStatic
    fun encodeOnboardingPlanBody(body: AccountOnboardingPlanBodyV1): ByteArray =
        NoritoCodec.encodeAdaptive(body, ONBOARDING_PLAN_BODY_ADAPTER).payload()

    /** Decodes an exact bare sponsored-onboarding receipt body. */
    @JvmStatic
    fun decodeOnboardingPlanBody(
        payload: ByteArray,
        chainDiscriminant: Int,
    ): AccountOnboardingPlanBodyV1 = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decodeAdaptive(payload, ONBOARDING_PLAN_BODY_ADAPTER)
    }

    /** Encodes one typed EnsureAlias instruction with its canonical schema-bound header. */
    @JvmStatic
    fun encodeEnsureAliasFrame(instruction: EnsureAlias): ByteArray =
        NoritoCodec.encode(instruction, ENSURE_SCHEMA, ENSURE_ADAPTER)

    /** Decodes one schema-bound EnsureAlias frame. */
    @JvmStatic
    fun decodeEnsureAliasFrame(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): EnsureAlias = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decode(frame, ENSURE_ADAPTER, ENSURE_SCHEMA)
    }

    /** Encodes one typed renewal instruction with its canonical schema-bound header. */
    @JvmStatic
    fun encodeRenewAliasLeaseFrame(instruction: RenewAliasLease): ByteArray =
        NoritoCodec.encode(instruction, RENEW_SCHEMA, RENEW_ADAPTER)

    /** Decodes one schema-bound renewal frame. */
    @JvmStatic
    fun decodeRenewAliasLeaseFrame(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): RenewAliasLease = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decode(frame, RENEW_ADAPTER, RENEW_SCHEMA)
    }

    /** Encodes one typed auto-renew instruction with its canonical schema-bound header. */
    @JvmStatic
    fun encodeConfigureAutoRenewFrame(instruction: ConfigureAliasAutoRenew): ByteArray =
        NoritoCodec.encode(instruction, AUTO_RENEW_SCHEMA, CONFIGURE_AUTO_RENEW_ADAPTER)

    /** Decodes one schema-bound auto-renew frame. */
    @JvmStatic
    fun decodeConfigureAutoRenewFrame(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): ConfigureAliasAutoRenew = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decode(frame, CONFIGURE_AUTO_RENEW_ADAPTER, AUTO_RENEW_SCHEMA)
    }

    /** Encodes one typed account-alias rebind instruction. */
    @JvmStatic
    fun encodeRebindAccountAliasFrame(instruction: RebindAccountAlias): ByteArray =
        NoritoCodec.encode(instruction, REBIND_SCHEMA, REBIND_ADAPTER)

    /** Decodes one schema-bound account-alias rebind frame. */
    @JvmStatic
    fun decodeRebindAccountAliasFrame(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): RebindAccountAlias = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decode(frame, REBIND_ADAPTER, REBIND_SCHEMA)
    }

    /** Encodes one typed primary-alias compare-and-set instruction. */
    @JvmStatic
    fun encodeCompareAndSetPrimaryAliasFrame(instruction: CompareAndSetPrimaryAccountAlias): ByteArray =
        NoritoCodec.encode(instruction, PRIMARY_SCHEMA, PRIMARY_ADAPTER)

    /** Decodes one schema-bound primary-alias compare-and-set frame. */
    @JvmStatic
    fun decodeCompareAndSetPrimaryAliasFrame(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): CompareAndSetPrimaryAccountAlias = withDecodeChain(chainDiscriminant) {
        NoritoCodec.decode(frame, PRIMARY_ADAPTER, PRIMARY_SCHEMA)
    }

    private val U8 = NoritoAdapters.uint(8)
    private val U16 = NoritoAdapters.uint(16)
    private val U32 = NoritoAdapters.uint(32)
    private val U64 = NoritoAdapters.uint(64)
    private val STRING = NoritoAdapters.stringAdapter()
    private val RAW_BYTES = NoritoAdapters.rawByteVecAdapter()

    private val BIG_U64_ADAPTER = object : TypeAdapter<BigInteger> {
        override fun encode(encoder: NoritoEncoder, value: BigInteger) {
            requireU64(value, "u64")
            encoder.writeUInt(value.toLong(), 64)
        }

        override fun decode(decoder: NoritoDecoder): BigInteger {
            val value = decoder.readUInt(64)
            return if (value >= 0) BigInteger.valueOf(value)
            else BigInteger.valueOf(value and Long.MAX_VALUE).setBit(63)
        }
    }

    private val ACCOUNT_ID_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encoder.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value))
        }

        override fun decode(decoder: NoritoDecoder): String =
            TransferWirePayloadEncoder.decodeAccountIdPayload(
                decoder.readBytes(decoder.remaining()),
                requiredDecodeChainDiscriminant(),
                decoder.flags,
                decoder.flagsHint,
            )
    }

    private fun requiredDecodeChainDiscriminant(): Int =
        checkNotNull(decodeChainDiscriminant.get()) {
            "alias decoding requires an explicit chainDiscriminant"
        }

    private fun <T> withDecodeChain(
        chainDiscriminant: Int,
        operation: () -> T,
    ): T {
        require(chainDiscriminant in 0..0xffff) {
            "chainDiscriminant must fit in u16"
        }
        val previous = decodeChainDiscriminant.get()
        check(previous == null || previous == chainDiscriminant) {
            "Conflicting nested chainDiscriminant context"
        }
        decodeChainDiscriminant.set(chainDiscriminant)
        return try {
            operation()
        } finally {
            if (previous == null) {
                decodeChainDiscriminant.remove()
            } else {
                decodeChainDiscriminant.set(previous)
            }
        }
    }

    private val CHAIN_ID_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) = encodeField(encoder, STRING, value)
        override fun decode(decoder: NoritoDecoder): String = decodeField(decoder, STRING)
    }

    private val NETWORK_ID_ADAPTER = object : TypeAdapter<NetworkId> {
        override fun encode(encoder: NoritoEncoder, value: NetworkId) {
            encoder.writeBytes(value.bytes())
        }

        override fun decode(decoder: NoritoDecoder): NetworkId {
            require(decoder.remaining() == NetworkId.BYTE_LENGTH) {
                "NetworkId must contain exactly ${NetworkId.BYTE_LENGTH} bytes"
            }
            return NetworkId.fromBytes(decoder.readBytes(NetworkId.BYTE_LENGTH))
        }

        override fun fixedSize(): Int = NetworkId.BYTE_LENGTH
    }

    private val HASH_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encoder.writeBytes(requireNotNull(AliasHashText.decode(value)) { "invalid hash" })
        }

        override fun decode(decoder: NoritoDecoder): String {
            require(decoder.remaining() == 32) { "Hash must contain 32 bytes" }
            return decoder.readBytes(32).toHex()
        }

        override fun fixedSize(): Int = 32
    }

    private val ASSET_ID_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeFixedBytes(encoder, AssetDefinitionIdEncoder.parseAddressBytes(value))
        }

        override fun decode(decoder: NoritoDecoder): String =
            AssetDefinitionIdEncoder.encodeFromBytes(decodeFixedBytes(decoder, 16, "AssetDefinitionId"))
    }

    private val QUANTITY_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            val quantity = KotodamaQuantity.parseCanonical(value)
            encodeBigIntegerField(encoder, quantity.mantissa)
            encodeField(encoder, U32, quantity.scale.toLong())
        }

        override fun decode(decoder: NoritoDecoder): String {
            val mantissa = decodeBigIntegerField(decoder)
            val scale = Math.toIntExact(decodeField(decoder, U32))
            return KotodamaQuantity.of(mantissa, scale).toString()
        }
    }

    private val DATASPACE_ID_ADAPTER = object : TypeAdapter<BigInteger> {
        override fun encode(encoder: NoritoEncoder, value: BigInteger) =
            encodeField(encoder, BIG_U64_ADAPTER, value)

        override fun decode(decoder: NoritoDecoder): BigInteger = decodeField(decoder, BIG_U64_ADAPTER)
    }

    private val ACCOUNT_ALIAS_NAME_ADAPTER = object : TypeAdapter<AccountAliasName> {
        private val optionalName = NoritoAdapters.option(STRING)

        override fun encode(encoder: NoritoEncoder, value: AccountAliasName) {
            encodeField(encoder, STRING, value.label)
            encodeField(encoder, optionalName, Optional.ofNullable(value.domain))
            encodeField(encoder, STRING, value.dataspace)
        }

        override fun decode(decoder: NoritoDecoder): AccountAliasName = AccountAliasName(
            decodeField(decoder, STRING),
            decodeField(decoder, optionalName).orElse(null),
            decodeField(decoder, STRING),
        )
    }

    private val RESOLVED_DATASPACE_ADAPTER = object : TypeAdapter<ResolvedDataSpaceV1> {
        override fun encode(encoder: NoritoEncoder, value: ResolvedDataSpaceV1) {
            encodeField(encoder, STRING, value.canonicalName)
            encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId)
        }

        override fun decode(decoder: NoritoDecoder): ResolvedDataSpaceV1 = ResolvedDataSpaceV1(
            decodeField(decoder, STRING),
            decodeField(decoder, DATASPACE_ID_ADAPTER),
        )
    }

    private val DOMAIN_ID_ADAPTER = object : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            val dot = value.indexOf('.')
            require(dot > 0 && dot == value.lastIndexOf('.') && dot < value.length - 1) {
                "domain must use domain.dataspace format"
            }
            encodeField(encoder, STRING, value.substring(0, dot))
            encodeField(encoder, STRING, value.substring(dot + 1))
        }

        override fun decode(decoder: NoritoDecoder): String =
            decodeField(decoder, STRING) + "." + decodeField(decoder, STRING)
    }

    private val RESOLVED_DOMAIN_ADAPTER = object : TypeAdapter<ResolvedDomainV1> {
        override fun encode(encoder: NoritoEncoder, value: ResolvedDomainV1) {
            encodeField(encoder, DOMAIN_ID_ADAPTER, value.canonicalName)
            encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId)
        }

        override fun decode(decoder: NoritoDecoder): ResolvedDomainV1 = ResolvedDomainV1(
            decodeField(decoder, DOMAIN_ID_ADAPTER),
            decodeField(decoder, DATASPACE_ID_ADAPTER),
        )
    }

    private val RESOLVED_ACCOUNT_ALIAS_ADAPTER = object : TypeAdapter<ResolvedAccountAliasV1> {
        override fun encode(encoder: NoritoEncoder, value: ResolvedAccountAliasV1) {
            encodeField(encoder, ACCOUNT_ALIAS_NAME_ADAPTER, value.canonicalName)
            encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId)
        }

        override fun decode(decoder: NoritoDecoder): ResolvedAccountAliasV1 = ResolvedAccountAliasV1(
            decodeField(decoder, ACCOUNT_ALIAS_NAME_ADAPTER),
            decodeField(decoder, DATASPACE_ID_ADAPTER),
        )
    }

    private val ALIAS_INTENT_ADAPTER = object : TypeAdapter<AliasIntentV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasIntentV1) {
            when (value) {
                is AliasIntentV1.Dataspace -> encodeEnum(encoder, 0, DATASPACE_INTENT_ADAPTER, value.intent)
                is AliasIntentV1.Domain -> encodeEnum(encoder, 1, DOMAIN_INTENT_ADAPTER, value.intent)
                is AliasIntentV1.AccountAlias -> encodeEnum(encoder, 2, ACCOUNT_INTENT_ADAPTER, value.intent)
            }
        }

        override fun decode(decoder: NoritoDecoder): AliasIntentV1 = when (val tag = U32.decode(decoder)) {
            0L -> AliasIntentV1.Dataspace(decodeEnumPayload(decoder, DATASPACE_INTENT_ADAPTER))
            1L -> AliasIntentV1.Domain(decodeEnumPayload(decoder, DOMAIN_INTENT_ADAPTER))
            2L -> AliasIntentV1.AccountAlias(decodeEnumPayload(decoder, ACCOUNT_INTENT_ADAPTER))
            else -> error("Unknown AliasIntentV1 discriminant: $tag")
        }
    }

    private val DATASPACE_INTENT_ADAPTER = object : TypeAdapter<AliasDataSpaceIntentV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasDataSpaceIntentV1) {
            encodeField(encoder, RESOLVED_DATASPACE_ADAPTER, value.dataspace)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.owner)
        }

        override fun decode(decoder: NoritoDecoder): AliasDataSpaceIntentV1 = AliasDataSpaceIntentV1(
            decodeField(decoder, RESOLVED_DATASPACE_ADAPTER),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
        )
    }

    private val DOMAIN_INTENT_ADAPTER = object : TypeAdapter<AliasDomainIntentV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasDomainIntentV1) {
            encodeField(encoder, RESOLVED_DOMAIN_ADAPTER, value.domain)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.owner)
        }

        override fun decode(decoder: NoritoDecoder): AliasDomainIntentV1 = AliasDomainIntentV1(
            decodeField(decoder, RESOLVED_DOMAIN_ADAPTER),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
        )
    }

    private val ACCOUNT_INTENT_ADAPTER = object : TypeAdapter<AliasAccountIntentV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasAccountIntentV1) {
            encodeField(encoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER, value.alias)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.targetAccount)
            encodeField(encoder, U32, value.provision.ordinal.toLong())
            encodeField(encoder, U32, value.role.ordinal.toLong())
        }

        override fun decode(decoder: NoritoDecoder): AliasAccountIntentV1 = AliasAccountIntentV1(
            decodeField(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
            enumAt(AccountProvisionV1.values(), decodeField(decoder, U32), "AccountProvisionV1"),
            enumAt(AccountAliasRoleV1.values(), decodeField(decoder, U32), "AccountAliasRoleV1"),
        )
    }

    private val TARGET_ADAPTER = object : TypeAdapter<AliasTargetV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasTargetV1) {
            when (value) {
                is AliasTargetV1.Dataspace -> encodeEnum(encoder, 0, RESOLVED_DATASPACE_ADAPTER, value.resource)
                is AliasTargetV1.Domain -> encodeEnum(encoder, 1, RESOLVED_DOMAIN_ADAPTER, value.resource)
                is AliasTargetV1.AccountAlias -> encodeEnum(encoder, 2, RESOLVED_ACCOUNT_ALIAS_ADAPTER, value.resource)
            }
        }

        override fun decode(decoder: NoritoDecoder): AliasTargetV1 = when (val tag = U32.decode(decoder)) {
            0L -> AliasTargetV1.Dataspace(decodeEnumPayload(decoder, RESOLVED_DATASPACE_ADAPTER))
            1L -> AliasTargetV1.Domain(decodeEnumPayload(decoder, RESOLVED_DOMAIN_ADAPTER))
            2L -> AliasTargetV1.AccountAlias(decodeEnumPayload(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER))
            else -> error("Unknown AliasTargetV1 discriminant: $tag")
        }
    }

    private val ACQUISITION_ADAPTER = object : TypeAdapter<AliasLeaseAcquisitionV1> {
        private val optionalU8 = NoritoAdapters.option(U8)

        override fun encode(encoder: NoritoEncoder, value: AliasLeaseAcquisitionV1) {
            encodeField(encoder, U8, value.termYears.toLong())
            encodeField(encoder, optionalU8, Optional.ofNullable(value.pricingClassHint?.toLong()))
        }

        override fun decode(decoder: NoritoDecoder): AliasLeaseAcquisitionV1 = AliasLeaseAcquisitionV1(
            Math.toIntExact(decodeField(decoder, U8)),
            decodeField(decoder, optionalU8).map(Math::toIntExact).orElse(null),
        )
    }

    private val QUOTE_GUARD_ADAPTER = object : TypeAdapter<AliasQuoteGuardV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasQuoteGuardV1) {
            encodeField(encoder, U16, value.expectedPolicyVersion.toLong())
            encodeField(encoder, ASSET_ID_ADAPTER, value.expectedPaymentAsset)
            encodeField(encoder, QUANTITY_ADAPTER, value.maxAmount)
            encodeField(encoder, U64, value.validUntilMs)
        }

        override fun decode(decoder: NoritoDecoder): AliasQuoteGuardV1 = AliasQuoteGuardV1(
            Math.toIntExact(decodeField(decoder, U16)),
            decodeField(decoder, ASSET_ID_ADAPTER),
            decodeField(decoder, QUANTITY_ADAPTER),
            decodeNonNegativeU64Field(decoder, "AliasQuoteGuardV1.valid_until_ms"),
        )
    }

    private val ENSURE_ADAPTER = object : TypeAdapter<EnsureAlias> {
        override fun encode(encoder: NoritoEncoder, value: EnsureAlias) {
            encodeField(encoder, ALIAS_INTENT_ADAPTER, value.intent)
            encodeField(encoder, ACQUISITION_ADAPTER, value.acquisition)
            encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard)
        }

        override fun decode(decoder: NoritoDecoder): EnsureAlias = EnsureAlias(
            decodeField(decoder, ALIAS_INTENT_ADAPTER),
            decodeField(decoder, ACQUISITION_ADAPTER),
            decodeField(decoder, QUOTE_GUARD_ADAPTER),
        )
    }

    private val RENEW_ADAPTER = object : TypeAdapter<RenewAliasLease> {
        override fun encode(encoder: NoritoEncoder, value: RenewAliasLease) {
            encodeField(encoder, TARGET_ADAPTER, value.target)
            encodeField(encoder, U64, value.expectedCurrentExpiryMs)
            encodeField(encoder, U64, value.targetExpiryMs)
            encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard)
        }

        override fun decode(decoder: NoritoDecoder): RenewAliasLease = RenewAliasLease(
            decodeField(decoder, TARGET_ADAPTER),
            decodeNonNegativeU64Field(decoder, "RenewAliasLease.expected_current_expiry_ms"),
            decodeNonNegativeU64Field(decoder, "RenewAliasLease.target_expiry_ms"),
            decodeField(decoder, QUOTE_GUARD_ADAPTER),
        )
    }

    private val AUTO_RENEW_CONFIG_ADAPTER = object : TypeAdapter<AliasAutoRenewConfigV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasAutoRenewConfigV1) {
            encodeField(encoder, U8, value.termYears.toLong())
            encodeField(encoder, U16, value.policyVersion.toLong())
            encodeField(encoder, ASSET_ID_ADAPTER, value.paymentAsset)
            encodeField(encoder, QUANTITY_ADAPTER, value.maxAmount)
            encodeField(encoder, U64, value.renewBeforeExpiryMs)
            encodeField(encoder, U64, value.retryBackoffMs)
            encodeField(encoder, U32, value.maxFailures)
        }

        override fun decode(decoder: NoritoDecoder): AliasAutoRenewConfigV1 = AliasAutoRenewConfigV1(
            Math.toIntExact(decodeField(decoder, U8)),
            Math.toIntExact(decodeField(decoder, U16)),
            decodeField(decoder, ASSET_ID_ADAPTER),
            decodeField(decoder, QUANTITY_ADAPTER),
            decodeNonNegativeU64Field(decoder, "AliasAutoRenewConfigV1.renew_before_expiry_ms"),
            decodeNonNegativeU64Field(decoder, "AliasAutoRenewConfigV1.retry_backoff_ms"),
            decodeField(decoder, U32),
        )
    }

    private val CONFIGURE_AUTO_RENEW_ADAPTER = object : TypeAdapter<ConfigureAliasAutoRenew> {
        private val optionalConfig = NoritoAdapters.option(AUTO_RENEW_CONFIG_ADAPTER)

        override fun encode(encoder: NoritoEncoder, value: ConfigureAliasAutoRenew) {
            encodeField(encoder, TARGET_ADAPTER, value.target)
            encodeField(encoder, U64, value.expectedRevision)
            encodeField(encoder, optionalConfig, Optional.ofNullable(value.config))
        }

        override fun decode(decoder: NoritoDecoder): ConfigureAliasAutoRenew = ConfigureAliasAutoRenew(
            decodeField(decoder, TARGET_ADAPTER),
            decodeNonNegativeU64Field(decoder, "ConfigureAliasAutoRenew.expected_revision"),
            decodeField(decoder, optionalConfig).orElse(null),
        )
    }

    private val REBIND_ADAPTER = object : TypeAdapter<RebindAccountAlias> {
        override fun encode(encoder: NoritoEncoder, value: RebindAccountAlias) {
            encodeField(encoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER, value.alias)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.expectedTargetAccount)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.newTargetAccount)
        }

        override fun decode(decoder: NoritoDecoder): RebindAccountAlias = RebindAccountAlias(
            decodeField(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
        )
    }

    private val PRIMARY_ADAPTER = object : TypeAdapter<CompareAndSetPrimaryAccountAlias> {
        private val optionalAlias = NoritoAdapters.option(RESOLVED_ACCOUNT_ALIAS_ADAPTER)

        override fun encode(encoder: NoritoEncoder, value: CompareAndSetPrimaryAccountAlias) {
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.account)
            encodeField(encoder, optionalAlias, Optional.ofNullable(value.expectedAlias))
            encodeField(encoder, optionalAlias, Optional.ofNullable(value.newAlias))
        }

        override fun decode(decoder: NoritoDecoder): CompareAndSetPrimaryAccountAlias =
            CompareAndSetPrimaryAccountAlias(
                decodeField(decoder, ACCOUNT_ID_ADAPTER),
                decodeField(decoder, optionalAlias).orElse(null),
                decodeField(decoder, optionalAlias).orElse(null),
            )
    }

    private val LIFECYCLE_OPERATION_ADAPTER = object : TypeAdapter<AliasLifecycleOperationV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasLifecycleOperationV1) {
            when (value) {
                is AliasLifecycleOperationV1.RenewLease -> encodeEnum(encoder, 0, RENEW_ADAPTER, value.renewal)
                is AliasLifecycleOperationV1.ConfigureAutoRenew ->
                    encodeEnum(encoder, 1, CONFIGURE_AUTO_RENEW_ADAPTER, value.configuration)
            }
        }

        override fun decode(decoder: NoritoDecoder): AliasLifecycleOperationV1 =
            when (val tag = U32.decode(decoder)) {
                0L -> AliasLifecycleOperationV1.RenewLease(decodeEnumPayload(decoder, RENEW_ADAPTER))
                1L -> AliasLifecycleOperationV1.ConfigureAutoRenew(
                    decodeEnumPayload(decoder, CONFIGURE_AUTO_RENEW_ADAPTER),
                )
                else -> error("Unknown AliasLifecycleOperationV1 discriminant: $tag")
            }
    }

    private val LEASE_QUOTE_ADAPTER = object : TypeAdapter<AliasLeaseQuoteV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasLeaseQuoteV1) {
            encodeField(encoder, TARGET_ADAPTER, value.target)
            encodeField(encoder, U8, value.pricingClass.toLong())
            encodeField(encoder, QUANTITY_ADAPTER, value.exactAmount)
            encodeField(encoder, QUOTE_GUARD_ADAPTER, value.guard)
            encodeField(encoder, U64, value.expiresAtMs)
            encodeField(encoder, U64, value.graceExpiresAtMs)
            encodeField(encoder, U64, value.redemptionExpiresAtMs)
        }

        override fun decode(decoder: NoritoDecoder): AliasLeaseQuoteV1 = AliasLeaseQuoteV1(
            decodeField(decoder, TARGET_ADAPTER),
            Math.toIntExact(decodeField(decoder, U8)),
            decodeField(decoder, QUANTITY_ADAPTER),
            decodeField(decoder, QUOTE_GUARD_ADAPTER),
            decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.expires_at_ms"),
            decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.grace_expires_at_ms"),
            decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.redemption_expires_at_ms"),
        )
    }

    private val PLAN_RESOURCE_ADAPTER = object : TypeAdapter<AliasPlanResourceV1> {
        private val optionalQuote = NoritoAdapters.option(LEASE_QUOTE_ADAPTER)
        private val optionalIndex = NoritoAdapters.option(U32)

        override fun encode(encoder: NoritoEncoder, value: AliasPlanResourceV1) {
            encodeField(encoder, ALIAS_INTENT_ADAPTER, value.intent)
            encodeField(encoder, U32, value.disposition.ordinal.toLong())
            encodeField(encoder, optionalQuote, Optional.ofNullable(value.quote))
            encodeField(encoder, optionalIndex, Optional.ofNullable(value.instructionIndex))
        }

        override fun decode(decoder: NoritoDecoder): AliasPlanResourceV1 = AliasPlanResourceV1(
            decodeField(decoder, ALIAS_INTENT_ADAPTER),
            enumAt(AliasPlanDispositionV1.values(), decodeField(decoder, U32), "AliasPlanDispositionV1"),
            decodeField(decoder, optionalQuote).orElse(null),
            decodeField(decoder, optionalIndex).orElse(null),
        )
    }

    private val FRAMED_INSTRUCTION_ADAPTER = object : TypeAdapter<AliasFramedInstructionV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasFramedInstructionV1) {
            encodeField(encoder, STRING, value.wireId)
            encodeField(encoder, RAW_BYTES, value.framedPayload)
        }

        override fun decode(decoder: NoritoDecoder): AliasFramedInstructionV1 = AliasFramedInstructionV1(
            decodeField(decoder, STRING),
            decodeField(decoder, RAW_BYTES),
        )
    }

    private val ASSET_TOTAL_ADAPTER = object : TypeAdapter<AliasAssetTotalV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasAssetTotalV1) {
            encodeField(encoder, ASSET_ID_ADAPTER, value.paymentAsset)
            encodeField(encoder, QUANTITY_ADAPTER, value.amount)
        }

        override fun decode(decoder: NoritoDecoder): AliasAssetTotalV1 = AliasAssetTotalV1(
            decodeField(decoder, ASSET_ID_ADAPTER),
            decodeField(decoder, QUANTITY_ADAPTER),
        )
    }

    private val DIAGNOSTIC_ADAPTER = object : TypeAdapter<AliasSetupDiagnosticV1> {
        private val optionalString = NoritoAdapters.option(STRING)

        override fun encode(encoder: NoritoEncoder, value: AliasSetupDiagnosticV1) {
            encodeField(encoder, U32, value.phase.ordinal.toLong())
            encodeField(encoder, STRING, value.code)
            encodeField(encoder, U32, value.severity.ordinal.toLong())
            encodeField(encoder, optionalString, Optional.ofNullable(value.resource))
            encodeField(encoder, optionalString, Optional.ofNullable(value.configPath))
            encodeField(encoder, optionalString, Optional.ofNullable(value.expected))
            encodeField(encoder, optionalString, Optional.ofNullable(value.actual))
            encodeField(encoder, STRING, value.remediation)
        }

        override fun decode(decoder: NoritoDecoder): AliasSetupDiagnosticV1 = AliasSetupDiagnosticV1(
            enumAt(AliasSetupValidationPhaseV1.values(), decodeField(decoder, U32), "AliasSetupValidationPhaseV1"),
            decodeField(decoder, STRING),
            enumAt(AliasSetupSeverityV1.values(), decodeField(decoder, U32), "AliasSetupSeverityV1"),
            decodeField(decoder, optionalString).orElse(null),
            decodeField(decoder, optionalString).orElse(null),
            decodeField(decoder, optionalString).orElse(null),
            decodeField(decoder, optionalString).orElse(null),
            decodeField(decoder, STRING),
        )
    }

    private val ANCHOR_ADAPTER = object : TypeAdapter<AliasPlanAnchorV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasPlanAnchorV1) {
            encodeField(encoder, U64, value.blockHeight)
            encodeField(encoder, HASH_ADAPTER, value.blockHash)
        }

        override fun decode(decoder: NoritoDecoder): AliasPlanAnchorV1 = AliasPlanAnchorV1(
            decodeNonNegativeU64Field(decoder, "AliasPlanAnchorV1.block_height"),
            decodeField(decoder, HASH_ADAPTER),
        )
    }

    private val RESOURCE_LIST = NoritoAdapters.sequence(PLAN_RESOURCE_ADAPTER)
    private val INSTRUCTION_LIST = NoritoAdapters.sequence(FRAMED_INSTRUCTION_ADAPTER)
    private val TOTAL_LIST = NoritoAdapters.sequence(ASSET_TOTAL_ADAPTER)
    private val DIAGNOSTIC_LIST = NoritoAdapters.sequence(DIAGNOSTIC_ADAPTER)

    private val PLAN_BODY_ADAPTER = object : TypeAdapter<AliasTransactionPlanBodyV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasTransactionPlanBodyV1) {
            encodeField(encoder, U8, value.version.toLong())
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
            encodeField(encoder, NETWORK_ID_ADAPTER, value.networkId)
            encodeField(encoder, ANCHOR_ADAPTER, value.anchor)
            encodeField(encoder, RESOURCE_LIST, value.resources)
            encodeField(encoder, INSTRUCTION_LIST, value.instructions)
            encodeField(encoder, TOTAL_LIST, value.totalsByAsset)
            encodeField(encoder, DIAGNOSTIC_LIST, value.warnings)
            encodeField(encoder, DIAGNOSTIC_LIST, value.blockers)
            encodeField(encoder, U64, value.validUntilMs)
        }

        override fun decode(decoder: NoritoDecoder): AliasTransactionPlanBodyV1 = AliasTransactionPlanBodyV1(
            Math.toIntExact(decodeField(decoder, U8)),
            decodeField(decoder, ACCOUNT_ID_ADAPTER),
            decodeField(decoder, NETWORK_ID_ADAPTER),
            decodeField(decoder, ANCHOR_ADAPTER),
            decodeField(decoder, RESOURCE_LIST),
            decodeField(decoder, INSTRUCTION_LIST),
            decodeField(decoder, TOTAL_LIST),
            decodeField(decoder, DIAGNOSTIC_LIST),
            decodeField(decoder, DIAGNOSTIC_LIST),
            decodeNonNegativeU64Field(decoder, "AliasTransactionPlanBodyV1.valid_until_ms"),
        )
    }

    private val OPTIONAL_INSTRUCTION = NoritoAdapters.option(FRAMED_INSTRUCTION_ADAPTER)
    private val OPTIONAL_QUOTE = NoritoAdapters.option(LEASE_QUOTE_ADAPTER)
    private val STRING_LIST = NoritoAdapters.sequence(STRING)

    private val ONBOARDING_REQUEST_ADAPTER = object : TypeAdapter<AccountOnboardingPlanRequestV1> {
        override fun encode(encoder: NoritoEncoder, value: AccountOnboardingPlanRequestV1) {
            encodeField(encoder, U8, value.version.toLong())
            encodeField(encoder, STRING, value.alias)
            encodeField(encoder, STRING, value.accountId)
            encodeField(encoder, STRING_LIST, value.permissions)
        }

        override fun decode(decoder: NoritoDecoder): AccountOnboardingPlanRequestV1 {
            val version = Math.toIntExact(decodeField(decoder, U8))
            val alias = decodeField(decoder, STRING)
            val accountId = decodeField(decoder, STRING)
            val permissions = decodeField(decoder, STRING_LIST)
            return AccountOnboardingPlanRequestV1(alias, accountId, permissions, version)
        }
    }

    private val ONBOARDING_PLAN_BODY_ADAPTER = object : TypeAdapter<AccountOnboardingPlanBodyV1> {
        override fun encode(encoder: NoritoEncoder, value: AccountOnboardingPlanBodyV1) {
            encodeField(encoder, U8, value.version.toLong())
            encodeField(encoder, ONBOARDING_REQUEST_ADAPTER, value.request)
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
            encodeField(encoder, NETWORK_ID_ADAPTER, value.networkId)
            encodeField(encoder, ANCHOR_ADAPTER, value.anchor)
            encodeField(encoder, PLAN_RESOURCE_ADAPTER, value.resource)
            encodeField(encoder, ACQUISITION_ADAPTER, value.acquisition)
            encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard)
            encodeField(encoder, INSTRUCTION_LIST, value.instructions)
            encodeField(
                encoder,
                OPTIONAL_INSTRUCTION,
                Optional.ofNullable(value.ownerAutoRenewInstruction),
            )
            encodeField(encoder, U64, value.validUntilMs)
        }

        override fun decode(decoder: NoritoDecoder): AccountOnboardingPlanBodyV1 =
            AccountOnboardingPlanBodyV1(
                Math.toIntExact(decodeField(decoder, U8)),
                decodeField(decoder, ONBOARDING_REQUEST_ADAPTER),
                decodeField(decoder, ACCOUNT_ID_ADAPTER),
                decodeField(decoder, NETWORK_ID_ADAPTER),
                decodeField(decoder, ANCHOR_ADAPTER),
                decodeField(decoder, PLAN_RESOURCE_ADAPTER),
                decodeField(decoder, ACQUISITION_ADAPTER),
                decodeField(decoder, QUOTE_GUARD_ADAPTER),
                decodeField(decoder, INSTRUCTION_LIST),
                decodeField(decoder, OPTIONAL_INSTRUCTION).orElse(null),
                decodeNonNegativeU64Field(decoder, "AccountOnboardingPlanBodyV1.valid_until_ms"),
            )
    }

    private val LIFECYCLE_PLAN_BODY_ADAPTER = object : TypeAdapter<AliasLifecycleTransactionPlanBodyV1> {
        override fun encode(encoder: NoritoEncoder, value: AliasLifecycleTransactionPlanBodyV1) {
            encodeField(encoder, U8, value.version.toLong())
            encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
            encodeField(encoder, NETWORK_ID_ADAPTER, value.networkId)
            encodeField(encoder, ANCHOR_ADAPTER, value.anchor)
            encodeField(encoder, LIFECYCLE_OPERATION_ADAPTER, value.operation)
            encodeField(encoder, U32, value.disposition.ordinal.toLong())
            encodeField(encoder, OPTIONAL_INSTRUCTION, Optional.ofNullable(value.instruction))
            encodeField(encoder, OPTIONAL_QUOTE, Optional.ofNullable(value.quote))
            encodeField(encoder, TOTAL_LIST, value.totalsByAsset)
            encodeField(encoder, DIAGNOSTIC_LIST, value.warnings)
            encodeField(encoder, DIAGNOSTIC_LIST, value.blockers)
            encodeField(encoder, U64, value.validUntilMs)
        }

        override fun decode(decoder: NoritoDecoder): AliasLifecycleTransactionPlanBodyV1 =
            AliasLifecycleTransactionPlanBodyV1(
                Math.toIntExact(decodeField(decoder, U8)),
                decodeField(decoder, ACCOUNT_ID_ADAPTER),
                decodeField(decoder, NETWORK_ID_ADAPTER),
                decodeField(decoder, ANCHOR_ADAPTER),
                decodeField(decoder, LIFECYCLE_OPERATION_ADAPTER),
                enumAt(
                    AliasLifecyclePlanDispositionV1.values(),
                    decodeField(decoder, U32),
                    "AliasLifecyclePlanDispositionV1",
                ),
                decodeField(decoder, OPTIONAL_INSTRUCTION).orElse(null),
                decodeField(decoder, OPTIONAL_QUOTE).orElse(null),
                decodeField(decoder, TOTAL_LIST),
                decodeField(decoder, DIAGNOSTIC_LIST),
                decodeField(decoder, DIAGNOSTIC_LIST),
                decodeNonNegativeU64Field(decoder, "AliasLifecycleTransactionPlanBodyV1.valid_until_ms"),
            )

    }

    private fun <T> encodeField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), encoder.flags and NoritoHeader.COMPACT_LEN != 0)
        encoder.writeBytes(payload)
    }

    private fun <T> decodeField(decoder: NoritoDecoder, adapter: TypeAdapter<T>): T {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length in 0..Int.MAX_VALUE.toLong()) { "Field payload too large" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after field payload" }
        return value
    }

    private fun <T> encodeEnum(encoder: NoritoEncoder, tag: Int, adapter: TypeAdapter<T>, value: T) {
        U32.encode(encoder, tag.toLong())
        encodeField(encoder, adapter, value)
    }

    private fun <T> decodeEnumPayload(decoder: NoritoDecoder, adapter: TypeAdapter<T>): T =
        decodeField(decoder, adapter)

    private fun decodeNonNegativeU64Field(decoder: NoritoDecoder, path: String): Long {
        val value = decodeField(decoder, U64)
        require(value >= 0) { "$path exceeds the SDK's signed timestamp bound" }
        return value
    }

    private fun encodeFixedBytes(encoder: NoritoEncoder, bytes: ByteArray) {
        val compact = encoder.flags and NoritoHeader.COMPACT_LEN != 0
        bytes.forEach {
            encoder.writeLength(1, compact)
            encoder.writeByte(it.toInt())
        }
    }

    private fun decodeFixedBytes(decoder: NoritoDecoder, count: Int, path: String): ByteArray =
        ByteArray(count) { index ->
            require(decoder.readLength(decoder.compactLenActive()) == 1L) {
                "$path element $index must contain exactly one byte"
            }
            decoder.readByte().toByte()
        }

    private fun encodeBigIntegerField(encoder: NoritoEncoder, value: BigInteger) {
        val child = encoder.childEncoder()
        val bytes = value.toCanonicalLittleEndian()
        child.writeUInt(bytes.size.toLong(), 32)
        child.writeBytes(bytes)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), encoder.flags and NoritoHeader.COMPACT_LEN != 0)
        encoder.writeBytes(payload)
    }

    private fun decodeBigIntegerField(decoder: NoritoDecoder): BigInteger {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length in 4..Int.MAX_VALUE.toLong()) { "BigInteger field length is invalid" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val byteLength = child.readUInt(32)
        require(byteLength in 0..Int.MAX_VALUE.toLong()) { "BigInteger payload is too large" }
        val bytes = child.readBytes(byteLength.toInt())
        require(child.remaining() == 0) { "Trailing bytes after BigInteger payload" }
        val value = bytes.fromCanonicalLittleEndian()
        require(value.toCanonicalLittleEndian().contentEquals(bytes)) { "BigInteger payload is not canonical" }
        return value
    }

    private fun BigInteger.toCanonicalLittleEndian(): ByteArray {
        if (signum() == 0) return ByteArray(0)
        val bigEndian = toByteArray()
        val littleEndian = ByteArray(bigEndian.size) { bigEndian[bigEndian.lastIndex - it] }
        var size = littleEndian.size
        if (signum() > 0) {
            while (size > 1 && littleEndian[size - 1] == 0.toByte() && littleEndian[size - 2].toInt() and 0x80 == 0) size--
        } else {
            while (size > 1 && littleEndian[size - 1] == 0xff.toByte() && littleEndian[size - 2].toInt() and 0x80 != 0) size--
        }
        return littleEndian.copyOf(size)
    }

    private fun ByteArray.fromCanonicalLittleEndian(): BigInteger {
        if (isEmpty()) return BigInteger.ZERO
        return BigInteger(ByteArray(size) { this[lastIndex - it] })
    }

    private fun <T> enumAt(values: Array<T>, tag: Long, name: String): T {
        require(tag in values.indices.map(Int::toLong)) { "Unknown $name discriminant: $tag" }
        return values[tag.toInt()]
    }

    private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }
}

/** Default canonical setup-plan body encoder. */
object DefaultAliasPlanBodyNoritoEncoder : AliasPlanBodyNoritoEncoder {
    override fun encode(body: AliasTransactionPlanBodyV1): ByteArray = AliasNoritoCodec.encodePlanBody(body)
}

/** Default typed EnsureAlias registry codec. */
object DefaultAliasEnsureInstructionFrameCodec : AliasEnsureInstructionFrameCodec {
    override fun decodeAndReencode(
        wireId: String,
        framedPayload: ByteArray,
        chainDiscriminant: Int,
    ): DecodedEnsureAliasFrame {
        require(wireId == EnsureAlias.WIRE_ID) { "unsupported alias setup wire id: $wireId" }
        val value =
            AliasNoritoCodec.decodeEnsureAliasFrame(
                framedPayload,
                chainDiscriminant,
            )
        return DecodedEnsureAliasFrame(value, AliasNoritoCodec.encodeEnsureAliasFrame(value))
    }
}

/** Default canonical lifecycle-plan body encoder. */
object DefaultAliasLifecyclePlanBodyNoritoEncoder : AliasLifecyclePlanBodyNoritoEncoder {
    override fun encode(body: AliasLifecycleTransactionPlanBodyV1): ByteArray =
        AliasNoritoCodec.encodeLifecyclePlanBody(body)
}

/** Default typed renewal/auto-renew registry codec. */
object DefaultAliasLifecycleInstructionFrameCodec : AliasLifecycleInstructionFrameCodec {
    override fun decodeAndReencode(
        wireId: String,
        framedPayload: ByteArray,
        chainDiscriminant: Int,
    ): DecodedAliasLifecycleFrame {
        return when (wireId) {
            RenewAliasLease.WIRE_ID -> {
                val value =
                    AliasNoritoCodec.decodeRenewAliasLeaseFrame(
                        framedPayload,
                        chainDiscriminant,
                    )
                DecodedAliasLifecycleFrame(
                    AliasLifecycleOperationV1.RenewLease(value),
                    AliasNoritoCodec.encodeRenewAliasLeaseFrame(value),
                )
            }
            ConfigureAliasAutoRenew.WIRE_ID -> {
                val value =
                    AliasNoritoCodec.decodeConfigureAutoRenewFrame(
                        framedPayload,
                        chainDiscriminant,
                    )
                DecodedAliasLifecycleFrame(
                    AliasLifecycleOperationV1.ConfigureAutoRenew(value),
                    AliasNoritoCodec.encodeConfigureAutoRenewFrame(value),
                )
            }
            else -> error("unsupported alias lifecycle wire id: $wireId")
        }
    }
}
