package org.hyperledger.iroha.android.address;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

import org.hyperledger.iroha.android.crypto.Blake2b;

public final class AccountAddress {

  public static final String DEFAULT_DOMAIN_NAME = "default";
  public static final int DEFAULT_IH58_PREFIX = 753;

  private static final byte[] IH58_CHECKSUM_PREFIX = "IH58PRE".getBytes(StandardCharsets.UTF_8);
  private static final String COMPRESSED_SENTINEL = "sora";
  private static final int COMPRESSED_CHECKSUM_LEN = 6;
  private static final int BECH32M_CONST = 0x2bc830a3;
  private static final String COMPRESSED_WARNING =
      "Compressed Sora addresses rely on half-width kana and are only interoperable inside "
          + "Sora-aware apps. Prefer IH58 when sharing with explorers, wallets, or QR codes.";

  private static final String[] IH58_ALPHABET = {
      "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H",
      "J", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b",
      "c", "d", "e", "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t",
      "u", "v", "w", "x", "y", "z"
  };

  private static final String[] SORA_KANA = {
      "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ",
      "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ",
      "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ"
  };

  private static final String[] SORA_KANA_FULLWIDTH = {
      "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ", "レ",
      "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ", "コ", "エ",
      "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス"
  };

  private static final String[] COMPRESSED_ALPHABET;
  private static final String[] COMPRESSED_ALPHABET_FULLWIDTH;
  private static final Map<String, Integer> COMPRESSED_INDEX;
  private static final Map<String, Integer> IH58_INDEX;
  private static final int COMPRESSED_BASE;
  private static final BigInteger BASE_58 = BigInteger.valueOf(58L);

  private static volatile boolean allowMlDsa;
  private static volatile boolean allowGost;
  private static volatile boolean allowSm2;

  static {
    configureCurveSupport(CurveSupportConfig.ed25519Only());
    COMPRESSED_ALPHABET = new String[IH58_ALPHABET.length + SORA_KANA.length];
    System.arraycopy(IH58_ALPHABET, 0, COMPRESSED_ALPHABET, 0, IH58_ALPHABET.length);
    System.arraycopy(SORA_KANA, 0, COMPRESSED_ALPHABET, IH58_ALPHABET.length, SORA_KANA.length);
    COMPRESSED_ALPHABET_FULLWIDTH = new String[IH58_ALPHABET.length + SORA_KANA_FULLWIDTH.length];
    System.arraycopy(IH58_ALPHABET, 0, COMPRESSED_ALPHABET_FULLWIDTH, 0, IH58_ALPHABET.length);
    System.arraycopy(
        SORA_KANA_FULLWIDTH, 0, COMPRESSED_ALPHABET_FULLWIDTH, IH58_ALPHABET.length, SORA_KANA_FULLWIDTH.length);
    COMPRESSED_BASE = COMPRESSED_ALPHABET.length;

    COMPRESSED_INDEX = new HashMap<>();
    for (int i = 0; i < COMPRESSED_ALPHABET.length; i++) {
      COMPRESSED_INDEX.put(COMPRESSED_ALPHABET[i], i);
    }
    for (int i = 0; i < COMPRESSED_ALPHABET_FULLWIDTH.length; i++) {
      COMPRESSED_INDEX.put(COMPRESSED_ALPHABET_FULLWIDTH[i], i);
    }

    IH58_INDEX = new HashMap<>();
    for (int i = 0; i < IH58_ALPHABET.length; i++) {
      IH58_INDEX.put(IH58_ALPHABET[i], i);
    }
  }

  private final byte[] canonicalBytes;

  private AccountAddress(final byte[] canonicalBytes) {
    this.canonicalBytes = canonicalBytes;
  }

  public byte[] canonicalBytes() {
    return Arrays.copyOf(canonicalBytes, canonicalBytes.length);
  }

  /**
   * Retained for API compatibility. Canonical payloads are now selector-free and this helper is a no-op.
   */
  public AccountAddress rebasedFromDefaultDomain(final String domainLabel) throws AccountAddressException {
    Objects.requireNonNull(domainLabel, "domainLabel must not be null");
    parseCanonical(canonicalBytes);
    return this;
  }

  public String canonicalHex() {
    return "0x" + bytesToHex(canonicalBytes);
  }

  public String toIH58(final int networkPrefix) throws AccountAddressException {
    return encodeIh58(networkPrefix, canonicalBytes);
  }

  public String toCompressedSora() throws AccountAddressException {
    return encodeCompressed(canonicalBytes, COMPRESSED_ALPHABET);
  }

  public String toCompressedSoraFullWidth() throws AccountAddressException {
    return encodeCompressed(canonicalBytes, COMPRESSED_ALPHABET_FULLWIDTH);
  }

  /**
   * Convenience helper that surfaces the IH58 (preferred)/sora (second-best) pair alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats() throws AccountAddressException {
    return displayFormats(DEFAULT_IH58_PREFIX);
  }

  /**
   * Convenience helper that surfaces the IH58 (preferred)/sora (second-best) pair alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats(final int networkPrefix) throws AccountAddressException {
    final String ih58 = toIH58(networkPrefix);
    final String compressed = toCompressedSora();
    return new DisplayFormats(ih58, compressed, networkPrefix, COMPRESSED_WARNING);
  }

  /**
   * Returns the single-key controller payload when this address encodes a single-key controller.
   *
   * <p>Multisig addresses return {@link Optional#empty()}.
   */
  public Optional<SingleKeyPayload> singleKeyPayload() throws AccountAddressException {
    parseCanonical(canonicalBytes);
    return extractSingleKeyPayload(canonicalBytes);
  }

  /**
   * Returns the multisig policy payload when this address encodes a multisig controller.
   *
   * <p>Single-key addresses return {@link Optional#empty()}.
   */
  public Optional<MultisigPolicyPayload> multisigPolicyPayload() throws AccountAddressException {
    parseCanonical(canonicalBytes);
    return extractMultisigPayload(canonicalBytes);
  }

  public static String compressedWarningMessage() {
    return COMPRESSED_WARNING;
  }

  public static AccountAddress fromAccount(
      final String domain,
      final byte[] publicKey,
      final String algorithm) throws AccountAddressException {
    if (publicKey.length > 0xFF) {
      throw new AccountAddressException(
          AccountAddressErrorCode.KEY_PAYLOAD_TOO_LONG, "key payload too long: " + publicKey.length);
    }
    final byte header = encodeHeader((byte) 0, (byte) 0, (byte) 1);

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(header);
    // Canonical payloads are globally scoped and no longer encode domain selectors.
    if (domain == null) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_LENGTH, "domain must not be null");
    }

    out.write(0x00);
    out.write(curveIdForAlgorithm(algorithm));
    out.write(publicKey.length);
    out.write(publicKey, 0, publicKey.length);

    return fromCanonicalBytes(out.toByteArray());
  }

  /**
   * Constructs a multisig account address from the provided policy payload.
   */
  public static AccountAddress fromMultisigPolicy(
      final String domain,
      final MultisigPolicyPayload policy) throws AccountAddressException {
    if (domain == null || domain.isBlank()) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "domain must not be blank");
    }
    if (policy == null) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "multisig policy must not be null");
    }

    final List<MultisigMemberPayload> members = policy.members();
    if (members.isEmpty()) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: zero members");
    }
    if (members.size() > 0xFF) {
      throw new AccountAddressException(
          AccountAddressErrorCode.MULTISIG_MEMBER_OVERFLOW,
          "InvalidMultisigPolicy: too many members (" + members.size() + ")");
    }

    long totalWeight = 0L;
    for (final MultisigMemberPayload member : members) {
      if (member.weight() <= 0) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: non-positive weight");
      }
      if (member.weight() > 0xFFFF) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: weight too large");
      }
      ensureCurveEnabled(member.curveId(), "curve id " + member.curveId());
      if (member.publicKey().length == 0) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: invalid key length");
      }
      if (member.publicKey().length > 0xFFFF) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: key too long");
      }
      totalWeight += member.weight();
    }
    if (policy.threshold() <= 0) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: ZeroThreshold");
    }
    if (totalWeight < policy.threshold()) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
          "InvalidMultisigPolicy: threshold exceeds total weight");
    }

    final byte header = encodeHeader((byte) 0, (byte) 0, (byte) 1);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(header);
    // Canonical payloads are globally scoped and no longer encode domain selectors.

    out.write(0x01); // multisig controller tag
    out.write(policy.version() & 0xFF);
    out.write((policy.threshold() >> 8) & 0xFF);
    out.write(policy.threshold() & 0xFF);
    out.write(members.size() & 0xFF);

    for (final MultisigMemberPayload member : members) {
      final int curveId = member.curveId() & 0xFF;
      final int weight = member.weight();
      final byte[] keyBytes = member.publicKey();
      out.write(curveId);
      out.write((weight >> 8) & 0xFF);
      out.write(weight & 0xFF);
      out.write((keyBytes.length >> 8) & 0xFF);
      out.write(keyBytes.length & 0xFF);
      out.write(keyBytes, 0, keyBytes.length);
    }

    return fromCanonicalBytes(out.toByteArray());
  }

  public static AccountAddress fromCanonicalBytes(final byte[] canonical) throws AccountAddressException {
    final byte[] copy = Arrays.copyOf(canonical, canonical.length);
    parseCanonical(copy);
    return new AccountAddress(copy);
  }

  public static AccountAddress fromCanonicalHex(final String encoded) throws AccountAddressException {
    final String body = encoded.startsWith("0x") || encoded.startsWith("0X")
        ? encoded.substring(2)
        : encoded;
    final byte[] bytes = hexToBytes(body);
    return fromCanonicalBytes(bytes);
  }

  public static AccountAddress fromIH58(final String encoded, final Integer expectedPrefix)
      throws AccountAddressException {
    final DecodeResult decode = decodeIh58(encoded);
    if (expectedPrefix != null && decode.networkPrefix != expectedPrefix) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
          "unexpected IH58 network prefix: expected " + expectedPrefix + ", found " + decode.networkPrefix);
    }
    final AccountAddress address = fromCanonicalBytes(decode.canonical);
    final String reencoded = address.toIH58(decode.networkPrefix);
    if (!reencoded.equals(encoded)) {
      throw new AccountAddressException(AccountAddressErrorCode.CHECKSUM_MISMATCH, "IH58 checksum mismatch");
    }
    return address;
  }

  public static AccountAddress fromCompressedSora(final String encoded) throws AccountAddressException {
    final byte[] canonical = decodeCompressed(encoded);
    return fromCanonicalBytes(canonical);
  }

  public static ParseResult parseAny(final String input, final Integer expectedPrefix)
      throws AccountAddressException {
    final String trimmed = input.trim();
    if (trimmed.isEmpty()) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "address string is empty");
    }
    if (trimmed.startsWith(COMPRESSED_SENTINEL)) {
      return new ParseResult(fromCompressedSora(trimmed), Format.COMPRESSED);
    }
    if (containsCompressedGlyph(trimmed)) {
      throw new AccountAddressException(
          AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL, "compressed address must start with sora sentinel");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      return new ParseResult(fromCanonicalHex(trimmed), Format.CANONICAL_HEX);
    }
    try {
      return new ParseResult(fromIH58(trimmed, expectedPrefix), Format.IH58);
    } catch (final AccountAddressException ex) {
      final String message = ex.getMessage();
      if (message != null
          && (message.startsWith("unexpected IH58 network prefix")
              || message.contains("IH58"))) {
        throw ex;
      }
    }
    throw new AccountAddressException(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format");
  }

  private static boolean containsCompressedGlyph(final String value) {
    for (int i = 0; i < value.length(); i++) {
      final String symbol = String.valueOf(value.charAt(i));
      if (COMPRESSED_INDEX.containsKey(symbol) && !IH58_INDEX.containsKey(symbol)) {
        return true;
      }
    }
    return false;
  }

  public enum Format {
    IH58,
    COMPRESSED,
    CANONICAL_HEX
  }

  public enum AccountAddressErrorCode {
    UNSUPPORTED_ALGORITHM("ERR_UNSUPPORTED_ALGORITHM"),
    KEY_PAYLOAD_TOO_LONG("ERR_KEY_PAYLOAD_TOO_LONG"),
    INVALID_HEADER_VERSION("ERR_INVALID_HEADER_VERSION"),
    INVALID_NORM_VERSION("ERR_INVALID_NORM_VERSION"),
    INVALID_IH58_PREFIX("ERR_INVALID_IH58_PREFIX"),
    INVALID_IH58_ENCODING("ERR_INVALID_IH58_ENCODING"),
    INVALID_IH58_PREFIX_ENCODING("ERR_INVALID_IH58_PREFIX_ENCODING"),
    INVALID_LENGTH("ERR_INVALID_LENGTH"),
    CHECKSUM_MISMATCH("ERR_CHECKSUM_MISMATCH"),
    INVALID_HEX_ADDRESS("ERR_INVALID_HEX_ADDRESS"),
    UNEXPECTED_NETWORK_PREFIX("ERR_UNEXPECTED_NETWORK_PREFIX"),
    UNKNOWN_ADDRESS_CLASS("ERR_UNKNOWN_ADDRESS_CLASS"),
    UNKNOWN_DOMAIN_TAG("ERR_UNKNOWN_DOMAIN_TAG"),
    UNEXPECTED_EXTENSION_FLAG("ERR_UNEXPECTED_EXTENSION_FLAG"),
    UNKNOWN_CONTROLLER_TAG("ERR_UNKNOWN_CONTROLLER_TAG"),
    UNKNOWN_CURVE("ERR_UNKNOWN_CURVE"),
    INVALID_COMPRESSED_CHAR("ERR_INVALID_COMPRESSED_CHAR"),
    INVALID_COMPRESSED_DIGIT("ERR_INVALID_COMPRESSED_DIGIT"),
    MISSING_COMPRESSED_SENTINEL("ERR_MISSING_COMPRESSED_SENTINEL"),
    UNEXPECTED_TRAILING_BYTES("ERR_UNEXPECTED_TRAILING_BYTES"),
    MULTISIG_MEMBER_OVERFLOW("ERR_MULTISIG_MEMBER_OVERFLOW"),
    INVALID_MULTISIG_POLICY("ERR_INVALID_MULTISIG_POLICY"),
    UNSUPPORTED_ADDRESS_FORMAT("ERR_UNSUPPORTED_ADDRESS_FORMAT"),
    INVALID_COMPRESSED_BASE("ERR_INVALID_COMPRESSED_BASE"),
    COMPRESSED_TOO_SHORT("ERR_COMPRESSED_TOO_SHORT");

    private final String code;

    AccountAddressErrorCode(final String code) {
      this.code = code;
    }

    public String value() {
      return code;
    }
  }

  public static final class ParseResult {
    public final AccountAddress address;
    public final Format format;

    private ParseResult(final AccountAddress address, final Format format) {
      this.address = address;
      this.format = format;
    }
  }

  public static final class DisplayFormats {
    public final String ih58;
    public final String compressed;
    public final int networkPrefix;
    public final String compressedWarning;

    private DisplayFormats(
        final String ih58,
        final String compressed,
        final int networkPrefix,
        final String compressedWarning) {
      this.ih58 = ih58;
      this.compressed = compressed;
      this.networkPrefix = networkPrefix;
      this.compressedWarning = compressedWarning;
    }
  }

  public static final class SingleKeyPayload {
    private final int curveId;
    private final byte[] publicKey;

    private SingleKeyPayload(final int curveId, final byte[] publicKey) {
      this.curveId = curveId;
      this.publicKey = Arrays.copyOf(publicKey, publicKey.length);
    }

    public int curveId() {
      return curveId;
    }

    public byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }
  }

  public static final class MultisigPolicyPayload {
    private final int version;
    private final int threshold;
    private final List<MultisigMemberPayload> members;

    private MultisigPolicyPayload(
        final int version,
        final int threshold,
        final List<MultisigMemberPayload> members) {
      this.version = version;
      this.threshold = threshold;
      this.members = Collections.unmodifiableList(new ArrayList<>(members));
    }

    public static MultisigPolicyPayload of(
        final int version,
        final int threshold,
        final List<MultisigMemberPayload> members) {
      if (members == null) {
        throw new IllegalArgumentException("members must not be null");
      }
      return new MultisigPolicyPayload(version, threshold, members);
    }

    public int version() {
      return version;
    }

    public int threshold() {
      return threshold;
    }

    public List<MultisigMemberPayload> members() {
      return members;
    }
  }

  public static final class MultisigMemberPayload {
    private final int curveId;
    private final int weight;
    private final byte[] publicKey;

    private MultisigMemberPayload(final int curveId, final int weight, final byte[] publicKey) {
      this.curveId = curveId;
      this.weight = weight;
      this.publicKey = Arrays.copyOf(publicKey, publicKey.length);
    }

    public static MultisigMemberPayload of(
        final int curveId,
        final int weight,
        final byte[] publicKey) {
      if (publicKey == null) {
        throw new IllegalArgumentException("publicKey must not be null");
      }
      return new MultisigMemberPayload(curveId, weight, publicKey);
    }

    public int curveId() {
      return curveId;
    }

    public int weight() {
      return weight;
    }

    public byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }
  }

  public static final class AccountAddressException extends Exception {

    private static final long serialVersionUID = 1L;
    private final AccountAddressErrorCode code;

    public AccountAddressException(final AccountAddressErrorCode code, final String message) {
      super(message);
      this.code = code;
    }

    public AccountAddressErrorCode getCode() {
      return code;
    }

    public String getCodeValue() {
      return code.value();
    }
  }

  // -- Canonical decoding helpers --

  private static void parseCanonical(final byte[] canonical) throws AccountAddressException {
    if (canonical.length < 2) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte header = canonical[0];
    decodeHeader(header);
    AccountAddressException canonicalError = null;
    try {
      final ParsedController parsed = parseControllerAt(canonical, 1);
      if (parsed.cursor == canonical.length) {
        return;
      }
      canonicalError =
          new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
              "unexpected trailing bytes in canonical payload");
    } catch (final AccountAddressException ex) {
      canonicalError = ex;
    }

    // Backward compatibility: legacy payloads may include explicit domain selector bytes.
    try {
      final LegacyDomainDecode legacyDomain = decodeLegacyDomainAt(canonical, 1);
      final ParsedController parsed = parseControllerAt(canonical, legacyDomain.cursor);
      if (parsed.cursor != canonical.length) {
        throw new AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
            "unexpected trailing bytes in canonical payload");
      }
    } catch (final AccountAddressException ex) {
      throw canonicalError != null ? canonicalError : ex;
    }
  }

  private static Optional<SingleKeyPayload> extractSingleKeyPayload(final byte[] canonical)
      throws AccountAddressException {
    if (canonical.length < 2) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte header = canonical[0];
    decodeHeader(header);

    AccountAddressException canonicalError = null;
    try {
      final ParsedController parsed = parseControllerAt(canonical, 1);
      if (parsed.cursor == canonical.length) {
        return Optional.ofNullable(parsed.singleKey);
      }
      canonicalError =
          new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
              "unexpected trailing bytes in canonical payload");
    } catch (final AccountAddressException ex) {
      canonicalError = ex;
    }

    try {
      final LegacyDomainDecode legacyDomain = decodeLegacyDomainAt(canonical, 1);
      final ParsedController parsed = parseControllerAt(canonical, legacyDomain.cursor);
      if (parsed.cursor != canonical.length) {
        throw new AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
            "unexpected trailing bytes in canonical payload");
      }
      return Optional.ofNullable(parsed.singleKey);
    } catch (final AccountAddressException ex) {
      throw canonicalError != null ? canonicalError : ex;
    }
  }

  private static Optional<MultisigPolicyPayload> extractMultisigPayload(final byte[] canonical)
      throws AccountAddressException {
    if (canonical.length < 2) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte header = canonical[0];
    decodeHeader(header);

    AccountAddressException canonicalError = null;
    try {
      final ParsedController parsed = parseControllerAt(canonical, 1);
      if (parsed.cursor == canonical.length) {
        return Optional.ofNullable(parsed.multisigPolicy);
      }
      canonicalError =
          new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
              "unexpected trailing bytes in canonical payload");
    } catch (final AccountAddressException ex) {
      canonicalError = ex;
    }

    try {
      final LegacyDomainDecode legacyDomain = decodeLegacyDomainAt(canonical, 1);
      final ParsedController parsed = parseControllerAt(canonical, legacyDomain.cursor);
      if (parsed.cursor != canonical.length) {
        throw new AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
            "unexpected trailing bytes in canonical payload");
      }
      return Optional.ofNullable(parsed.multisigPolicy);
    } catch (final AccountAddressException ex) {
      throw canonicalError != null ? canonicalError : ex;
    }
  }

  private static ParsedController parseControllerAt(final byte[] canonical, int cursor)
      throws AccountAddressException {
    if (cursor >= canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }

    final int controllerTag = canonical[cursor++] & 0xFF;
    switch (controllerTag) {
      case 0x00: {
        if (cursor + 2 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        final int curveId = canonical[cursor++] & 0xFF;
        ensureCurveEnabled(curveId, "curve id " + curveId);
        final int keyLen = canonical[cursor++] & 0xFF;
        final int end = cursor + keyLen;
        if (end > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        final byte[] key = Arrays.copyOfRange(canonical, cursor, end);
        return ParsedController.single(end, new SingleKeyPayload(curveId, key));
      }
      case 0x01: {
        if (cursor + 4 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        final int version = canonical[cursor++] & 0xFF;
        final int threshold =
            ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
        cursor += 2;
        final int memberCount = canonical[cursor++] & 0xFF;
        if (memberCount == 0) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: zero members");
        }

        long totalWeight = 0L;
        final List<MultisigMemberPayload> members = new ArrayList<>(memberCount);
        for (int i = 0; i < memberCount; i++) {
          if (cursor + 5 > canonical.length) {
            throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
          }
          final int curveId = canonical[cursor++] & 0xFF;
          ensureCurveEnabled(curveId, "curve id " + curveId);
          final int weight =
              ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
          cursor += 2;
          if (weight <= 0) {
            throw new AccountAddressException(
                AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: non-positive weight");
          }
          final int keyLen =
              ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
          cursor += 2;
          if (keyLen <= 0) {
            throw new AccountAddressException(
                AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: invalid key length");
          }
          if (cursor + keyLen > canonical.length) {
            throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
          }
          final byte[] key = Arrays.copyOfRange(canonical, cursor, cursor + keyLen);
          cursor += keyLen;
          members.add(new MultisigMemberPayload(curveId, weight, key));
          totalWeight += weight;
        }
        if (threshold <= 0) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: ZeroThreshold");
        }
        if (totalWeight < threshold) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
              "InvalidMultisigPolicy: threshold exceeds total weight");
        }
        return ParsedController.multisig(
            cursor, new MultisigPolicyPayload(version, threshold, members));
      }
      default:
        throw new AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG,
            "unknown controller payload tag: " + controllerTag);
    }
  }

  private static LegacyDomainDecode decodeLegacyDomainAt(final byte[] canonical, int cursor)
      throws AccountAddressException {
    if (cursor >= canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }

    final int domainTag = canonical[cursor++] & 0xFF;
    switch (domainTag) {
      case 0x00:
        return new LegacyDomainDecode(domainTag, cursor);
      case 0x01:
        if (cursor + 12 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        return new LegacyDomainDecode(domainTag, cursor + 12);
      case 0x02:
        if (cursor + 4 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        return new LegacyDomainDecode(domainTag, cursor + 4);
      default:
        throw new AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG,
            "unknown domain selector tag: " + domainTag);
    }
  }

  private static final class ParsedController {
    final int tag;
    final int cursor;
    final SingleKeyPayload singleKey;
    final MultisigPolicyPayload multisigPolicy;

    private ParsedController(
        final int tag,
        final int cursor,
        final SingleKeyPayload singleKey,
        final MultisigPolicyPayload multisigPolicy) {
      this.tag = tag;
      this.cursor = cursor;
      this.singleKey = singleKey;
      this.multisigPolicy = multisigPolicy;
    }

    static ParsedController single(final int cursor, final SingleKeyPayload payload) {
      return new ParsedController(0x00, cursor, payload, null);
    }

    static ParsedController multisig(final int cursor, final MultisigPolicyPayload payload) {
      return new ParsedController(0x01, cursor, null, payload);
    }
  }

  private static final class LegacyDomainDecode {
    final int tag;
    final int cursor;

    private LegacyDomainDecode(final int tag, final int cursor) {
      this.tag = tag;
      this.cursor = cursor;
    }
  }

  private static byte encodeHeader(final byte version, final byte classId, final byte normVersion)
      throws AccountAddressException {
    if (version < 0 || version > 0b111) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_HEADER_VERSION, "invalid address header version: " + version);
    }
    if (normVersion < 0 || normVersion > 0b11) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_NORM_VERSION, "invalid normalization version: " + normVersion);
    }
    return (byte) (((version & 0b111) << 5) | ((classId & 0b11) << 3) | ((normVersion & 0b11) << 1));
  }

  private static void decodeHeader(final byte header) throws AccountAddressException {
    final int classBits = (header >> 3) & 0b11;
    final int extFlag = header & 0x01;
    if (extFlag != 0) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNEXPECTED_EXTENSION_FLAG, "address header reserves extension flag but it was set");
    }
    if (classBits != 0 && classBits != 1) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNKNOWN_ADDRESS_CLASS, "unknown address class: " + classBits);
    }
  }

  // -- Encoding helpers --

  private static byte curveIdForAlgorithm(final String algorithm) throws AccountAddressException {
    final String normalized = algorithm.trim().toLowerCase();
    final int curveId;
    switch (normalized) {
      case "ed25519":
      case "ed":
        curveId = 0x01;
        break;
      case "ml-dsa":
      case "mldsa":
      case "ml_dsa":
        curveId = 0x02;
        break;
      case "gost256a":
      case "gost-256-a":
        curveId = 0x0A;
        break;
      case "gost256b":
      case "gost-256-b":
        curveId = 0x0B;
        break;
      case "gost256c":
      case "gost-256-c":
        curveId = 0x0C;
        break;
      case "gost512a":
      case "gost-512-a":
        curveId = 0x0D;
        break;
      case "gost512b":
      case "gost-512-b":
        curveId = 0x0E;
        break;
      case "sm2":
      case "sm-2":
        curveId = 0x0F;
        break;
      default:
        throw new AccountAddressException(
            AccountAddressErrorCode.UNSUPPORTED_ALGORITHM, "unsupported signing algorithm: " + algorithm);
    }
    ensureCurveEnabled(curveId, "signing algorithm: " + normalized);
    return (byte) curveId;
  }

  public static void configureCurveSupport(final CurveSupportConfig config) {
    final CurveSupportConfig effective = Objects.requireNonNull(config, "config");
    allowMlDsa = effective.allowMlDsa;
    allowGost = effective.allowGost;
    allowSm2 = effective.allowSm2;
  }

  private static void ensureCurveEnabled(final int curveId, final String context)
      throws AccountAddressException {
    if (!isCurveEnabled(curveId)) {
      final boolean known = isKnownCurveId(curveId);
      final AccountAddressErrorCode code =
          known ? AccountAddressErrorCode.UNSUPPORTED_ALGORITHM : AccountAddressErrorCode.UNKNOWN_CURVE;
      final String reason =
          known ? context + " disabled by configuration: " + curveName(curveId) : "unknown curve id: " + curveName(curveId);
      throw new AccountAddressException(code, reason);
    }
  }

  private static boolean isCurveEnabled(final int curveId) {
    final int id = curveId & 0xFF;
    switch (id) {
      case 0x01:
        return true;
      case 0x02:
        return allowMlDsa;
      case 0x0A:
      case 0x0B:
      case 0x0C:
      case 0x0D:
      case 0x0E:
        return allowGost;
      case 0x0F:
        return allowSm2;
      default:
        return false;
    }
  }

  private static boolean isKnownCurveId(final int curveId) {
    switch (curveId & 0xFF) {
      case 0x01:
      case 0x02:
      case 0x0A:
      case 0x0B:
      case 0x0C:
      case 0x0D:
      case 0x0E:
      case 0x0F:
        return true;
      default:
        return false;
    }
  }

  private static String curveName(final int curveId) {
    switch (curveId & 0xFF) {
      case 0x01:
        return "ed25519";
      case 0x02:
        return "ml-dsa";
      case 0x0A:
        return "gost256a";
      case 0x0B:
        return "gost256b";
      case 0x0C:
        return "gost256c";
      case 0x0D:
        return "gost512a";
      case 0x0E:
        return "gost512b";
      case 0x0F:
        return "sm2";
      default:
        return "0x" + Integer.toHexString(curveId & 0xFF);
    }
  }

  private static String encodeIh58(final int prefix, final byte[] canonical) throws AccountAddressException {
    final byte[] prefixBytes = encodeIh58Prefix(prefix);
    final byte[] body = new byte[prefixBytes.length + canonical.length];
    System.arraycopy(prefixBytes, 0, body, 0, prefixBytes.length);
    System.arraycopy(canonical, 0, body, prefixBytes.length, canonical.length);

    final byte[] checksumInput = new byte[IH58_CHECKSUM_PREFIX.length + body.length];
    System.arraycopy(IH58_CHECKSUM_PREFIX, 0, checksumInput, 0, IH58_CHECKSUM_PREFIX.length);
    System.arraycopy(body, 0, checksumInput, IH58_CHECKSUM_PREFIX.length, body.length);

    final byte[] checksum = Blake2b.digest512(checksumInput);
    final byte[] payload = new byte[body.length + 2];
    System.arraycopy(body, 0, payload, 0, body.length);
    payload[payload.length - 2] = checksum[0];
    payload[payload.length - 1] = checksum[1];

    return encodeBase58(payload);
  }

  private static DecodeResult decodeIh58(final String encoded) throws AccountAddressException {
    final byte[] body = decodeBase58(encoded);
    if (body.length < 3) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
    }
    final byte[] payload = Arrays.copyOf(body, body.length - 2);
    final byte[] checksumBytes = Arrays.copyOfRange(body, body.length - 2, body.length);
    final PrefixResult prefixResult = decodeIh58Prefix(payload);

    final byte[] checksumInput = new byte[IH58_CHECKSUM_PREFIX.length + payload.length];
    System.arraycopy(IH58_CHECKSUM_PREFIX, 0, checksumInput, 0, IH58_CHECKSUM_PREFIX.length);
    System.arraycopy(payload, 0, checksumInput, IH58_CHECKSUM_PREFIX.length, payload.length);
    final byte[] expected = Arrays.copyOf(Blake2b.digest512(checksumInput), 2);
    if (!Arrays.equals(checksumBytes, expected)) {
      throw new AccountAddressException(AccountAddressErrorCode.CHECKSUM_MISMATCH, "IH58 checksum mismatch");
    }

    final byte[] canonical = Arrays.copyOfRange(payload, prefixResult.prefixLength, payload.length);
    return new DecodeResult(prefixResult.networkPrefix, canonical);
  }

  private static final class DecodeResult {
    final int networkPrefix;
    final byte[] canonical;

    DecodeResult(final int networkPrefix, final byte[] canonical) {
      this.networkPrefix = networkPrefix;
      this.canonical = canonical;
    }
  }

  private static byte[] encodeIh58Prefix(final int prefix) throws AccountAddressException {
    if (prefix < 0 || prefix > 0x3FFF) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_IH58_PREFIX, "invalid IH58 prefix: " + prefix);
    }
    if (prefix <= 63) {
      return new byte[] { (byte) prefix };
    }
    final int lower = (prefix & 0b0011_1111) | 0b0100_0000;
    final int upper = prefix >> 6;
    return new byte[] { (byte) lower, (byte) upper };
  }

  private static PrefixResult decodeIh58Prefix(final byte[] payload) throws AccountAddressException {
    if (payload.length == 0) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
    }
    final int first = payload[0] & 0xFF;
    if (first <= 63) {
      return new PrefixResult(first, 1);
    }
    if ((first & 0b0100_0000) != 0) {
      if (payload.length < 2) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload");
      }
      final int value = ((payload[1] & 0xFF) << 6) | (first & 0x3F);
      return new PrefixResult(value, 2);
    }
    throw new AccountAddressException(AccountAddressErrorCode.INVALID_IH58_PREFIX_ENCODING, "invalid IH58 prefix encoding: " + first);
  }

  private static final class PrefixResult {
    final int networkPrefix;
    final int prefixLength;

    PrefixResult(final int networkPrefix, final int prefixLength) {
      this.networkPrefix = networkPrefix;
      this.prefixLength = prefixLength;
    }
  }

  private static String encodeCompressed(final byte[] canonical, final String[] alphabet)
      throws AccountAddressException {
    final int[] digits = encodeBaseN(canonical, COMPRESSED_BASE);
    final int[] checksum = compressedChecksumDigits(canonical);
    final StringBuilder sb = new StringBuilder();
    sb.append(COMPRESSED_SENTINEL);
    for (final int digit : digits) {
      sb.append(alphabet[digit]);
    }
    for (final int digit : checksum) {
      sb.append(alphabet[digit]);
    }
    return sb.toString();
  }

  private static byte[] decodeCompressed(final String encoded) throws AccountAddressException {
    if (!encoded.startsWith(COMPRESSED_SENTINEL)) {
      throw new AccountAddressException(AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL, "compressed address must start with sora sentinel");
    }
    final String payload = encoded.substring(COMPRESSED_SENTINEL.length());
    if (payload.length() <= COMPRESSED_CHECKSUM_LEN) {
      throw new AccountAddressException(AccountAddressErrorCode.COMPRESSED_TOO_SHORT, "compressed address is too short");
    }
    final int[] digits = new int[payload.length()];
    for (int i = 0; i < payload.length(); i++) {
      final String symbol = String.valueOf(payload.charAt(i));
      final Integer value = COMPRESSED_INDEX.get(symbol);
      if (value == null) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_COMPRESSED_CHAR, "invalid compressed alphabet symbol: " + symbol);
      }
      digits[i] = value;
    }
    final int[] dataDigits = Arrays.copyOf(digits, digits.length - COMPRESSED_CHECKSUM_LEN);
    final int[] checksumDigits = Arrays.copyOfRange(digits, digits.length - COMPRESSED_CHECKSUM_LEN, digits.length);
    final byte[] canonical = decodeBaseN(dataDigits, COMPRESSED_BASE);
    final int[] expected = compressedChecksumDigits(canonical);
    if (!Arrays.equals(checksumDigits, expected)) {
      throw new AccountAddressException(AccountAddressErrorCode.CHECKSUM_MISMATCH, "compressed checksum mismatch");
    }
    return canonical;
  }

  private static int[] encodeBaseN(final byte[] input, final int base) throws AccountAddressException {
    if (base < 2) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for encoding");
    }
    if (input.length == 0) {
      return new int[] { 0 };
    }
    final int[] value = new int[input.length];
    for (int i = 0; i < input.length; i++) {
      value[i] = input[i] & 0xFF;
    }
    int leadingZeros = 0;
    while (leadingZeros < value.length && value[leadingZeros] == 0) {
      leadingZeros++;
    }
    final List<Integer> digits = new ArrayList<>();
    int start = leadingZeros;
    while (start < value.length) {
      int remainder = 0;
      for (int i = start; i < value.length; i++) {
        final int acc = (remainder << 8) | value[i];
        value[i] = acc / base;
        remainder = acc % base;
      }
      digits.add(remainder);
      while (start < value.length && value[start] == 0) {
        start++;
      }
    }
    for (int i = 0; i < leadingZeros; i++) {
      digits.add(0);
    }
    if (digits.isEmpty()) {
      digits.add(0);
    }
    Collections.reverse(digits);
    final int[] result = new int[digits.size()];
    for (int i = 0; i < digits.size(); i++) {
      result[i] = digits.get(i);
    }
    return result;
  }

  private static String encodeBase58(final byte[] input) throws AccountAddressException {
    if (input.length == 0) {
      return IH58_ALPHABET[0];
    }
    BigInteger value = new BigInteger(1, input);
    final StringBuilder builder = new StringBuilder();
    while (value.compareTo(BigInteger.ZERO) > 0) {
      final BigInteger[] divRem = value.divideAndRemainder(BASE_58);
      builder.append(IH58_ALPHABET[divRem[1].intValue()]);
      value = divRem[0];
    }
    final String zeroSymbol = IH58_ALPHABET[0];
    for (int i = 0; i < input.length && input[i] == 0; i++) {
      builder.append(zeroSymbol);
    }
    if (builder.length() == 0) {
      builder.append(zeroSymbol);
    }
    return builder.reverse().toString();
  }

  private static byte[] decodeBase58(final String encoded) throws AccountAddressException {
    if (encoded == null || encoded.isEmpty()) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid IH58 payload length");
    }
    BigInteger value = BigInteger.ZERO;
    for (int index = 0; index < encoded.length(); index++) {
      final String symbol = String.valueOf(encoded.charAt(index));
      final Integer digit = IH58_INDEX.get(symbol);
      if (digit == null) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_IH58_ENCODING, "invalid IH58 base58 encoding");
      }
      value = value.multiply(BASE_58).add(BigInteger.valueOf(digit));
    }
    byte[] decoded = value.toByteArray();
    if (decoded.length > 0 && decoded[0] == 0) {
      decoded = Arrays.copyOfRange(decoded, 1, decoded.length);
    }
    final char zeroChar = IH58_ALPHABET[0].charAt(0);
    int leadingZeros = 0;
    while (leadingZeros < encoded.length() && encoded.charAt(leadingZeros) == zeroChar) {
      leadingZeros++;
    }
    final byte[] result = new byte[leadingZeros + decoded.length];
    System.arraycopy(decoded, 0, result, leadingZeros, decoded.length);
    return result;
  }

  private static byte[] decodeBaseN(final int[] digits, final int base) throws AccountAddressException {
    if (base < 2) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for decoding");
    }
    if (digits.length == 0) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    for (final int digit : digits) {
      if (digit < 0 || digit >= base) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_COMPRESSED_DIGIT, "invalid digit " + digit + " for base " + base);
      }
    }
    final int[] value = Arrays.copyOf(digits, digits.length);
    int leadingZeros = 0;
    while (leadingZeros < value.length && value[leadingZeros] == 0) {
      leadingZeros++;
    }
    final List<Byte> bytes = new ArrayList<>();
    int start = leadingZeros;
    while (start < value.length) {
      int remainder = 0;
      for (int i = start; i < value.length; i++) {
        final int acc = remainder * base + value[i];
        value[i] = acc / 256;
        remainder = acc % 256;
      }
      bytes.add((byte) remainder);
      while (start < value.length && value[start] == 0) {
        start++;
      }
    }
    for (int i = 0; i < leadingZeros; i++) {
      bytes.add((byte) 0);
    }
    if (bytes.isEmpty()) {
      bytes.add((byte) 0);
    }
    Collections.reverse(bytes);
    final byte[] result = new byte[bytes.size()];
    for (int i = 0; i < bytes.size(); i++) {
      result[i] = bytes.get(i);
    }
    return result;
  }

  private static int[] compressedChecksumDigits(final byte[] canonical) {
    final int[] values = convertToBase32(canonical);
    final int[] expanded = expandHrp("snx", values, COMPRESSED_CHECKSUM_LEN);
    final int polymod = bech32Polymod(expanded) ^ BECH32M_CONST;
    final int[] checksum = new int[COMPRESSED_CHECKSUM_LEN];
    for (int i = 0; i < COMPRESSED_CHECKSUM_LEN; i++) {
      final int shift = 5 * (COMPRESSED_CHECKSUM_LEN - 1 - i);
      checksum[i] = (polymod >> shift) & 0x1F;
    }
    return checksum;
  }

  private static int[] convertToBase32(final byte[] data) {
    int acc = 0;
    int bits = 0;
    final List<Integer> out = new ArrayList<>();
    for (final byte b : data) {
      acc = (acc << 8) | (b & 0xFF);
      bits += 8;
      while (bits >= 5) {
        bits -= 5;
        out.add((acc >> bits) & 0x1F);
      }
    }
    if (bits > 0) {
      out.add((acc << (5 - bits)) & 0x1F);
    }
    final int[] result = new int[out.size()];
    for (int i = 0; i < out.size(); i++) {
      result[i] = out.get(i);
    }
    return result;
  }

  private static int[] expandHrp(final String hrp, final int[] data, final int checksumLen) {
    final int[] values = new int[hrp.length() * 2 + 1 + data.length + checksumLen];
    int idx = 0;
    for (int i = 0; i < hrp.length(); i++) {
      final int code = hrp.charAt(i);
      values[idx++] = code >> 5;
    }
    values[idx++] = 0;
    for (int i = 0; i < hrp.length(); i++) {
      values[idx++] = hrp.charAt(i) & 0x1F;
    }
    System.arraycopy(data, 0, values, idx, data.length);
    return values;
  }

  private static int bech32Polymod(final int[] values) {
    final int[] generators = { 0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3 };
    int chk = 1;
    for (final int value : values) {
      final int top = chk >> 25;
      chk = ((chk & 0x1ffffff) << 5) ^ value;
      for (int i = 0; i < generators.length; i++) {
        if (((top >> i) & 1) == 1) {
          chk ^= generators[i];
        }
      }
    }
    return chk;
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      sb.append(String.format("%02x", b & 0xFF));
    }
    return sb.toString();
  }

  private static byte[] hexToBytes(final String hex) throws AccountAddressException {
    if ((hex.length() & 1) == 1) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_HEX_ADDRESS, "hex string must have even length");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < hex.length(); i += 2) {
      try {
        out[i / 2] = (byte) Integer.parseInt(hex.substring(i, i + 2), 16);
      } catch (final NumberFormatException ex) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_HEX_ADDRESS, "invalid hex string");
      }
    }
    return out;
  }

  public static final class CurveSupportConfig {
    public final boolean allowMlDsa;
    public final boolean allowGost;
    public final boolean allowSm2;

    private CurveSupportConfig(final boolean allowMlDsa, final boolean allowGost, final boolean allowSm2) {
      this.allowMlDsa = allowMlDsa;
      this.allowGost = allowGost;
      this.allowSm2 = allowSm2;
    }

    public static CurveSupportConfig ed25519Only() {
      return new CurveSupportConfig(false, false, false);
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private boolean allowMlDsa;
      private boolean allowGost;
      private boolean allowSm2;

      public Builder allowMlDsa(final boolean value) {
        this.allowMlDsa = value;
        return this;
      }

      public Builder allowGost(final boolean value) {
        this.allowGost = value;
        return this;
      }

      public Builder allowSm2(final boolean value) {
        this.allowSm2 = value;
        return this;
      }

      public CurveSupportConfig build() {
        return new CurveSupportConfig(allowMlDsa, allowGost, allowSm2);
      }
    }
  }
}
