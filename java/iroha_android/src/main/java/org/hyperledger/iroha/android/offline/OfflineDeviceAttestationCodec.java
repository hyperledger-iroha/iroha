// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;
import java.util.function.Supplier;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Package-private canonical Norito codec for the current registration instruction. */
final class OfflineDeviceAttestationCodec {
  private static final ThreadLocal<Integer> DECODE_CHAIN_DISCRIMINANT = new ThreadLocal<>();

  static final String REGISTRATION_SCHEMA =
      "iroha_data_model::offline::OfflineDeviceAttestationRegistration";
  static final String CHALLENGE_SCHEMA =
      "iroha_data_model::offline::OfflineDeviceAttestationChallengePreimage";
  static final String ANDROID_CHALLENGE_SCHEMA =
      "iroha_data_model::offline::OfflineAndroidKeyMintChallengePreimage";
  static final String INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation";

  private static final RegistrationAdapter REGISTRATION_ADAPTER = new RegistrationAdapter();
  private static final InstructionAdapter INSTRUCTION_ADAPTER = new InstructionAdapter();
  private static final ChallengeAdapter CHALLENGE_ADAPTER = new ChallengeAdapter();
  private static final AndroidChallengeAdapter ANDROID_CHALLENGE_ADAPTER =
      new AndroidChallengeAdapter();

  private OfflineDeviceAttestationCodec() {}

  static byte[] encodeRegistration(final DeviceAttestationRegistration registration) {
    return NoritoCodec.encode(
        Objects.requireNonNull(registration, "registration"),
        REGISTRATION_SCHEMA,
        REGISTRATION_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  static DeviceAttestationRegistration decodeRegistrationCanonical(
      final byte[] archive, final int chainDiscriminant) {
    return withDecodeChain(
        chainDiscriminant,
        () -> {
          final byte[] snapshot = Objects.requireNonNull(archive, "archive").clone();
          final NoritoCodec.ArchiveView view =
              NoritoCodec.fromBytesView(snapshot, REGISTRATION_SCHEMA);
          if (view.flags() != NoritoHeader.COMPACT_LEN) {
            throw new IllegalArgumentException(
                "registration archive must use canonical compact lengths");
          }
          final DeviceAttestationRegistration decoded = view.decode(REGISTRATION_ADAPTER);
          if (!Arrays.equals(snapshot, encodeRegistration(decoded))) {
            throw new IllegalArgumentException(
                "registration archive is not canonically encoded");
          }
          return decoded;
        });
  }

  static byte[] encodeInstructionPayload(final DeviceAttestationRegistration registration) {
    return NoritoCodec.encode(
        Objects.requireNonNull(registration, "registration"),
        INSTRUCTION_SCHEMA,
        INSTRUCTION_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  static DeviceAttestationRegistration decodeInstructionPayloadCanonical(
      final byte[] archive, final int chainDiscriminant) {
    return withDecodeChain(
        chainDiscriminant,
        () -> {
          final byte[] snapshot = Objects.requireNonNull(archive, "archive").clone();
          final NoritoCodec.ArchiveView view =
              NoritoCodec.fromBytesView(snapshot, INSTRUCTION_SCHEMA);
          if (view.flags() != NoritoHeader.COMPACT_LEN) {
            throw new IllegalArgumentException(
                "instruction archive must use canonical compact lengths");
          }
          final DeviceAttestationRegistration decoded = view.decode(INSTRUCTION_ADAPTER);
          if (!Arrays.equals(snapshot, encodeInstructionPayload(decoded))) {
            throw new IllegalArgumentException(
                "instruction archive is not canonically encoded");
          }
          return decoded;
        });
  }

  static InstructionBox instruction(final DeviceAttestationRegistration registration) {
    return InstructionBox.fromWirePayload(
        INSTRUCTION_SCHEMA, encodeInstructionPayload(registration));
  }

  static byte[] canonicalChallengeHash(final DeviceAttestationRegistration value) {
    if (DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM.equals(value.platform())) {
      return androidPreKeyGenerationChallengeHash(
          value.version(),
          value.deviceId(),
          value.accountId(),
          value.assetDefinitionId(),
          value.androidPackageName(),
          value.androidSigningCertificateSha256(),
          value.publicKey().sec1Bytes(),
          value.recentBlockHeight(),
          value.recentBlockHash(),
          value.expiresAtMs());
    }
    final Challenge valueToHash = new Challenge(value);
    return IrohaHash.prehash(
        NoritoCodec.encode(
            valueToHash,
            CHALLENGE_SCHEMA,
            CHALLENGE_ADAPTER,
            NoritoHeader.COMPACT_LEN));
  }

  static byte[] androidPreKeyGenerationChallengeHash(
      final int version,
      final String deviceId,
      final String accountId,
      final String assetDefinitionId,
      final String androidPackageName,
      final byte[] androidSigningCertificateSha256,
      final byte[] publicKey,
      final long recentBlockHeight,
      final byte[] recentBlockHash,
      final long expiresAtMs) {
    if (version != DeviceAttestationRegistration.REGISTRATION_VERSION) {
      throw new IllegalArgumentException("registration version must be exactly 1");
    }
    final AndroidChallenge challenge =
        new AndroidChallenge(
            deviceId,
            accountId,
            assetDefinitionId,
            androidPackageName,
            androidSigningCertificateSha256,
            KagemushaP256Codec.requireUncompressedPublicKey(publicKey),
            recentBlockHeight,
            recentBlockHash,
            expiresAtMs);
    return IrohaHash.prehash(
        NoritoCodec.encode(
            challenge,
            ANDROID_CHALLENGE_SCHEMA,
            ANDROID_CHALLENGE_ADAPTER,
            NoritoHeader.COMPACT_LEN));
  }

  static void validateAccountId(final String accountId) {
    TransferWirePayloadEncoder.encodeAccountIdPayload(accountId);
  }

  private static final class RegistrationAdapter
      implements TypeAdapter<DeviceAttestationRegistration> {
    @Override
    public void encode(
        final NoritoEncoder encoder, final DeviceAttestationRegistration value) {
      field(encoder, child -> child.writeUInt(value.version(), 16));
      field(encoder, child -> string(child, value.platform()));
      field(encoder, child -> string(child, value.keyId()));
      field(encoder, child -> string(child, value.deviceId()));
      field(encoder, child -> child.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value.accountId())));
      field(encoder, child -> optionAssetDefinitionId(child, value.assetDefinitionId()));
      field(encoder, child -> optionString(child, value.iosTeamId()));
      field(encoder, child -> optionString(child, value.iosBundleId()));
      field(encoder, child -> optionString(child, value.iosEnvironment()));
      field(encoder, child -> optionString(child, value.androidPackageName()));
      field(encoder, child -> optionBytes(child, value.androidSigningCertificateSha256()));
      field(
          encoder,
          child ->
              optionAndroidAttestedDeviceProperties(
                  child, value.androidAttestedDeviceProperties()));
      field(encoder, child -> p256PublicKey(child, value.publicKey().sec1Bytes()));
      field(encoder, child -> string(child, value.assertionScheme()));
      field(encoder, child -> string(child, value.assertionKeyAlgorithm()));
      field(encoder, child -> bytes(child, value.assertionPublicKey()));
      field(encoder, child -> optionU32(child, value.assertionUsageCountLimit()));
      field(encoder, child -> child.writeByte(value.oneUse() ? 1 : 0));
      field(encoder, child -> child.writeBytes(value.challengeHash()));
      field(encoder, child -> child.writeBytes(value.attestationReportHash()));
      field(encoder, child -> bytes(child, value.attestationReport()));
      field(encoder, child -> child.writeBytes(value.evidenceHash()));
      field(encoder, child -> bytes(child, value.evidence()));
      field(encoder, child -> child.writeUInt(value.recentBlockHeight(), 64));
      field(encoder, child -> child.writeBytes(value.recentBlockHash()));
      field(encoder, child -> child.writeUInt(value.expiresAtMs(), 64));
    }

    @Override
    public DeviceAttestationRegistration decode(final NoritoDecoder decoder) {
      return new DeviceAttestationRegistration(
          readField(decoder, child -> checkedU16(child.readUInt(16))),
          readField(decoder, OfflineDeviceAttestationCodec::readString),
          readField(decoder, OfflineDeviceAttestationCodec::readString),
          readField(decoder, OfflineDeviceAttestationCodec::readString),
          readField(decoder, child -> TransferWirePayloadEncoder.decodeAccountIdPayload(
              child.readBytes(child.remaining()),
              requiredDecodeChainDiscriminant(),
              child.flags(),
              child.flagsHint())),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionAssetDefinitionId),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionString),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionString),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionString),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionString),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionBytes),
          readField(
              decoder,
              OfflineDeviceAttestationCodec::readOptionAndroidAttestedDeviceProperties),
          new KagemushaDevicePublicKeyV2(
              readField(decoder, OfflineDeviceAttestationCodec::readP256PublicKey)),
          readField(decoder, OfflineDeviceAttestationCodec::readString),
          readField(decoder, OfflineDeviceAttestationCodec::readString),
          readField(decoder, OfflineDeviceAttestationCodec::readBytes),
          readField(decoder, OfflineDeviceAttestationCodec::readOptionU32),
          readField(decoder, OfflineDeviceAttestationCodec::readBool),
          readField(decoder, child -> readHash(child, "challenge_hash")),
          readField(decoder, child -> readHash(child, "attestation_report_hash")),
          readField(decoder, OfflineDeviceAttestationCodec::readBytes),
          readField(decoder, child -> readHash(child, "evidence_hash")),
          readField(decoder, OfflineDeviceAttestationCodec::readBytes),
          readField(decoder, child -> child.readUInt(64)),
          readField(decoder, child -> readHash(child, "recent_block_hash")),
          readField(decoder, child -> child.readUInt(64)));
    }
  }

  private static int requiredDecodeChainDiscriminant() {
    final Integer value = DECODE_CHAIN_DISCRIMINANT.get();
    if (value == null) {
      throw new IllegalStateException(
          "offline attestation decoding requires an explicit chainDiscriminant");
    }
    return value;
  }

  private static <T> T withDecodeChain(
      final int chainDiscriminant, final Supplier<T> operation) {
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    final Integer previous = DECODE_CHAIN_DISCRIMINANT.get();
    if (previous != null && previous.intValue() != chainDiscriminant) {
      throw new IllegalStateException("Conflicting nested chainDiscriminant context");
    }
    DECODE_CHAIN_DISCRIMINANT.set(chainDiscriminant);
    try {
      return operation.get();
    } finally {
      if (previous == null) {
        DECODE_CHAIN_DISCRIMINANT.remove();
      } else {
        DECODE_CHAIN_DISCRIMINANT.set(previous);
      }
    }
  }

  private static final class InstructionAdapter
      implements TypeAdapter<DeviceAttestationRegistration> {
    @Override
    public void encode(
        final NoritoEncoder encoder, final DeviceAttestationRegistration registration) {
      field(encoder, child -> REGISTRATION_ADAPTER.encode(child, registration));
    }

    @Override
    public DeviceAttestationRegistration decode(final NoritoDecoder decoder) {
      return readField(decoder, REGISTRATION_ADAPTER::decode);
    }
  }

  private static final class ChallengeAdapter implements TypeAdapter<Challenge> {
    @Override
    public void encode(final NoritoEncoder encoder, final Challenge value) {
      field(encoder, child -> string(child, DeviceAttestationRegistration.DEVICE_ATTESTATION_CHALLENGE_DOMAIN));
      field(encoder, child -> child.writeUInt(value.registration.version(), 16));
      field(encoder, child -> string(child, value.registration.platform()));
      field(encoder, child -> string(child, value.registration.keyId()));
      field(encoder, child -> string(child, value.registration.deviceId()));
      field(encoder, child -> child.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value.registration.accountId())));
      field(encoder, child -> optionAssetDefinitionId(child, value.registration.assetDefinitionId()));
      field(encoder, child -> optionString(child, value.registration.iosTeamId()));
      field(encoder, child -> optionString(child, value.registration.iosBundleId()));
      field(encoder, child -> optionString(child, value.registration.iosEnvironment()));
      field(encoder, child -> optionString(child, value.registration.androidPackageName()));
      field(encoder, child -> optionBytes(child, value.registration.androidSigningCertificateSha256()));
      field(
          encoder,
          child -> p256PublicKey(child, value.registration.publicKey().sec1Bytes()));
      field(encoder, child -> string(child, value.registration.assertionScheme()));
      field(encoder, child -> string(child, value.registration.assertionKeyAlgorithm()));
      field(encoder, child -> optionU32(child, value.registration.assertionUsageCountLimit()));
      field(encoder, child -> child.writeByte(value.registration.oneUse() ? 1 : 0));
      field(encoder, child -> child.writeUInt(value.registration.recentBlockHeight(), 64));
      field(encoder, child -> child.writeBytes(value.registration.recentBlockHash()));
      field(encoder, child -> child.writeUInt(value.registration.expiresAtMs(), 64));
    }

    @Override
    public Challenge decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("challenge preimages are encode-only");
    }
  }

  private static final class AndroidChallengeAdapter implements TypeAdapter<AndroidChallenge> {
    @Override
    public void encode(final NoritoEncoder encoder, final AndroidChallenge value) {
      field(encoder, child -> string(child, DeviceAttestationRegistration.DEVICE_ATTESTATION_CHALLENGE_DOMAIN));
      field(encoder, child -> child.writeUInt(DeviceAttestationRegistration.REGISTRATION_VERSION, 16));
      field(encoder, child -> string(child, DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM));
      field(encoder, child -> string(child, value.deviceId));
      field(encoder, child -> child.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value.accountId)));
      field(encoder, child -> optionAssetDefinitionId(child, value.assetDefinitionId));
      field(encoder, child -> optionString(child, null));
      field(encoder, child -> optionString(child, null));
      field(encoder, child -> optionString(child, null));
      field(encoder, child -> optionString(child, value.androidPackageName));
      field(encoder, child -> optionBytes(child, value.androidSigningCertificateSha256));
      field(encoder, child -> p256PublicKey(child, value.publicKey));
      field(encoder, child -> string(child, DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME));
      field(encoder, child -> string(child, DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM));
      field(encoder, child -> optionU32(child, 1));
      field(encoder, child -> child.writeByte(1));
      field(encoder, child -> child.writeUInt(value.recentBlockHeight, 64));
      field(encoder, child -> child.writeBytes(value.recentBlockHash));
      field(encoder, child -> child.writeUInt(value.expiresAtMs, 64));
    }

    @Override
    public AndroidChallenge decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("challenge preimages are encode-only");
    }
  }

  private static final class Challenge {
    private final DeviceAttestationRegistration registration;

    private Challenge(final DeviceAttestationRegistration registration) {
      this.registration = registration;
    }
  }

  private static final class AndroidChallenge {
    private final String deviceId;
    private final String accountId;
    private final String assetDefinitionId;
    private final String androidPackageName;
    private final byte[] androidSigningCertificateSha256;
    private final byte[] publicKey;
    private final long recentBlockHeight;
    private final byte[] recentBlockHash;
    private final long expiresAtMs;

    private AndroidChallenge(
        final String deviceId,
        final String accountId,
        final String assetDefinitionId,
        final String androidPackageName,
        final byte[] androidSigningCertificateSha256,
        final byte[] publicKey,
        final long recentBlockHeight,
        final byte[] recentBlockHash,
        final long expiresAtMs) {
      this.deviceId = exact(deviceId, "device_id");
      this.accountId = exact(accountId, "account_id");
      this.assetDefinitionId = assetDefinitionId;
      this.androidPackageName = exact(androidPackageName, "android_package_name");
      this.androidSigningCertificateSha256 =
          Objects.requireNonNull(androidSigningCertificateSha256, "android_signing_certificate_sha256").clone();
      this.publicKey = Objects.requireNonNull(publicKey, "public_key").clone();
      this.recentBlockHeight = recentBlockHeight;
      this.recentBlockHash = Objects.requireNonNull(recentBlockHash, "recent_block_hash").clone();
      this.expiresAtMs = expiresAtMs;
      validateAccountId(this.accountId);
      if (assetDefinitionId != null) {
        AssetDefinitionIdEncoder.parseAddressBytes(assetDefinitionId);
      }
      if (this.androidSigningCertificateSha256.length != 32) {
        throw new IllegalArgumentException("android_signing_certificate_sha256 must be 32 bytes");
      }
      KagemushaP256Codec.requireUncompressedPublicKey(this.publicKey);
      if (recentBlockHeight <= 0 || expiresAtMs <= 0) {
        throw new IllegalArgumentException("challenge lifetime fields must be positive");
      }
      DeviceAttestationRegistration.requireHash(this.recentBlockHash, "recent_block_hash");
    }
  }

  @FunctionalInterface
  private interface Writer {
    void write(NoritoEncoder encoder);
  }

  @FunctionalInterface
  private interface Reader<T> {
    T read(NoritoDecoder decoder);
  }

  private static void field(final NoritoEncoder parent, final Writer writer) {
    final NoritoEncoder child = parent.childEncoder();
    writer.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, compact(parent));
    parent.writeBytes(payload);
  }

  private static <T> T readField(final NoritoDecoder parent, final Reader<T> reader) {
    final int length = checkedLength(parent.readLength(compact(parent)), "field");
    final NoritoDecoder child =
        new NoritoDecoder(parent.readBytes(length), parent.flags(), parent.flagsHint());
    final T value = reader.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("field has trailing bytes");
    }
    return value;
  }

  private static void string(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, compact(encoder));
    encoder.writeBytes(bytes);
  }

  private static String readString(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readLength(compact(decoder)), "string");
    final byte[] bytes = decoder.readBytes(length);
    final String value = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(bytes, value.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalArgumentException("string is not canonical UTF-8");
    }
    return value;
  }

  private static void bytes(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static byte[] readBytes(final NoritoDecoder decoder) {
    return decoder.readBytes(checkedLength(decoder.readUInt(64), "byte vector"));
  }

  private static void p256PublicKey(final NoritoEncoder encoder, final byte[] value) {
    final byte[] key = KagemushaP256Codec.requireUncompressedPublicKey(value);
    encoder.writeBytes(key);
  }

  private static byte[] readP256PublicKey(final NoritoDecoder decoder) {
    if (decoder.remaining() != KagemushaP256Codec.PUBLIC_KEY_BYTES) {
      throw new IllegalArgumentException("P-256 public key must contain exactly 65 bytes");
    }
    return KagemushaP256Codec.requireUncompressedPublicKey(
        decoder.readBytes(KagemushaP256Codec.PUBLIC_KEY_BYTES));
  }

  private static void optionString(final NoritoEncoder encoder, final String value) {
    option(encoder, value, child -> string(child, value));
  }

  private static String readOptionString(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("invalid option tag");
    }
    return readField(decoder, OfflineDeviceAttestationCodec::readString);
  }

  private static void optionBytes(final NoritoEncoder encoder, final byte[] value) {
    option(encoder, value, child -> bytes(child, value));
  }

  private static byte[] readOptionBytes(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("invalid option tag");
    }
    return readField(decoder, OfflineDeviceAttestationCodec::readBytes);
  }

  private static void optionAndroidAttestedDeviceProperties(
      final NoritoEncoder encoder,
      final OfflineAndroidAttestedDevicePropertiesV2 value) {
    option(encoder, value, child -> encodeAndroidAttestedDeviceProperties(child, value));
  }

  private static OfflineAndroidAttestedDevicePropertiesV2
      readOptionAndroidAttestedDeviceProperties(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("invalid option tag");
    }
    return readField(decoder, OfflineDeviceAttestationCodec::readAndroidAttestedDeviceProperties);
  }

  private static void encodeAndroidAttestedDeviceProperties(
      final NoritoEncoder encoder,
      final OfflineAndroidAttestedDevicePropertiesV2 value) {
    field(encoder, child -> child.writeUInt(value.version(), 16));
    field(encoder, child -> child.writeUInt(value.attestationVersion(), 32));
    field(encoder, child -> child.writeUInt(value.keymintVersion(), 32));
    field(encoder, child -> child.writeUInt(value.securityLevel().noritoDiscriminant(), 32));
    field(encoder, child -> string(child, value.brand()));
    field(encoder, child -> string(child, value.device()));
    field(encoder, child -> string(child, value.product()));
    field(encoder, child -> string(child, value.manufacturer()));
    field(encoder, child -> string(child, value.model()));
    field(encoder, child -> child.writeUInt(value.osVersion(), 32));
    field(encoder, child -> child.writeUInt(value.osPatchLevel(), 32));
    field(encoder, child -> child.writeUInt(value.vendorPatchLevel(), 32));
    field(encoder, child -> child.writeUInt(value.bootPatchLevel(), 32));
    field(encoder, child -> bytes(child, value.verifiedBootKey()));
    field(encoder, child -> child.writeBytes(value.verifiedBootHash()));
  }

  private static OfflineAndroidAttestedDevicePropertiesV2
      readAndroidAttestedDeviceProperties(final NoritoDecoder decoder) {
    return new OfflineAndroidAttestedDevicePropertiesV2(
        readField(decoder, child -> checkedU16(child.readUInt(16))),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(
            decoder,
            child -> {
              final long tag = child.readUInt(32);
              if (tag == 0) return OfflineAndroidDeviceSecurityLevelV2.TRUSTED_ENVIRONMENT;
              if (tag == 1) return OfflineAndroidDeviceSecurityLevelV2.STRONG_BOX;
              throw new IllegalArgumentException("invalid Android device security level");
            }),
        readField(decoder, OfflineDeviceAttestationCodec::readString),
        readField(decoder, OfflineDeviceAttestationCodec::readString),
        readField(decoder, OfflineDeviceAttestationCodec::readString),
        readField(decoder, OfflineDeviceAttestationCodec::readString),
        readField(decoder, OfflineDeviceAttestationCodec::readString),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(decoder, child -> checkedUnsignedU32(child.readUInt(32))),
        readField(decoder, OfflineDeviceAttestationCodec::readBytes),
        readField(decoder, child -> child.readBytes(32)));
  }

  private static void optionU32(final NoritoEncoder encoder, final Integer value) {
    option(encoder, value, child -> child.writeUInt(value, 32));
  }

  private static Integer readOptionU32(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("invalid option tag");
    }
    return readField(decoder, child -> checkedU32(child.readUInt(32)));
  }

  private static long checkedUnsignedU32(final long value) {
    if (value < 0 || value > OfflineAndroidAttestedDevicePropertiesV2.U32_MAX) {
      throw new IllegalArgumentException("u32 exceeds supported range");
    }
    return value;
  }

  private static void optionAssetDefinitionId(
      final NoritoEncoder encoder, final String value) {
    option(
        encoder,
        value,
        child -> {
          for (final byte item : AssetDefinitionIdEncoder.parseAddressBytes(value)) {
            child.writeLength(1, compact(child));
            child.writeByte(item);
          }
        });
  }

  private static String readOptionAssetDefinitionId(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("invalid option tag");
    }
    return readField(
        decoder,
        child -> {
          final ByteArrayOutputStream out = new ByteArrayOutputStream();
          while (child.remaining() > 0) {
            if (child.readLength(compact(child)) != 1) {
              throw new IllegalArgumentException("asset definition byte length must be one");
            }
            out.write(child.readByte());
          }
          return AssetDefinitionIdEncoder.encodeFromBytes(out.toByteArray());
        });
  }

  private static void option(
      final NoritoEncoder encoder, final Object value, final Writer presentWriter) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    field(encoder, presentWriter);
  }

  private static boolean readBool(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return false;
    }
    if (tag == 1) {
      return true;
    }
    throw new IllegalArgumentException("invalid boolean tag");
  }

  private static byte[] readHash(final NoritoDecoder decoder, final String field) {
    final byte[] value = decoder.readBytes(32);
    DeviceAttestationRegistration.requireHash(value, field);
    return value;
  }

  private static int checkedU16(final long value) {
    if (value < 0 || value > 0xffffL) {
      throw new IllegalArgumentException("u16 exceeds JVM range");
    }
    return (int) value;
  }

  private static int checkedU32(final long value) {
    if (value < 0 || value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("u32 exceeds supported JVM range");
    }
    return (int) value;
  }

  private static int checkedLength(final long value, final String field) {
    if (value < 0 || value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " length exceeds JVM range");
    }
    return (int) value;
  }

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean compact(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static String exact(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
