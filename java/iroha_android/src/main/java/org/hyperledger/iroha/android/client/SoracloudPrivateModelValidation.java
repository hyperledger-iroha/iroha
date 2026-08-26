package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.bouncycastle.crypto.agreement.X25519Agreement;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Shared invariants for the Soracloud private uploaded-model client surface. */
final class SoracloudPrivateModelValidation {
  static final long U32_MAX = 4_294_967_295L;
  static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  static final long ENCRYPTED_ARTIFACT_MAX_BYTES = 72L * 1024L * 1024L;
  static final int NAME_MAX_UTF8_BYTES = 255;
  static final int IDENTIFIER_MAX_BYTES = 128;
  static final int SERVICE_VERSION_MAX_UTF8_BYTES = 256;
  static final int PRIVATE_RECEIPT_CURSOR_LENGTH_V1 = 114;
  static final String RUNTIME_VERSION_V1 = "soracloud.quantized-cpu.v1";
  static final String X25519_HKDF_SHA256 = "X25519HkdfSha256";
  static final String AES_256_GCM = "Aes256Gcm";
  private static final int MAX_JSON_DEPTH = 128;

  private static final String ZERO_PREHASH_SENTINEL =
      "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
  private static final byte[] LOW_ORDER_X25519_PROBE_PRIVATE_KEY = filledBytes((byte) 1, 32);

  private SoracloudPrivateModelValidation() {}

  static long requireSchemaVersion(final long value, final String field) {
    if (value != 1L) {
      throw new IllegalArgumentException(field + " must equal 1");
    }
    return value;
  }

  static long requireU32(final long value, final String field) {
    if (value < 0L || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be within 0..=" + U32_MAX);
    }
    return value;
  }

  static long requirePositiveU32(final long value, final String field) {
    if (value < 1L || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be within 1..=" + U32_MAX);
    }
    return value;
  }

  static String requireCanonicalString(final String value, final String field) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    if (isWhitespace(value.charAt(0)) || isWhitespace(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException(
          field + " must not contain leading or trailing whitespace");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (Character.isHighSurrogate(character)) {
        if (index + 1 >= value.length() || !Character.isLowSurrogate(value.charAt(index + 1))) {
          throw new IllegalArgumentException(field + " must contain valid Unicode scalar values");
        }
        index++;
      } else if (Character.isLowSurrogate(character)) {
        throw new IllegalArgumentException(field + " must contain valid Unicode scalar values");
      } else if (Character.isISOControl(character)) {
        throw new IllegalArgumentException(field + " must be free of control characters");
      }
    }
    return value;
  }

  static String requireCanonicalName(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.getBytes(StandardCharsets.UTF_8).length > NAME_MAX_UTF8_BYTES) {
      throw new IllegalArgumentException(
          field + " must contain at most " + NAME_MAX_UTF8_BYTES + " UTF-8 bytes");
    }
    if (!Normalizer.isNormalized(canonical, Normalizer.Form.NFC)) {
      throw new IllegalArgumentException(field + " must use its exact NFC-normalized spelling");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (isWhitespace(character)
          || isBidiControl(character)
          || character == '@'
          || character == '#'
          || character == '$') {
        throw new IllegalArgumentException(field + " must be a canonical Iroha Name");
      }
    }
    return canonical;
  }

  static String requireIdentifier(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.length() > IDENTIFIER_MAX_BYTES) {
      throw new IllegalArgumentException(
          field + " must contain at most " + IDENTIFIER_MAX_BYTES + " ASCII bytes");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (!(character >= 'A' && character <= 'Z')
          && !(character >= 'a' && character <= 'z')
          && !(character >= '0' && character <= '9')
          && character != '-'
          && character != '_'
          && character != '.'
          && character != ':'
          && character != '#') {
        throw new IllegalArgumentException(
            field + " must use only ASCII letters, digits, or [-_.:#]");
      }
    }
    return canonical;
  }

  static String requireServiceVersion(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.getBytes(StandardCharsets.UTF_8).length > SERVICE_VERSION_MAX_UTF8_BYTES) {
      throw new IllegalArgumentException(
          field
              + " must contain at most "
              + SERVICE_VERSION_MAX_UTF8_BYTES
              + " UTF-8 bytes");
    }
    return canonical;
  }

  static String requireArtifactRole(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (!"input".equals(canonical) && !"output".equals(canonical)) {
      throw new IllegalArgumentException(field + " must equal input or output");
    }
    return canonical;
  }

  static String requireNetworkId(final String value, final String field) {
    final String canonical = NetworkId.parse(requireCanonicalString(value, field)).literal();
    if (!canonical.equals(value)) {
      throw new IllegalArgumentException(
          field + " must be an exact canonical checksummed 32-byte NetworkId literal");
    }
    return canonical;
  }

  static String requireHash(final String value, final String field) {
    DaJson.requireHash(value, field);
    return value;
  }

  static String requireSoracloudHash(final String value, final String field) {
    requireHash(value, field);
    if (ZERO_PREHASH_SENTINEL.equals(value)) {
      throw new IllegalArgumentException(field + " must not be the zero prehash sentinel");
    }
    return value;
  }

  static byte[] decodeCanonicalX25519PublicKey(final String encoded, final String field) {
    requireCanonicalString(encoded, field);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(field + " must be canonical base64", error);
    }
    if (decoded.length != 32 || !Base64.getEncoder().encodeToString(decoded).equals(encoded)) {
      throw new IllegalArgumentException(
          field + " must be canonical base64 encoding exactly 32 bytes");
    }
    if (isLowOrderX25519PublicKey(decoded)) {
      throw new IllegalArgumentException(field + " must not be a low-order X25519 public key");
    }
    return decoded;
  }

  static String requireRecipientFingerprint(
      final String fingerprint, final byte[] publicKeyBytes, final String field) {
    final String canonical = requireSoracloudHash(fingerprint, field);
    final String expected = HashLiteral.canonicalize(IrohaHash.prehash(publicKeyBytes));
    if (!expected.equals(canonical)) {
      throw new IllegalArgumentException(
          field + " must equal IrohaHash.prehash(publicKeyBytes)");
    }
    return canonical;
  }

  static void requireValidatorIdentity(
      final String validatorAccountId, final String peerId) {
    final String canonicalAccount =
        AccountIdLiteral.requireCanonicalI105Address(
            requireCanonicalString(validatorAccountId, "validatorAccountId"),
            "validatorAccountId");
    final Optional<AccountAddress.SingleKeyPayload> signatory;
    try {
      signatory = AccountAddress.fromI105(canonicalAccount, null).singleKeyPayload();
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          "validatorAccountId must use a canonical universal domainless AccountId", error);
    }
    if (!signatory.isPresent()) {
      throw new IllegalArgumentException(
          "validatorAccountId must have exactly one signatory");
    }

    final String canonicalPeerId = requireCanonicalString(peerId, "peerId");
    final PublicKeyCodec.PublicKeyPayload peer;
    try {
      peer = PublicKeyCodec.decodePublicKeyLiteral(canonicalPeerId);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException("peerId must be a canonical PeerId", error);
    }
    if (peer == null
        || !PublicKeyCodec.encodePublicKeyMultihash(peer.curveId(), peer.keyBytes())
            .equals(canonicalPeerId)) {
      throw new IllegalArgumentException(
          "peerId must use the exact canonical peer public-key spelling");
    }
    final AccountAddress.SingleKeyPayload accountSignatory = signatory.get();
    if (peer.curveId() != accountSignatory.curveId()
        || !Arrays.equals(peer.keyBytes(), accountSignatory.publicKey())) {
      throw new IllegalArgumentException(
          "peerId must equal validatorAccountId's exact single signatory");
    }
  }

  static void requireLedgerCoordinates(
      final BigInteger authorizationClaimBlockHeight,
      final BigInteger authorizationClaimEpoch,
      final BigInteger sequence,
      final BigInteger blockHeight,
      final BigInteger epoch) {
    Objects.requireNonNull(authorizationClaimBlockHeight, "authorizationClaimBlockHeight");
    Objects.requireNonNull(authorizationClaimEpoch, "authorizationClaimEpoch");
    Objects.requireNonNull(sequence, "emittedSequence");
    Objects.requireNonNull(blockHeight, "emittedBlockHeight");
    Objects.requireNonNull(epoch, "emittedEpoch");
    if (authorizationClaimBlockHeight.signum() < 0
        || authorizationClaimEpoch.signum() < 0
        || sequence.signum() < 0
        || blockHeight.signum() < 0
        || epoch.signum() < 0
        || authorizationClaimBlockHeight.compareTo(U64_MAX) > 0
        || authorizationClaimEpoch.compareTo(U64_MAX) > 0
        || sequence.compareTo(U64_MAX) > 0
        || blockHeight.compareTo(U64_MAX) > 0
        || epoch.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(
          "authorization and emission coordinates must fit unsigned 64-bit integers");
    }
    if (!((authorizationClaimBlockHeight.signum() == 0
            && authorizationClaimEpoch.signum() == 0
            && sequence.signum() == 0
            && blockHeight.signum() == 0
            && epoch.signum() == 0)
        || (authorizationClaimBlockHeight.signum() > 0
            && authorizationClaimEpoch.signum() > 0
            && sequence.signum() > 0
            && blockHeight.signum() > 0
            && epoch.signum() > 0))) {
      throw new IllegalArgumentException(
          "authorization and emission coordinates must all be zero or all be positive");
    }
    if (authorizationClaimBlockHeight.signum() > 0
        && (blockHeight.compareTo(authorizationClaimBlockHeight) < 0
            || epoch.compareTo(authorizationClaimEpoch) < 0)) {
      throw new IllegalArgumentException(
          "emission coordinates must not precede authorization claim coordinates");
    }
  }

  static String requireSubmissionStatus(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (!"submitted".equals(canonical) && !"committed".equals(canonical)) {
      throw new IllegalArgumentException(field + " must equal submitted or committed");
    }
    return canonical;
  }

  static Map<String, Object> snapshotUploadedModelStatus(
      final Map<String, Object> status) {
    Objects.requireNonNull(status, "status");
    if (status.size() != 3
        || !status.containsKey("schema_version")
        || !status.containsKey("bundle")
        || !status.containsKey("artifact")) {
      throw new IllegalArgumentException(
          "status must contain exactly schema_version, bundle, and artifact");
    }
    requireSchemaVersion(
        JsonNumbers.asLong(status.get("schema_version"), "status.schema_version"),
        "status.schema_version");
    if (!(status.get("bundle") instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("status.bundle must be a JSON object");
    }
    final Object artifact = status.get("artifact");
    if (artifact != null && !(artifact instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("status.artifact must be null or a JSON object");
    }
    return immutableJsonObject(status, "status", new IdentityHashMap<>(), 0);
  }

  private static Map<String, Object> immutableJsonObject(
      final Map<?, ?> source,
      final String path,
      final IdentityHashMap<Object, Boolean> active,
      final int depth) {
    if (depth > MAX_JSON_DEPTH) {
      throw new IllegalArgumentException(path + " exceeds maximum JSON nesting depth");
    }
    if (active.put(source, Boolean.TRUE) != null) {
      throw new IllegalArgumentException(path + " must not contain a reference cycle");
    }
    try {
      final Map<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : source.entrySet()) {
        if (!(entry.getKey() instanceof String)) {
          throw new IllegalArgumentException(path + " must use string object keys");
        }
        final String key = (String) entry.getKey();
        copy.put(
            key,
            immutableJsonValue(entry.getValue(), path + "." + key, active, depth + 1));
      }
      return Collections.unmodifiableMap(copy);
    } finally {
      active.remove(source);
    }
  }

  private static Object immutableJsonValue(
      final Object value,
      final String path,
      final IdentityHashMap<Object, Boolean> active,
      final int depth) {
    if (value == null
        || value instanceof String
        || value instanceof Boolean
        || value instanceof BigInteger
        || value instanceof BigDecimal
        || value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return value;
    }
    if (value instanceof Float || value instanceof Double) {
      if (!Double.isFinite(((Number) value).doubleValue())) {
        throw new IllegalArgumentException(path + " must be a finite JSON number");
      }
      return value;
    }
    if (value instanceof Map<?, ?>) {
      return immutableJsonObject((Map<?, ?>) value, path, active, depth);
    }
    if (value instanceof List<?>) {
      if (depth > MAX_JSON_DEPTH) {
        throw new IllegalArgumentException(path + " exceeds maximum JSON nesting depth");
      }
      if (active.put(value, Boolean.TRUE) != null) {
        throw new IllegalArgumentException(path + " must not contain a reference cycle");
      }
      try {
        final List<Object> copy = new ArrayList<>(((List<?>) value).size());
        int index = 0;
        for (final Object element : (List<?>) value) {
          copy.add(immutableJsonValue(element, path + "[" + index + "]", active, depth + 1));
          index++;
        }
        return Collections.unmodifiableList(copy);
      } finally {
        active.remove(value);
      }
    }
    throw new IllegalArgumentException(path + " must contain only JSON values");
  }

  static void requireExecuteResponseState(
      final String submissionStatus,
      final String transactionHash,
      final SoracloudPrivateUploadedModelExecutionReceipt receipt,
      final SoracloudPrivateModelArtifactRef outputArtifact) {
    final String canonicalStatus = requireSubmissionStatus(submissionStatus, "submissionStatus");
    Objects.requireNonNull(receipt, "receipt");
    Objects.requireNonNull(outputArtifact, "outputArtifact");
    if ("committed".equals(canonicalStatus) && transactionHash != null) {
      throw new IllegalArgumentException("transactionHash must be null for committed");
    }
    if ("submitted".equals(canonicalStatus)
        && (receipt.authorizationClaimBlockHeight().signum() != 0
            || receipt.authorizationClaimEpoch().signum() != 0
            || receipt.emittedSequence().signum() != 0
            || receipt.emittedBlockHeight().signum() != 0
            || receipt.emittedEpoch().signum() != 0)) {
      throw new IllegalArgumentException("submitted receipt must use zero ledger coordinates");
    }
    if ("committed".equals(canonicalStatus)
        && (receipt.authorizationClaimBlockHeight().signum() <= 0
            || receipt.authorizationClaimEpoch().signum() <= 0
            || receipt.emittedSequence().signum() <= 0
            || receipt.emittedBlockHeight().signum() <= 0
            || receipt.emittedEpoch().signum() <= 0)) {
      throw new IllegalArgumentException("committed receipt must use positive ledger coordinates");
    }
    if (!sameArtifact(outputArtifact, receipt.outputArtifact())) {
      throw new IllegalArgumentException("outputArtifact must match receipt.outputArtifact");
    }
  }

  static void requireReceiptListMetadata(
      final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts,
      final Long total,
      final long returnedItems,
      final Long remainingItems,
      final boolean hasMore,
      final String countMode,
      final String continueCursor) {
    Objects.requireNonNull(receipts, "receipts");
    if (!"bounded".equals(countMode) && !"exact".equals(countMode)) {
      throw new IllegalArgumentException("countMode must equal bounded or exact");
    }
    if (total != null) {
      requireU32(total.longValue(), "total");
    }
    requireU32(returnedItems, "returnedItems");
    if (remainingItems != null) {
      requireU32(remainingItems.longValue(), "remainingItems");
    }
    if (("bounded".equals(countMode)) != (total == null)) {
      throw new IllegalArgumentException(
          "total must be null for bounded countMode and non-null for exact countMode");
    }
    if (returnedItems != receipts.size()) {
      throw new IllegalArgumentException("returnedItems must equal receipts.size()");
    }
    SoracloudPrivateUploadedModelExecutionReceipt previous = null;
    for (final SoracloudPrivateUploadedModelExecutionReceipt receipt : receipts) {
      if (receipt == null
          || receipt.authorizationClaimBlockHeight().signum() <= 0
          || receipt.authorizationClaimEpoch().signum() <= 0
          || receipt.emittedSequence().signum() <= 0
          || receipt.emittedBlockHeight().signum() <= 0
          || receipt.emittedEpoch().signum() <= 0) {
        throw new IllegalArgumentException(
            "receipt-list entries must have positive ledger coordinates");
      }
      if (previous != null
          && (previous.emittedSequence().compareTo(receipt.emittedSequence()) > 0
              || (previous.emittedSequence().equals(receipt.emittedSequence())
                  && previous.receiptId().compareTo(receipt.receiptId()) >= 0))) {
        throw new IllegalArgumentException(
            "receipt-list entries must be strictly ordered by emittedSequence and receiptId");
      }
      previous = receipt;
    }
    if (("bounded".equals(countMode)) != (remainingItems == null)) {
      throw new IllegalArgumentException(
          "remainingItems must be null for bounded countMode and non-null for exact countMode");
    }
    if (hasMore != (continueCursor != null)) {
      throw new IllegalArgumentException("hasMore must equal continueCursor presence");
    }
    if (remainingItems != null) {
      if (hasMore != (remainingItems.longValue() > 0L)) {
        throw new IllegalArgumentException("hasMore must equal (remainingItems > 0) in exact mode");
      }
      final long saturatedKnownItems =
          Math.min(U32_MAX, returnedItems + remainingItems.longValue());
      if (total.longValue() < saturatedKnownItems) {
        throw new IllegalArgumentException(
            "total must cover returnedItems and remainingItems in exact mode");
      }
    }
  }

  static String requirePrivateReceiptCursor(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.length() != PRIVATE_RECEIPT_CURSOR_LENGTH_V1) {
      throw new IllegalArgumentException(field + " must be an exact canonical V1 receipt cursor");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (!(character >= 'A' && character <= 'Z')
          && !(character >= 'a' && character <= 'z')
          && !(character >= '0' && character <= '9')
          && character != '-'
          && character != '_') {
        throw new IllegalArgumentException(field + " must be an exact canonical V1 receipt cursor");
      }
    }
    return canonical;
  }

  static boolean sameArtifact(
      final SoracloudPrivateModelArtifactRef left,
      final SoracloudPrivateModelArtifactRef right) {
    return left.schemaVersion() == right.schemaVersion()
        && left.ciphertextBytes() == right.ciphertextBytes()
        && Arrays.equals(left.sorafsManifestDigest(), right.sorafsManifestDigest())
        && left.sorafsRootCid().equals(right.sorafsRootCid())
        && left.artifactHash().equals(right.artifactHash())
        && left.artifactRole().equals(right.artifactRole());
  }

  private static boolean isLowOrderX25519PublicKey(final byte[] publicKey) {
    final X25519PublicKeyParameters peer = new X25519PublicKeyParameters(publicKey, 0);
    final X25519PrivateKeyParameters probe =
        new X25519PrivateKeyParameters(LOW_ORDER_X25519_PROBE_PRIVATE_KEY, 0);
    final X25519Agreement agreement = new X25519Agreement();
    final byte[] shared = new byte[32];
    try {
      agreement.init(probe);
      agreement.calculateAgreement(peer, shared, 0);
      for (final byte value : shared) {
        if (value != 0) {
          return false;
        }
      }
      return true;
    } catch (final IllegalStateException ignored) {
      return true;
    } finally {
      Arrays.fill(shared, (byte) 0);
    }
  }

  private static boolean isWhitespace(final char value) {
    return Character.isWhitespace(value) || Character.isSpaceChar(value);
  }

  private static boolean isBidiControl(final char value) {
    return value == '\u061c'
        || value == '\u200e'
        || value == '\u200f'
        || (value >= '\u202a' && value <= '\u202e')
        || (value >= '\u2066' && value <= '\u2069');
  }

  private static byte[] filledBytes(final byte value, final int size) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, value);
    return bytes;
  }
}
