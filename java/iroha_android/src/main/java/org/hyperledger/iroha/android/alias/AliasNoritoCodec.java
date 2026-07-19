package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Canonical V1 Norito codecs used by alias planners and local apply. */
public final class AliasNoritoCodec {
  private static final String ENSURE_SCHEMA =
      "iroha_data_model::isi::alias_setup::EnsureAlias";
  private static final String RENEW_SCHEMA =
      "iroha_data_model::isi::alias_setup::RenewAliasLease";
  private static final String AUTO_RENEW_SCHEMA =
      "iroha_data_model::isi::alias_setup::ConfigureAliasAutoRenew";
  private static final String REBIND_SCHEMA =
      "iroha_data_model::isi::alias_setup::RebindAccountAlias";
  private static final String PRIMARY_SCHEMA =
      "iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias";

  private static final TypeAdapter<Long> U8 = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> U16 = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> U32 = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> U64 = NoritoAdapters.uint(64);
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTES = NoritoAdapters.rawByteVecAdapter();

  private AliasNoritoCodec() {}

  /** Encodes the exact bare setup-plan body committed by the planner hash. */
  public static byte[] encodePlanBody(
      final AliasSetupModels.AliasTransactionPlanBodyV1 body) {
    return NoritoCodec.encodeAdaptive(Objects.requireNonNull(body, "body"), PLAN_BODY_ADAPTER)
        .payload();
  }

  /** Decodes an exact bare setup-plan body. */
  public static AliasSetupModels.AliasTransactionPlanBodyV1 decodePlanBody(
      final byte[] payload) {
    return NoritoCodec.decodeAdaptive(
        Objects.requireNonNull(payload, "payload"), PLAN_BODY_ADAPTER);
  }

  /** Encodes the exact bare lifecycle-plan body committed by the planner hash. */
  public static byte[] encodeLifecyclePlanBody(
      final AliasLifecycleTransactionPlanBodyV1 body) {
    return NoritoCodec.encodeAdaptive(
            Objects.requireNonNull(body, "body"), LIFECYCLE_PLAN_BODY_ADAPTER)
        .payload();
  }

  /** Decodes an exact bare lifecycle-plan body. */
  public static AliasLifecycleTransactionPlanBodyV1 decodeLifecyclePlanBody(
      final byte[] payload) {
    return NoritoCodec.decodeAdaptive(
        Objects.requireNonNull(payload, "payload"), LIFECYCLE_PLAN_BODY_ADAPTER);
  }

  /** Encodes the exact bare sponsored-onboarding receipt body signed by Torii. */
  public static byte[] encodeOnboardingPlanBody(final AccountOnboardingPlanBodyV1 body) {
    return NoritoCodec.encodeAdaptive(
            Objects.requireNonNull(body, "body"), ONBOARDING_PLAN_BODY_ADAPTER)
        .payload();
  }

  /** Decodes an exact bare sponsored-onboarding receipt body. */
  public static AccountOnboardingPlanBodyV1 decodeOnboardingPlanBody(final byte[] payload) {
    return NoritoCodec.decodeAdaptive(
        Objects.requireNonNull(payload, "payload"), ONBOARDING_PLAN_BODY_ADAPTER);
  }

  /** Encodes one typed EnsureAlias instruction. */
  public static byte[] encodeEnsureAliasFrame(final EnsureAlias value) {
    return NoritoCodec.encode(value, ENSURE_SCHEMA, ENSURE_ADAPTER);
  }

  /** Decodes one schema-bound EnsureAlias frame. */
  public static EnsureAlias decodeEnsureAliasFrame(final byte[] frame) {
    return NoritoCodec.decode(frame, ENSURE_ADAPTER, ENSURE_SCHEMA);
  }

  /** Encodes one typed renewal instruction. */
  public static byte[] encodeRenewAliasLeaseFrame(final RenewAliasLease value) {
    return NoritoCodec.encode(value, RENEW_SCHEMA, RENEW_ADAPTER);
  }

  /** Decodes one schema-bound renewal frame. */
  public static RenewAliasLease decodeRenewAliasLeaseFrame(final byte[] frame) {
    return NoritoCodec.decode(frame, RENEW_ADAPTER, RENEW_SCHEMA);
  }

  /** Encodes one typed auto-renew instruction. */
  public static byte[] encodeConfigureAutoRenewFrame(final ConfigureAliasAutoRenew value) {
    return NoritoCodec.encode(value, AUTO_RENEW_SCHEMA, CONFIGURE_AUTO_RENEW_ADAPTER);
  }

  /** Decodes one schema-bound auto-renew frame. */
  public static ConfigureAliasAutoRenew decodeConfigureAutoRenewFrame(final byte[] frame) {
    return NoritoCodec.decode(frame, CONFIGURE_AUTO_RENEW_ADAPTER, AUTO_RENEW_SCHEMA);
  }

  /** Encodes one typed account-alias rebind instruction. */
  public static byte[] encodeRebindAccountAliasFrame(final RebindAccountAlias value) {
    return NoritoCodec.encode(value, REBIND_SCHEMA, REBIND_ADAPTER);
  }

  /** Decodes one schema-bound account-alias rebind frame. */
  public static RebindAccountAlias decodeRebindAccountAliasFrame(final byte[] frame) {
    return NoritoCodec.decode(frame, REBIND_ADAPTER, REBIND_SCHEMA);
  }

  /** Encodes one typed primary-alias compare-and-set instruction. */
  public static byte[] encodeCompareAndSetPrimaryAliasFrame(
      final CompareAndSetPrimaryAccountAlias value) {
    return NoritoCodec.encode(value, PRIMARY_SCHEMA, PRIMARY_ADAPTER);
  }

  /** Decodes one schema-bound primary-alias compare-and-set frame. */
  public static CompareAndSetPrimaryAccountAlias decodeCompareAndSetPrimaryAliasFrame(
      final byte[] frame) {
    return NoritoCodec.decode(frame, PRIMARY_ADAPTER, PRIMARY_SCHEMA);
  }

  private static final TypeAdapter<BigInteger> BIG_U64_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final BigInteger value) {
          AliasNameSupport.requireU64(value, "u64");
          encoder.writeUInt(value.longValue(), 64);
        }

        @Override
        public BigInteger decode(final NoritoDecoder decoder) {
          final long value = decoder.readUInt(64);
          return value >= 0
              ? BigInteger.valueOf(value)
              : BigInteger.valueOf(value & Long.MAX_VALUE).setBit(63);
        }
      };

  private static final TypeAdapter<String> ACCOUNT_ID_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          encoder.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value));
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          return TransferWirePayloadEncoder.decodeAccountIdPayload(
              decoder.readBytes(decoder.remaining()), decoder.flags(), decoder.flagsHint());
        }
      };

  private static final TypeAdapter<String> CHAIN_ID_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          encodeField(encoder, STRING, value);
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          return decodeField(decoder, STRING);
        }
      };

  private static final TypeAdapter<String> HASH_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          final byte[] hash = AliasNameSupport.decodeHash(value);
          if (hash == null) throw new IllegalArgumentException("invalid hash");
          encoder.writeBytes(hash);
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          if (decoder.remaining() != 32) {
            throw new IllegalArgumentException("Hash must contain 32 bytes");
          }
          return hex(decoder.readBytes(32));
        }

        @Override
        public int fixedSize() {
          return 32;
        }
      };

  private static final TypeAdapter<String> ASSET_ID_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          encodeFixedBytes(encoder, AssetDefinitionIdEncoder.parseAddressBytes(value));
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          return AssetDefinitionIdEncoder.encodeFromBytes(
              decodeFixedBytes(decoder, 16, "AssetDefinitionId"));
        }
      };

  private static final TypeAdapter<String> QUANTITY_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          final NumericV1.QuantityValue quantity = NumericV1.QuantityValue.parseCanonical(value);
          encodeBigIntegerField(encoder, quantity.mantissa());
          encodeField(encoder, U32, (long) quantity.scale());
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          final BigInteger mantissa = decodeBigIntegerField(decoder);
          final int scale = Math.toIntExact(decodeField(decoder, U32));
          return NumericV1.QuantityValue.of(mantissa, scale).toString();
        }
      };

  private static final TypeAdapter<BigInteger> DATASPACE_ID_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final BigInteger value) {
          encodeField(encoder, BIG_U64_ADAPTER, value);
        }

        @Override
        public BigInteger decode(final NoritoDecoder decoder) {
          return decodeField(decoder, BIG_U64_ADAPTER);
        }
      };

  private static final TypeAdapter<AccountAliasName> ACCOUNT_ALIAS_NAME_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<String>> optionalName = NoritoAdapters.option(STRING);

        @Override
        public void encode(final NoritoEncoder encoder, final AccountAliasName value) {
          encodeField(encoder, STRING, value.label());
          encodeField(encoder, optionalName, Optional.ofNullable(value.domain()));
          encodeField(encoder, STRING, value.dataspace());
        }

        @Override
        public AccountAliasName decode(final NoritoDecoder decoder) {
          return new AccountAliasName(
              decodeField(decoder, STRING),
              decodeField(decoder, optionalName).orElse(null),
              decodeField(decoder, STRING));
        }
      };

  private static final TypeAdapter<ResolvedDataSpaceV1> RESOLVED_DATASPACE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ResolvedDataSpaceV1 value) {
          encodeField(encoder, STRING, value.canonicalName());
          encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId());
        }

        @Override
        public ResolvedDataSpaceV1 decode(final NoritoDecoder decoder) {
          return new ResolvedDataSpaceV1(
              decodeField(decoder, STRING), decodeField(decoder, DATASPACE_ID_ADAPTER));
        }
      };

  private static final TypeAdapter<String> DOMAIN_ID_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          final int dot = value.indexOf('.');
          if (dot <= 0 || dot != value.lastIndexOf('.') || dot >= value.length() - 1) {
            throw new IllegalArgumentException("domain must use domain.dataspace format");
          }
          encodeField(encoder, STRING, value.substring(0, dot));
          encodeField(encoder, STRING, value.substring(dot + 1));
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          return decodeField(decoder, STRING) + "." + decodeField(decoder, STRING);
        }
      };

  private static final TypeAdapter<ResolvedDomainV1> RESOLVED_DOMAIN_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ResolvedDomainV1 value) {
          encodeField(encoder, DOMAIN_ID_ADAPTER, value.canonicalName());
          encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId());
        }

        @Override
        public ResolvedDomainV1 decode(final NoritoDecoder decoder) {
          return new ResolvedDomainV1(
              decodeField(decoder, DOMAIN_ID_ADAPTER),
              decodeField(decoder, DATASPACE_ID_ADAPTER));
        }
      };

  private static final TypeAdapter<ResolvedAccountAliasV1> RESOLVED_ACCOUNT_ALIAS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ResolvedAccountAliasV1 value) {
          encodeField(encoder, ACCOUNT_ALIAS_NAME_ADAPTER, value.canonicalName());
          encodeField(encoder, DATASPACE_ID_ADAPTER, value.dataspaceId());
        }

        @Override
        public ResolvedAccountAliasV1 decode(final NoritoDecoder decoder) {
          return new ResolvedAccountAliasV1(
              decodeField(decoder, ACCOUNT_ALIAS_NAME_ADAPTER),
              decodeField(decoder, DATASPACE_ID_ADAPTER));
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasDataSpaceIntentV1>
      DATASPACE_INTENT_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder,
                final AliasSetupModels.AliasDataSpaceIntentV1 value) {
              encodeField(encoder, RESOLVED_DATASPACE_ADAPTER, value.dataspace());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.owner());
            }

            @Override
            public AliasSetupModels.AliasDataSpaceIntentV1 decode(
                final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasDataSpaceIntentV1(
                  decodeField(decoder, RESOLVED_DATASPACE_ADAPTER),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasDomainIntentV1>
      DOMAIN_INTENT_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final AliasSetupModels.AliasDomainIntentV1 value) {
              encodeField(encoder, RESOLVED_DOMAIN_ADAPTER, value.domain());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.owner());
            }

            @Override
            public AliasSetupModels.AliasDomainIntentV1 decode(final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasDomainIntentV1(
                  decodeField(decoder, RESOLVED_DOMAIN_ADAPTER),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasAccountIntentV1>
      ACCOUNT_INTENT_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final AliasSetupModels.AliasAccountIntentV1 value) {
              encodeField(encoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER, value.alias());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.targetAccount());
              encodeField(encoder, U32, (long) value.provision().ordinal());
              encodeField(encoder, U32, (long) value.role().ordinal());
            }

            @Override
            public AliasSetupModels.AliasAccountIntentV1 decode(final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasAccountIntentV1(
                  decodeField(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER),
                  enumAt(
                      AliasSetupModels.AccountProvisionV1.values(),
                      decodeField(decoder, U32),
                      "AccountProvisionV1"),
                  enumAt(
                      AliasSetupModels.AccountAliasRoleV1.values(),
                      decodeField(decoder, U32),
                      "AccountAliasRoleV1"));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasIntentV1> ALIAS_INTENT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final AliasSetupModels.AliasIntentV1 value) {
          if (value instanceof AliasSetupModels.DataspaceIntent) {
            encodeEnum(
                encoder,
                0,
                DATASPACE_INTENT_ADAPTER,
                ((AliasSetupModels.DataspaceIntent) value).intent());
          } else if (value instanceof AliasSetupModels.DomainIntent) {
            encodeEnum(
                encoder,
                1,
                DOMAIN_INTENT_ADAPTER,
                ((AliasSetupModels.DomainIntent) value).intent());
          } else if (value instanceof AliasSetupModels.AccountAliasIntent) {
            encodeEnum(
                encoder,
                2,
                ACCOUNT_INTENT_ADAPTER,
                ((AliasSetupModels.AccountAliasIntent) value).intent());
          } else {
            throw new IllegalArgumentException("unsupported AliasIntentV1");
          }
        }

        @Override
        public AliasSetupModels.AliasIntentV1 decode(final NoritoDecoder decoder) {
          final long tag = U32.decode(decoder);
          if (tag == 0) {
            return new AliasSetupModels.DataspaceIntent(
                decodeEnumPayload(decoder, DATASPACE_INTENT_ADAPTER));
          }
          if (tag == 1) {
            return new AliasSetupModels.DomainIntent(
                decodeEnumPayload(decoder, DOMAIN_INTENT_ADAPTER));
          }
          if (tag == 2) {
            return new AliasSetupModels.AccountAliasIntent(
                decodeEnumPayload(decoder, ACCOUNT_INTENT_ADAPTER));
          }
          throw new IllegalArgumentException("Unknown AliasIntentV1 discriminant: " + tag);
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasTargetV1> TARGET_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final AliasSetupModels.AliasTargetV1 value) {
          if (value instanceof AliasSetupModels.DataspaceTarget) {
            encodeEnum(
                encoder,
                0,
                RESOLVED_DATASPACE_ADAPTER,
                ((AliasSetupModels.DataspaceTarget) value).resource());
          } else if (value instanceof AliasSetupModels.DomainTarget) {
            encodeEnum(
                encoder,
                1,
                RESOLVED_DOMAIN_ADAPTER,
                ((AliasSetupModels.DomainTarget) value).resource());
          } else if (value instanceof AliasSetupModels.AccountAliasTarget) {
            encodeEnum(
                encoder,
                2,
                RESOLVED_ACCOUNT_ALIAS_ADAPTER,
                ((AliasSetupModels.AccountAliasTarget) value).resource());
          } else {
            throw new IllegalArgumentException("unsupported AliasTargetV1");
          }
        }

        @Override
        public AliasSetupModels.AliasTargetV1 decode(final NoritoDecoder decoder) {
          final long tag = U32.decode(decoder);
          if (tag == 0) {
            return new AliasSetupModels.DataspaceTarget(
                decodeEnumPayload(decoder, RESOLVED_DATASPACE_ADAPTER));
          }
          if (tag == 1) {
            return new AliasSetupModels.DomainTarget(
                decodeEnumPayload(decoder, RESOLVED_DOMAIN_ADAPTER));
          }
          if (tag == 2) {
            return new AliasSetupModels.AccountAliasTarget(
                decodeEnumPayload(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER));
          }
          throw new IllegalArgumentException("Unknown AliasTargetV1 discriminant: " + tag);
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasLeaseAcquisitionV1>
      ACQUISITION_ADAPTER =
          new TypeAdapter<>() {
            private final TypeAdapter<Optional<Long>> optionalU8 = NoritoAdapters.option(U8);

            @Override
            public void encode(
                final NoritoEncoder encoder,
                final AliasSetupModels.AliasLeaseAcquisitionV1 value) {
              encodeField(encoder, U8, (long) value.termYears());
              encodeField(
                  encoder,
                  optionalU8,
                  value.pricingClassHint() == null
                      ? Optional.empty()
                      : Optional.of(value.pricingClassHint().longValue()));
            }

            @Override
            public AliasSetupModels.AliasLeaseAcquisitionV1 decode(
                final NoritoDecoder decoder) {
              final int term = Math.toIntExact(decodeField(decoder, U8));
              final Optional<Long> pricing = decodeField(decoder, optionalU8);
              return new AliasSetupModels.AliasLeaseAcquisitionV1(
                  term, pricing.isPresent() ? Math.toIntExact(pricing.get()) : null);
            }
          };

  private static final TypeAdapter<AliasQuoteGuardV1> QUOTE_GUARD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AliasQuoteGuardV1 value) {
          encodeField(encoder, U16, (long) value.expectedPolicyVersion());
          encodeField(encoder, ASSET_ID_ADAPTER, value.expectedPaymentAsset());
          encodeField(encoder, QUANTITY_ADAPTER, value.maxAmount());
          encodeField(encoder, U64, value.validUntilMs());
        }

        @Override
        public AliasQuoteGuardV1 decode(final NoritoDecoder decoder) {
          return new AliasQuoteGuardV1(
              Math.toIntExact(decodeField(decoder, U16)),
              decodeField(decoder, ASSET_ID_ADAPTER),
              decodeField(decoder, QUANTITY_ADAPTER),
              decodeNonNegativeU64Field(decoder, "AliasQuoteGuardV1.valid_until_ms"));
        }
      };

  private static final TypeAdapter<EnsureAlias> ENSURE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final EnsureAlias value) {
          encodeField(encoder, ALIAS_INTENT_ADAPTER, value.intent());
          encodeField(encoder, ACQUISITION_ADAPTER, value.acquisition());
          encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard());
        }

        @Override
        public EnsureAlias decode(final NoritoDecoder decoder) {
          return new EnsureAlias(
              decodeField(decoder, ALIAS_INTENT_ADAPTER),
              decodeField(decoder, ACQUISITION_ADAPTER),
              decodeField(decoder, QUOTE_GUARD_ADAPTER));
        }
      };

  private static final TypeAdapter<RenewAliasLease> RENEW_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RenewAliasLease value) {
          encodeField(encoder, TARGET_ADAPTER, value.target());
          encodeField(encoder, U64, value.expectedCurrentExpiryMs());
          encodeField(encoder, U64, value.targetExpiryMs());
          encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard());
        }

        @Override
        public RenewAliasLease decode(final NoritoDecoder decoder) {
          return new RenewAliasLease(
              decodeField(decoder, TARGET_ADAPTER),
              decodeNonNegativeU64Field(decoder, "RenewAliasLease.expected_current_expiry_ms"),
              decodeNonNegativeU64Field(decoder, "RenewAliasLease.target_expiry_ms"),
              decodeField(decoder, QUOTE_GUARD_ADAPTER));
        }
      };

  private static final TypeAdapter<AliasAutoRenewConfigV1> AUTO_RENEW_CONFIG_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AliasAutoRenewConfigV1 value) {
          encodeField(encoder, U8, (long) value.termYears());
          encodeField(encoder, U16, (long) value.policyVersion());
          encodeField(encoder, ASSET_ID_ADAPTER, value.paymentAsset());
          encodeField(encoder, QUANTITY_ADAPTER, value.maxAmount());
          encodeField(encoder, U64, value.renewBeforeExpiryMs());
          encodeField(encoder, U64, value.retryBackoffMs());
          encodeField(encoder, U32, value.maxFailures());
        }

        @Override
        public AliasAutoRenewConfigV1 decode(final NoritoDecoder decoder) {
          return new AliasAutoRenewConfigV1(
              Math.toIntExact(decodeField(decoder, U8)),
              Math.toIntExact(decodeField(decoder, U16)),
              decodeField(decoder, ASSET_ID_ADAPTER),
              decodeField(decoder, QUANTITY_ADAPTER),
              decodeNonNegativeU64Field(decoder, "AliasAutoRenewConfigV1.renew_before_expiry_ms"),
              decodeNonNegativeU64Field(decoder, "AliasAutoRenewConfigV1.retry_backoff_ms"),
              decodeField(decoder, U32));
        }
      };

  private static final TypeAdapter<ConfigureAliasAutoRenew> CONFIGURE_AUTO_RENEW_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<AliasAutoRenewConfigV1>> optionalConfig =
            NoritoAdapters.option(AUTO_RENEW_CONFIG_ADAPTER);

        @Override
        public void encode(final NoritoEncoder encoder, final ConfigureAliasAutoRenew value) {
          encodeField(encoder, TARGET_ADAPTER, value.target());
          encodeField(encoder, U64, value.expectedRevision());
          encodeField(encoder, optionalConfig, Optional.ofNullable(value.config()));
        }

        @Override
        public ConfigureAliasAutoRenew decode(final NoritoDecoder decoder) {
          return new ConfigureAliasAutoRenew(
              decodeField(decoder, TARGET_ADAPTER),
              decodeNonNegativeU64Field(decoder, "ConfigureAliasAutoRenew.expected_revision"),
              decodeField(decoder, optionalConfig).orElse(null));
        }
      };

  private static final TypeAdapter<RebindAccountAlias> REBIND_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RebindAccountAlias value) {
          encodeField(encoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER, value.alias());
          encodeField(encoder, ACCOUNT_ID_ADAPTER, value.expectedTargetAccount());
          encodeField(encoder, ACCOUNT_ID_ADAPTER, value.newTargetAccount());
        }

        @Override
        public RebindAccountAlias decode(final NoritoDecoder decoder) {
          return new RebindAccountAlias(
              decodeField(decoder, RESOLVED_ACCOUNT_ALIAS_ADAPTER),
              decodeField(decoder, ACCOUNT_ID_ADAPTER),
              decodeField(decoder, ACCOUNT_ID_ADAPTER));
        }
      };

  private static final TypeAdapter<CompareAndSetPrimaryAccountAlias> PRIMARY_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<ResolvedAccountAliasV1>> optionalAlias =
            NoritoAdapters.option(RESOLVED_ACCOUNT_ALIAS_ADAPTER);

        @Override
        public void encode(
            final NoritoEncoder encoder, final CompareAndSetPrimaryAccountAlias value) {
          encodeField(encoder, ACCOUNT_ID_ADAPTER, value.account());
          encodeField(encoder, optionalAlias, Optional.ofNullable(value.expectedAlias()));
          encodeField(encoder, optionalAlias, Optional.ofNullable(value.newAlias()));
        }

        @Override
        public CompareAndSetPrimaryAccountAlias decode(final NoritoDecoder decoder) {
          return new CompareAndSetPrimaryAccountAlias(
              decodeField(decoder, ACCOUNT_ID_ADAPTER),
              decodeField(decoder, optionalAlias).orElse(null),
              decodeField(decoder, optionalAlias).orElse(null));
        }
      };

  private static final TypeAdapter<AliasLifecycleOperationV1> LIFECYCLE_OPERATION_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AliasLifecycleOperationV1 value) {
          if (value instanceof AliasLifecycleOperationV1.RenewLease) {
            encodeEnum(
                encoder,
                0,
                RENEW_ADAPTER,
                ((AliasLifecycleOperationV1.RenewLease) value).renewal());
          } else if (value instanceof AliasLifecycleOperationV1.ConfigureAutoRenew) {
            encodeEnum(
                encoder,
                1,
                CONFIGURE_AUTO_RENEW_ADAPTER,
                ((AliasLifecycleOperationV1.ConfigureAutoRenew) value).configuration());
          } else {
            throw new IllegalArgumentException("unsupported AliasLifecycleOperationV1");
          }
        }

        @Override
        public AliasLifecycleOperationV1 decode(final NoritoDecoder decoder) {
          final long tag = U32.decode(decoder);
          if (tag == 0) {
            return new AliasLifecycleOperationV1.RenewLease(
                decodeEnumPayload(decoder, RENEW_ADAPTER));
          }
          if (tag == 1) {
            return new AliasLifecycleOperationV1.ConfigureAutoRenew(
                decodeEnumPayload(decoder, CONFIGURE_AUTO_RENEW_ADAPTER));
          }
          throw new IllegalArgumentException(
              "Unknown AliasLifecycleOperationV1 discriminant: " + tag);
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasLeaseQuoteV1> LEASE_QUOTE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final AliasSetupModels.AliasLeaseQuoteV1 value) {
          encodeField(encoder, TARGET_ADAPTER, value.target());
          encodeField(encoder, U8, (long) value.pricingClass());
          encodeField(encoder, QUANTITY_ADAPTER, value.exactAmount());
          encodeField(encoder, QUOTE_GUARD_ADAPTER, value.guard());
          encodeField(encoder, U64, value.expiresAtMs());
          encodeField(encoder, U64, value.graceExpiresAtMs());
          encodeField(encoder, U64, value.redemptionExpiresAtMs());
        }

        @Override
        public AliasSetupModels.AliasLeaseQuoteV1 decode(final NoritoDecoder decoder) {
          return new AliasSetupModels.AliasLeaseQuoteV1(
              decodeField(decoder, TARGET_ADAPTER),
              Math.toIntExact(decodeField(decoder, U8)),
              decodeField(decoder, QUANTITY_ADAPTER),
              decodeField(decoder, QUOTE_GUARD_ADAPTER),
              decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.expires_at_ms"),
              decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.grace_expires_at_ms"),
              decodeNonNegativeU64Field(decoder, "AliasLeaseQuoteV1.redemption_expires_at_ms"));
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasPlanResourceV1>
      PLAN_RESOURCE_ADAPTER =
          new TypeAdapter<>() {
            private final TypeAdapter<Optional<AliasSetupModels.AliasLeaseQuoteV1>> optionalQuote =
                NoritoAdapters.option(LEASE_QUOTE_ADAPTER);
            private final TypeAdapter<Optional<Long>> optionalIndex = NoritoAdapters.option(U32);

            @Override
            public void encode(
                final NoritoEncoder encoder, final AliasSetupModels.AliasPlanResourceV1 value) {
              encodeField(encoder, ALIAS_INTENT_ADAPTER, value.intent());
              encodeField(encoder, U32, (long) value.disposition().ordinal());
              encodeField(encoder, optionalQuote, Optional.ofNullable(value.quote()));
              encodeField(encoder, optionalIndex, Optional.ofNullable(value.instructionIndex()));
            }

            @Override
            public AliasSetupModels.AliasPlanResourceV1 decode(final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasPlanResourceV1(
                  decodeField(decoder, ALIAS_INTENT_ADAPTER),
                  enumAt(
                      AliasSetupModels.AliasPlanDispositionV1.values(),
                      decodeField(decoder, U32),
                      "AliasPlanDispositionV1"),
                  decodeField(decoder, optionalQuote).orElse(null),
                  decodeField(decoder, optionalIndex).orElse(null));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasFramedInstructionV1>
      FRAMED_INSTRUCTION_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder,
                final AliasSetupModels.AliasFramedInstructionV1 value) {
              encodeField(encoder, STRING, value.wireId());
              encodeField(encoder, RAW_BYTES, value.framedPayload());
            }

            @Override
            public AliasSetupModels.AliasFramedInstructionV1 decode(
                final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasFramedInstructionV1(
                  decodeField(decoder, STRING), decodeField(decoder, RAW_BYTES));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasAssetTotalV1> ASSET_TOTAL_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final AliasSetupModels.AliasAssetTotalV1 value) {
          encodeField(encoder, ASSET_ID_ADAPTER, value.paymentAsset());
          encodeField(encoder, QUANTITY_ADAPTER, value.amount());
        }

        @Override
        public AliasSetupModels.AliasAssetTotalV1 decode(final NoritoDecoder decoder) {
          return new AliasSetupModels.AliasAssetTotalV1(
              decodeField(decoder, ASSET_ID_ADAPTER), decodeField(decoder, QUANTITY_ADAPTER));
        }
      };

  private static final TypeAdapter<AliasSetupModels.AliasSetupDiagnosticV1>
      DIAGNOSTIC_ADAPTER =
          new TypeAdapter<>() {
            private final TypeAdapter<Optional<String>> optionalString =
                NoritoAdapters.option(STRING);

            @Override
            public void encode(
                final NoritoEncoder encoder,
                final AliasSetupModels.AliasSetupDiagnosticV1 value) {
              encodeField(encoder, U32, (long) value.phase().ordinal());
              encodeField(encoder, STRING, value.code());
              encodeField(encoder, U32, (long) value.severity().ordinal());
              encodeField(encoder, optionalString, Optional.ofNullable(value.resource()));
              encodeField(encoder, optionalString, Optional.ofNullable(value.configPath()));
              encodeField(encoder, optionalString, Optional.ofNullable(value.expected()));
              encodeField(encoder, optionalString, Optional.ofNullable(value.actual()));
              encodeField(encoder, STRING, value.remediation());
            }

            @Override
            public AliasSetupModels.AliasSetupDiagnosticV1 decode(
                final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasSetupDiagnosticV1(
                  enumAt(
                      AliasSetupModels.AliasSetupValidationPhaseV1.values(),
                      decodeField(decoder, U32),
                      "AliasSetupValidationPhaseV1"),
                  decodeField(decoder, STRING),
                  enumAt(
                      AliasSetupModels.AliasSetupSeverityV1.values(),
                      decodeField(decoder, U32),
                      "AliasSetupSeverityV1"),
                  decodeField(decoder, optionalString).orElse(null),
                  decodeField(decoder, optionalString).orElse(null),
                  decodeField(decoder, optionalString).orElse(null),
                  decodeField(decoder, optionalString).orElse(null),
                  decodeField(decoder, STRING));
            }
          };

  private static final TypeAdapter<AliasSetupModels.AliasPlanAnchorV1> ANCHOR_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final AliasSetupModels.AliasPlanAnchorV1 value) {
          encodeField(encoder, U64, value.blockHeight());
          encodeField(encoder, HASH_ADAPTER, value.blockHash());
        }

        @Override
        public AliasSetupModels.AliasPlanAnchorV1 decode(final NoritoDecoder decoder) {
          return new AliasSetupModels.AliasPlanAnchorV1(
              decodeNonNegativeU64Field(decoder, "AliasPlanAnchorV1.block_height"),
              decodeField(decoder, HASH_ADAPTER));
        }
      };

  private static final TypeAdapter<List<AliasSetupModels.AliasPlanResourceV1>> RESOURCE_LIST =
      NoritoAdapters.sequence(PLAN_RESOURCE_ADAPTER);
  private static final TypeAdapter<List<AliasSetupModels.AliasFramedInstructionV1>>
      INSTRUCTION_LIST = NoritoAdapters.sequence(FRAMED_INSTRUCTION_ADAPTER);
  private static final TypeAdapter<List<AliasSetupModels.AliasAssetTotalV1>> TOTAL_LIST =
      NoritoAdapters.sequence(ASSET_TOTAL_ADAPTER);
  private static final TypeAdapter<List<AliasSetupModels.AliasSetupDiagnosticV1>> DIAGNOSTIC_LIST =
      NoritoAdapters.sequence(DIAGNOSTIC_ADAPTER);

  private static final TypeAdapter<AliasSetupModels.AliasTransactionPlanBodyV1>
      PLAN_BODY_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder,
                final AliasSetupModels.AliasTransactionPlanBodyV1 value) {
              encodeField(encoder, U8, (long) value.version());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority());
              encodeField(encoder, CHAIN_ID_ADAPTER, value.chainId());
              encodeField(encoder, ANCHOR_ADAPTER, value.anchor());
              encodeField(encoder, RESOURCE_LIST, value.resources());
              encodeField(encoder, INSTRUCTION_LIST, value.instructions());
              encodeField(encoder, TOTAL_LIST, value.totalsByAsset());
              encodeField(encoder, DIAGNOSTIC_LIST, value.warnings());
              encodeField(encoder, DIAGNOSTIC_LIST, value.blockers());
              encodeField(encoder, U64, value.validUntilMs());
            }

            @Override
            public AliasSetupModels.AliasTransactionPlanBodyV1 decode(
                final NoritoDecoder decoder) {
              return new AliasSetupModels.AliasTransactionPlanBodyV1(
                  Math.toIntExact(decodeField(decoder, U8)),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER),
                  decodeField(decoder, CHAIN_ID_ADAPTER),
                  decodeField(decoder, ANCHOR_ADAPTER),
                  decodeField(decoder, RESOURCE_LIST),
                  decodeField(decoder, INSTRUCTION_LIST),
                  decodeField(decoder, TOTAL_LIST),
                  decodeField(decoder, DIAGNOSTIC_LIST),
                  decodeField(decoder, DIAGNOSTIC_LIST),
                  decodeNonNegativeU64Field(
                      decoder, "AliasTransactionPlanBodyV1.valid_until_ms"));
            }
          };

  private static final TypeAdapter<Optional<AliasSetupModels.AliasFramedInstructionV1>>
      OPTIONAL_INSTRUCTION = NoritoAdapters.option(FRAMED_INSTRUCTION_ADAPTER);
  private static final TypeAdapter<Optional<AliasSetupModels.AliasLeaseQuoteV1>> OPTIONAL_QUOTE =
      NoritoAdapters.option(LEASE_QUOTE_ADAPTER);
  private static final TypeAdapter<List<String>> STRING_LIST = NoritoAdapters.sequence(STRING);

  private static final TypeAdapter<AccountOnboardingPlanRequestV1>
      ONBOARDING_REQUEST_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final AccountOnboardingPlanRequestV1 value) {
              encodeField(encoder, U8, (long) value.version());
              encodeField(encoder, STRING, value.alias());
              encodeField(encoder, STRING, value.accountId());
              encodeField(encoder, STRING_LIST, value.permissions());
            }

            @Override
            public AccountOnboardingPlanRequestV1 decode(final NoritoDecoder decoder) {
              return new AccountOnboardingPlanRequestV1(
                  Math.toIntExact(decodeField(decoder, U8)),
                  decodeField(decoder, STRING),
                  decodeField(decoder, STRING),
                  decodeField(decoder, STRING_LIST));
            }
          };

  private static final TypeAdapter<AccountOnboardingPlanBodyV1>
      ONBOARDING_PLAN_BODY_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final AccountOnboardingPlanBodyV1 value) {
              encodeField(encoder, U8, (long) value.version());
              encodeField(encoder, ONBOARDING_REQUEST_ADAPTER, value.request());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority());
              encodeField(encoder, CHAIN_ID_ADAPTER, value.chainId());
              encodeField(encoder, ANCHOR_ADAPTER, value.anchor());
              encodeField(encoder, PLAN_RESOURCE_ADAPTER, value.resource());
              encodeField(encoder, ACQUISITION_ADAPTER, value.acquisition());
              encodeField(encoder, QUOTE_GUARD_ADAPTER, value.quoteGuard());
              encodeField(encoder, INSTRUCTION_LIST, value.instructions());
              encodeField(
                  encoder,
                  OPTIONAL_INSTRUCTION,
                  Optional.ofNullable(value.ownerAutoRenewInstruction()));
              encodeField(encoder, U64, value.validUntilMs());
            }

            @Override
            public AccountOnboardingPlanBodyV1 decode(final NoritoDecoder decoder) {
              return new AccountOnboardingPlanBodyV1(
                  Math.toIntExact(decodeField(decoder, U8)),
                  decodeField(decoder, ONBOARDING_REQUEST_ADAPTER),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER),
                  decodeField(decoder, CHAIN_ID_ADAPTER),
                  decodeField(decoder, ANCHOR_ADAPTER),
                  decodeField(decoder, PLAN_RESOURCE_ADAPTER),
                  decodeField(decoder, ACQUISITION_ADAPTER),
                  decodeField(decoder, QUOTE_GUARD_ADAPTER),
                  decodeField(decoder, INSTRUCTION_LIST),
                  decodeField(decoder, OPTIONAL_INSTRUCTION).orElse(null),
                  decodeNonNegativeU64Field(
                      decoder, "AccountOnboardingPlanBodyV1.valid_until_ms"));
            }
          };

  private static final TypeAdapter<AliasLifecycleTransactionPlanBodyV1>
      LIFECYCLE_PLAN_BODY_ADAPTER =
          new TypeAdapter<>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final AliasLifecycleTransactionPlanBodyV1 value) {
              encodeField(encoder, U8, (long) value.version());
              encodeField(encoder, ACCOUNT_ID_ADAPTER, value.authority());
              encodeField(encoder, CHAIN_ID_ADAPTER, value.chainId());
              encodeField(encoder, ANCHOR_ADAPTER, value.anchor());
              encodeField(encoder, LIFECYCLE_OPERATION_ADAPTER, value.operation());
              encodeField(encoder, U32, (long) value.disposition().ordinal());
              encodeField(encoder, OPTIONAL_INSTRUCTION, Optional.ofNullable(value.instruction()));
              encodeField(encoder, OPTIONAL_QUOTE, Optional.ofNullable(value.quote()));
              encodeField(encoder, TOTAL_LIST, value.totalsByAsset());
              encodeField(encoder, DIAGNOSTIC_LIST, value.warnings());
              encodeField(encoder, DIAGNOSTIC_LIST, value.blockers());
              encodeField(encoder, U64, value.validUntilMs());
            }

            @Override
            public AliasLifecycleTransactionPlanBodyV1 decode(final NoritoDecoder decoder) {
              return new AliasLifecycleTransactionPlanBodyV1(
                  Math.toIntExact(decodeField(decoder, U8)),
                  decodeField(decoder, ACCOUNT_ID_ADAPTER),
                  decodeField(decoder, CHAIN_ID_ADAPTER),
                  decodeField(decoder, ANCHOR_ADAPTER),
                  decodeField(decoder, LIFECYCLE_OPERATION_ADAPTER),
                  enumAt(
                      AliasLifecyclePlanDispositionV1.values(),
                      decodeField(decoder, U32),
                      "AliasLifecyclePlanDispositionV1"),
                  decodeField(decoder, OPTIONAL_INSTRUCTION).orElse(null),
                  decodeField(decoder, OPTIONAL_QUOTE).orElse(null),
                  decodeField(decoder, TOTAL_LIST),
                  decodeField(decoder, DIAGNOSTIC_LIST),
                  decodeField(decoder, DIAGNOSTIC_LIST),
                  decodeNonNegativeU64Field(
                      decoder, "AliasLifecycleTransactionPlanBodyV1.valid_until_ms"));
            }
          };

  private static <T> void encodeField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length < 0 || length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Field payload too large");
    }
    final NoritoDecoder child =
        new NoritoDecoder(
            decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after field payload");
    }
    return value;
  }

  private static <T> void encodeEnum(
      final NoritoEncoder encoder,
      final int tag,
      final TypeAdapter<T> adapter,
      final T value) {
    U32.encode(encoder, (long) tag);
    encodeField(encoder, adapter, value);
  }

  private static <T> T decodeEnumPayload(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    return decodeField(decoder, adapter);
  }

  private static long decodeNonNegativeU64Field(
      final NoritoDecoder decoder, final String path) {
    final long value = decodeField(decoder, U64);
    if (value < 0) {
      throw new IllegalArgumentException(path + " exceeds the SDK's signed timestamp bound");
    }
    return value;
  }

  private static void encodeFixedBytes(final NoritoEncoder encoder, final byte[] value) {
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    for (final byte item : value) {
      encoder.writeLength(1, compact);
      encoder.writeByte(item);
    }
  }

  private static byte[] decodeFixedBytes(
      final NoritoDecoder decoder, final int count, final String path) {
    final byte[] result = new byte[count];
    for (int index = 0; index < count; index++) {
      if (decoder.readLength(decoder.compactLenActive()) != 1) {
        throw new IllegalArgumentException(
            path + " element " + index + " must contain exactly one byte");
      }
      result[index] = (byte) decoder.readByte();
    }
    return result;
  }

  private static void encodeBigIntegerField(
      final NoritoEncoder encoder, final BigInteger value) {
    final NoritoEncoder child = encoder.childEncoder();
    final byte[] bytes = toCanonicalLittleEndian(value);
    child.writeUInt(bytes.length, 32);
    child.writeBytes(bytes);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(payload);
  }

  private static BigInteger decodeBigIntegerField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length < 4 || length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("BigInteger field length is invalid");
    }
    final NoritoDecoder child =
        new NoritoDecoder(
            decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final long byteLength = child.readUInt(32);
    if (byteLength < 0 || byteLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("BigInteger payload is too large");
    }
    final byte[] bytes = child.readBytes((int) byteLength);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after BigInteger payload");
    }
    final BigInteger value = fromCanonicalLittleEndian(bytes);
    if (!java.util.Arrays.equals(toCanonicalLittleEndian(value), bytes)) {
      throw new IllegalArgumentException("BigInteger payload is not canonical");
    }
    return value;
  }

  private static byte[] toCanonicalLittleEndian(final BigInteger value) {
    if (value.signum() == 0) return new byte[0];
    final byte[] bigEndian = value.toByteArray();
    final byte[] littleEndian = new byte[bigEndian.length];
    for (int index = 0; index < bigEndian.length; index++) {
      littleEndian[index] = bigEndian[bigEndian.length - 1 - index];
    }
    int size = littleEndian.length;
    if (value.signum() > 0) {
      while (size > 1
          && littleEndian[size - 1] == 0
          && (littleEndian[size - 2] & 0x80) == 0) size--;
    } else {
      while (size > 1
          && littleEndian[size - 1] == (byte) 0xff
          && (littleEndian[size - 2] & 0x80) != 0) size--;
    }
    return size == littleEndian.length
        ? littleEndian
        : java.util.Arrays.copyOf(littleEndian, size);
  }

  private static BigInteger fromCanonicalLittleEndian(final byte[] value) {
    if (value.length == 0) return BigInteger.ZERO;
    final byte[] bigEndian = new byte[value.length];
    for (int index = 0; index < value.length; index++) {
      bigEndian[index] = value[value.length - 1 - index];
    }
    return new BigInteger(bigEndian);
  }

  private static <T> T enumAt(final T[] values, final long tag, final String name) {
    if (tag < 0 || tag >= values.length) {
      throw new IllegalArgumentException("Unknown " + name + " discriminant: " + tag);
    }
    return values[(int) tag];
  }

  private static String hex(final byte[] value) {
    final StringBuilder result = new StringBuilder(value.length * 2);
    for (final byte item : value) result.append(String.format("%02x", item & 0xff));
    return result.toString();
  }
}
