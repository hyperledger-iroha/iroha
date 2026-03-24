package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.SortedMap;
import java.util.TreeMap;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Helpers for computing deterministic multisig controller seeds. */
public final class MultisigSeedHelper {

  private static final int ALGO_TAG_ED25519 = 0;
  private static final int ALGO_TAG_ML_DSA = 4;
  private static final int ALGO_TAG_GOST_256_A = 5;
  private static final int ALGO_TAG_GOST_256_B = 6;
  private static final int ALGO_TAG_GOST_256_C = 7;
  private static final int ALGO_TAG_GOST_512_A = 8;
  private static final int ALGO_TAG_GOST_512_B = 9;
  private static final int ALGO_TAG_SM2 = 10;

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> U8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> U16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> U64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> ENUM_TAG_ADAPTER = NoritoAdapters.uint(32);

  private MultisigSeedHelper() {}

  /**
   * Returns true when {@code accountId} matches the deterministic controller derived from
   * {@code spec}.
   *
   * <p>If the seed cannot be computed (unsupported account formats/curves), this returns false.
   */
  public static boolean isDeterministicDerivedControllerId(
      final String accountId, final MultisigSpec spec) {
    Objects.requireNonNull(spec, "spec");
    final Optional<AccountIdParts> target = parseAccountIdParts(accountId);
    if (target.isEmpty()) {
      return false;
    }
    final AccountIdParts targetParts = target.get();
    if (!targetParts.isEd25519()) {
      return false;
    }
    final Optional<byte[]> derived = deriveDeterministicPublicKey(spec);
    if (derived.isEmpty()) {
      return false;
    }
    return Arrays.equals(targetParts.publicKey, derived.get());
  }

  static Optional<byte[]> deriveDeterministicPublicKey(final MultisigSpec spec) {
    final SortedMap<AccountIdParts, Long> signatories = new TreeMap<>();
    for (final Map.Entry<String, Integer> entry : spec.signatories().entrySet()) {
      final Optional<AccountIdParts> parts = parseAccountIdParts(entry.getKey());
      if (parts.isEmpty()) {
        return Optional.empty();
      }
      signatories.put(parts.get(), Long.valueOf(entry.getValue()));
    }

    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.MINOR_VERSION);
    encodeMultisigSpecField(encoder, signatories, spec);

    final byte[] seed = Blake2b.digest256(encoder.toByteArray());
    seed[seed.length - 1] |= 1;
    try {
      final Ed25519PrivateKeyParameters privateKey =
          new Ed25519PrivateKeyParameters(seed, 0);
      final Ed25519PublicKeyParameters publicKey = privateKey.generatePublicKey();
      return Optional.of(publicKey.getEncoded());
    } catch (final Exception ex) {
      return Optional.empty();
    }
  }

  private static void encodeMultisigSpecField(
      final NoritoEncoder encoder,
      final SortedMap<AccountIdParts, Long> signatories,
      final MultisigSpec spec) {
    final NoritoEncoder child = encoder.childEncoder();
    encodeMultisigSpec(child, signatories, spec);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static void encodeMultisigSpec(
      final NoritoEncoder encoder,
      final SortedMap<AccountIdParts, Long> signatories,
      final MultisigSpec spec) {
    final TypeAdapter<Map<AccountIdParts, Long>> signatoryAdapter =
        NoritoAdapters.map(ACCOUNT_ID_ADAPTER, U8_ADAPTER);
    encodeSizedField(encoder, signatoryAdapter, signatories);
    encodeSizedField(encoder, U16_ADAPTER, (long) spec.quorum());
    encodeSizedField(encoder, U64_ADAPTER, spec.transactionTtlMs());
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static Optional<AccountIdParts> parseAccountIdParts(final String accountId) {
    if (accountId == null) {
      return Optional.empty();
    }
    final String trimmed = accountId.trim();
    if (trimmed.isEmpty()) {
      return Optional.empty();
    }
    final AccountAddress address;
    try {
      address = AccountAddress.parseEncodedIgnoringCurveSupport(trimmed, null).address;
    } catch (final AccountAddress.AccountAddressException ignored) {
      return Optional.empty();
    }
    try {
      final Optional<AccountAddress.SingleKeyPayload> payloadOpt =
          address.singleKeyPayloadIgnoringCurveSupport();
      if (payloadOpt.isEmpty()) {
        return Optional.empty();
      }
      final AccountAddress.SingleKeyPayload payload = payloadOpt.get();
      final int algorithmTag = algorithmTagForCurveId(payload.curveId());
      if (algorithmTag < 0) {
        return Optional.empty();
      }
      return Optional.of(new AccountIdParts(payload.curveId(), algorithmTag, payload.publicKey()));
    } catch (final AccountAddress.AccountAddressException ignored) {
      return Optional.empty();
    }
  }

  private static int algorithmTagForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return ALGO_TAG_ED25519;
      case 0x02:
        return ALGO_TAG_ML_DSA;
      case 0x0A:
        return ALGO_TAG_GOST_256_A;
      case 0x0B:
        return ALGO_TAG_GOST_256_B;
      case 0x0C:
        return ALGO_TAG_GOST_256_C;
      case 0x0D:
        return ALGO_TAG_GOST_512_A;
      case 0x0E:
        return ALGO_TAG_GOST_512_B;
      case 0x0F:
        return ALGO_TAG_SM2;
      default:
        return -1;
    }
  }

  private static final class AccountIdParts implements Comparable<AccountIdParts> {
    private final int curveId;
    private final int algorithmTag;
    private final byte[] publicKey;
    private final String publicKeyLiteral;

    private AccountIdParts(final int curveId, final int algorithmTag, final byte[] publicKey) {
      this.curveId = curveId;
      this.algorithmTag = algorithmTag;
      this.publicKey = Arrays.copyOf(publicKey, publicKey.length);
      if (curveId < 0) {
        throw new IllegalArgumentException("curveId must be non-negative");
      }
      this.publicKeyLiteral = PublicKeyCodec.encodePublicKeyMultihash(curveId, publicKey);
    }

    private boolean isEd25519() {
      return algorithmTag == ALGO_TAG_ED25519;
    }

    @Override
    public int compareTo(final AccountIdParts other) {
      final int algoCmp = Integer.compare(algorithmTag, other.algorithmTag);
      if (algoCmp != 0) {
        return algoCmp;
      }
      final int min = Math.min(publicKey.length, other.publicKey.length);
      for (int i = 0; i < min; i++) {
        final int left = Byte.toUnsignedInt(publicKey[i]);
        final int right = Byte.toUnsignedInt(other.publicKey[i]);
        if (left != right) {
          return Integer.compare(left, right);
        }
      }
      return Integer.compare(publicKey.length, other.publicKey.length);
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof AccountIdParts other)) {
        return false;
      }
      return algorithmTag == other.algorithmTag
          && Arrays.equals(publicKey, other.publicKey);
    }

    @Override
    public int hashCode() {
      int result = algorithmTag;
      result = 31 * result + Arrays.hashCode(publicKey);
      return result;
    }
  }

  private static final TypeAdapter<AccountIdParts> ACCOUNT_CONTROLLER_ADAPTER =
      new TypeAdapter<AccountIdParts>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AccountIdParts value) {
          ENUM_TAG_ADAPTER.encode(encoder, 0L);
          encodeSizedField(encoder, STRING_ADAPTER, value.publicKeyLiteral);
        }

        @Override
        public AccountIdParts decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
          throw new UnsupportedOperationException("AccountController decode is not supported");
        }
      };

  private static final TypeAdapter<AccountIdParts> ACCOUNT_ID_ADAPTER =
      new TypeAdapter<AccountIdParts>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AccountIdParts value) {
          encodeSizedField(encoder, ACCOUNT_CONTROLLER_ADAPTER, value);
        }

        @Override
        public AccountIdParts decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
          throw new UnsupportedOperationException("AccountIdParts decode is not supported");
        }
      };
}
