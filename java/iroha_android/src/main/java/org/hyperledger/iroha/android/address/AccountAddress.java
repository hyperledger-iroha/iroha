package org.hyperledger.iroha.android.address;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.crypto.Blake2s;

public final class AccountAddress {

  public static final String DEFAULT_DOMAIN_NAME = "default";
  public static final int DEFAULT_I105_DISCRIMINANT = 753;

  private static final byte[] LOCAL_DOMAIN_KEY = "SORA-LOCAL-K:v1".getBytes(StandardCharsets.UTF_8);
  private static final String COMPRESSED_SENTINEL = "sora";
  private static final int COMPRESSED_CHECKSUM_LEN = 6;
  private static final int BECH32M_CONST = 0x2bc830a3;
  private static final String COMPRESSED_WARNING =
      "Canonical I105 addresses rely on the Sora sentinel and are the only supported public "
          + "account literal form. Use the full-width rendering only when your surface requires it.";

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
  private static final int COMPRESSED_BASE;

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

  }

  private final byte[] canonicalBytes;

  private AccountAddress(final byte[] canonicalBytes) {
    this.canonicalBytes = canonicalBytes;
  }

  public byte[] canonicalBytes() {
    return Arrays.copyOf(canonicalBytes, canonicalBytes.length);
  }

  /** Canonical payloads are domainless, so rebasing is a no-op. */
  public AccountAddress rebasedFromDefaultDomain(final String domainLabel) throws AccountAddressException {
    Objects.requireNonNull(domainLabel, "domainLabel must not be null");
    parseCanonical(canonicalBytes);
    return this;
  }

  public String canonicalHex() {
    return "0x" + bytesToHex(canonicalBytes);
  }

  public String toI105(final int discriminant) throws AccountAddressException {
    return encodeI105(canonicalBytes, discriminant, COMPRESSED_ALPHABET);
  }

  public String toI105Default() throws AccountAddressException {
    return toI105(DEFAULT_I105_DISCRIMINANT);
  }

  public String toI105FullWidth() throws AccountAddressException {
    return toI105FullWidth(DEFAULT_I105_DISCRIMINANT);
  }

  public String toI105FullWidth(final int discriminant) throws AccountAddressException {
    return encodeI105(canonicalBytes, discriminant, COMPRESSED_ALPHABET_FULLWIDTH);
  }

  /**
   * Convenience helper that surfaces the IH58 (preferred)/sora (second-best) pair alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats() throws AccountAddressException {
    return displayFormats(DEFAULT_I105_DISCRIMINANT);
  }

  /**
   * Convenience helper that surfaces the IH58 (preferred)/sora (second-best) pair alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats(final int discriminant) throws AccountAddressException {
    final String i105 = toI105(discriminant);
    final String i105FullWidth = toI105FullWidth(discriminant);
    return new DisplayFormats(i105, i105FullWidth, discriminant, COMPRESSED_WARNING);
  }

  /**
   * Returns the single-key controller payload when this address encodes a single-key controller.
   *
   * <p>Multisig addresses return {@link Optional#empty()}.
   */
  public Optional<SingleKeyPayload> singleKeyPayload() throws AccountAddressException {
    parseCanonical(canonicalBytes);
    return extractSingleKeyPayload(canonicalBytes, false);
  }

  /**
   * Returns the single-key controller payload without rejecting disabled-but-known curves.
   */
  public Optional<SingleKeyPayload> singleKeyPayloadIgnoringCurveSupport()
      throws AccountAddressException {
    parseCanonical(canonicalBytes, true);
    return extractSingleKeyPayload(canonicalBytes, true);
  }

  /**
   * Returns the multisig policy payload when this address encodes a multisig controller.
   *
   * <p>Single-key addresses return {@link Optional#empty()}.
   */
  public Optional<MultisigPolicyPayload> multisigPolicyPayload() throws AccountAddressException {
    parseCanonical(canonicalBytes);
    return extractMultisigPayload(canonicalBytes, false);
  }

  /**
   * Returns the multisig policy payload without rejecting disabled-but-known curves.
   */
  public Optional<MultisigPolicyPayload> multisigPolicyPayloadIgnoringCurveSupport()
      throws AccountAddressException {
    parseCanonical(canonicalBytes, true);
    return extractMultisigPayload(canonicalBytes, true);
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

    out.write(0x00);
    out.write(curveIdForAlgorithm(algorithm));
    out.write(publicKey.length);
    out.write(publicKey, 0, publicKey.length);

    return fromCanonicalBytes(out.toByteArray());
  }

  /**
   * Backward-compatible overload that uses the default domain selector.
   */
  public static AccountAddress fromAccount(
      final byte[] publicKey, final String algorithm) throws AccountAddressException {
    return fromAccount(DEFAULT_DOMAIN_NAME, publicKey, algorithm);
  }

  /**
   * Constructs a multisig account address from the provided policy payload.
   */
  public static AccountAddress fromMultisigPolicy(final MultisigPolicyPayload policy)
      throws AccountAddressException {
    return fromMultisigPolicy(DEFAULT_DOMAIN_NAME, policy);
  }

  /**
   * Constructs a multisig account address from the provided policy payload.
   */
  public static AccountAddress fromMultisigPolicy(
      final String domain,
      final MultisigPolicyPayload policy) throws AccountAddressException {
    if (policy == null) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "multisig policy must not be null");
    }

    final List<MultisigMemberPayload> members = policy.members();
    if (members.isEmpty()) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: zero members");
    }
    if (members.size() > 0xFFFF) {
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

    out.write(0x01); // multisig controller tag
    out.write(policy.version() & 0xFF);
    out.write((policy.threshold() >> 8) & 0xFF);
    out.write(policy.threshold() & 0xFF);
    out.write((members.size() >> 8) & 0xFF);
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

  public static AccountAddress fromI105(final String encoded, final Integer expectedDiscriminant)
      throws AccountAddressException {
    final byte[] canonical = decodeI105(encoded, expectedDiscriminant);
    return fromCanonicalBytes(canonical);
  }

  public static ParseResult parseAny(final String input, final Integer expectedPrefix)
      throws AccountAddressException {
    final String trimmed = input.trim();
    if (trimmed.isEmpty()) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "address string is empty");
    }
    if (hasI105Sentinel(trimmed)) {
      return new ParseResult(fromI105(trimmed, expectedPrefix), Format.I105);
    }
    if (containsCompressedGlyph(trimmed)) {
      throw new AccountAddressException(
          AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL, "I105 address must start with a sentinel");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      return new ParseResult(fromCanonicalHex(trimmed), Format.CANONICAL_HEX);
    }
    throw new AccountAddressException(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format");
  }

  /**
   * Backward-compatible alias for the older encoded-address parsing name.
   */
  public static ParseResult parseEncoded(final String input, final Integer expectedPrefix)
      throws AccountAddressException {
    final String trimmed = input.trim();
    if (trimmed.isEmpty()) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_LENGTH, "address string is empty");
    }
    if (!hasI105Sentinel(trimmed)) {
      if (containsCompressedGlyph(trimmed)) {
        throw new AccountAddressException(
            AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
            "I105 address must start with a sentinel");
      }
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format");
    }
    return new ParseResult(fromI105(trimmed, expectedPrefix), Format.I105);
  }

  /**
   * Parses any supported encoded form while tolerating known curves that are currently disabled in
   * the runtime configuration.
   */
  public static ParseResult parseEncodedIgnoringCurveSupport(
      final String input, final Integer expectedPrefix) throws AccountAddressException {
    final String trimmed = input.trim();
    if (trimmed.isEmpty()) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_LENGTH, "address string is empty");
    }
    if (hasI105Sentinel(trimmed)) {
      final byte[] canonical = decodeI105(trimmed, expectedPrefix);
      parseCanonical(canonical, true);
      return new ParseResult(new AccountAddress(canonical), Format.I105);
    }
    if (containsCompressedGlyph(trimmed)) {
      throw new AccountAddressException(
          AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
          "I105 address must start with a sentinel");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      final String body =
          trimmed.startsWith("0x") || trimmed.startsWith("0X") ? trimmed.substring(2) : trimmed;
      final byte[] canonical = hexToBytes(body);
      parseCanonical(canonical, true);
      return new ParseResult(new AccountAddress(canonical), Format.CANONICAL_HEX);
    }
    throw new AccountAddressException(
        AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format");
  }

  private static boolean containsCompressedGlyph(final String value) {
    for (int i = 0; i < value.length(); i++) {
      final String symbol = String.valueOf(value.charAt(i));
      if (COMPRESSED_INDEX.containsKey(symbol) && !isAsciiAlphaNumeric(symbol.charAt(0))) {
        return true;
      }
    }
    return false;
  }

  private static boolean isAsciiAlphaNumeric(final char ch) {
    return (ch >= '0' && ch <= '9')
        || (ch >= 'A' && ch <= 'Z')
        || (ch >= 'a' && ch <= 'z');
  }

  public enum Format {
    I105,
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
    public final String i105;
    public final String i105FullWidth;
    public final int discriminant;
    public final String i105Warning;

    private DisplayFormats(
        final String i105,
        final String i105FullWidth,
        final int discriminant,
        final String i105Warning) {
      this.i105 = i105;
      this.i105FullWidth = i105FullWidth;
      this.discriminant = discriminant;
      this.i105Warning = i105Warning;
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
    parseCanonical(canonical, false);
  }

  private static void parseCanonical(final byte[] canonical, final boolean ignoreCurveSupport)
      throws AccountAddressException {
    if (canonical.length < 4) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte header = canonical[0];
    decodeHeader(header);
    int cursor = 1;

    if (cursor >= canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte controllerTag = canonical[cursor++];
    switch (controllerTag) {
      case 0x00: {
        if (cursor + 2 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        final int curveId = canonical[cursor++] & 0xFF;
        if (!ignoreCurveSupport) {
          ensureCurveEnabled(curveId, "curve id " + curveId);
        }
        final int keyLen = canonical[cursor++] & 0xFF;
        final int end = cursor + keyLen;
        if (end > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        if (end != canonical.length) {
          throw new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES, "unexpected trailing bytes in canonical payload");
        }
        break;
      }
      case 0x01: {
        if (cursor + 5 > canonical.length) {
          throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
        }
        cursor++; // version (currently unused, but enforced for length)
        final int threshold =
            ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
        cursor += 2;
        final int memberCount =
            ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
        cursor += 2;
        if (memberCount == 0) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: zero members");
        }
        long totalWeight = 0L;
        for (int i = 0; i < memberCount; i++) {
          if (cursor + 5 > canonical.length) {
            throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
          }
          final int curveId = canonical[cursor++] & 0xFF;
          if (!ignoreCurveSupport) {
            ensureCurveEnabled(curveId, "curve id " + curveId);
          }
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
          cursor += keyLen;
          totalWeight += weight;
        }
        if (threshold <= 0) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: ZeroThreshold");
        }
        if (totalWeight < threshold) {
          throw new AccountAddressException(
              AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: threshold exceeds total weight");
        }
        if (cursor != canonical.length) {
          if (cursor > canonical.length) {
            throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
          }
          throw new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES, "unexpected trailing bytes in canonical payload");
        }
        break;
      }
      default:
        throw new AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG, "unknown controller tag: " + controllerTag);
    }
  }

  private static Optional<SingleKeyPayload> extractSingleKeyPayload(
      final byte[] canonical, final boolean ignoreCurveSupport) throws AccountAddressException {
    if (canonical.length < 4) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    int cursor = 0;
    final byte header = canonical[cursor++];
    decodeHeader(header);

    if (cursor >= canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte controllerTag = canonical[cursor++];
    if (controllerTag != 0x00) {
      return Optional.empty();
    }
    if (cursor + 2 > canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final int curveId = canonical[cursor++] & 0xFF;
    if (!ignoreCurveSupport) {
      ensureCurveEnabled(curveId, "curve id " + curveId);
    }
    final int keyLen = canonical[cursor++] & 0xFF;
    final int end = cursor + keyLen;
    if (end > canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    if (end != canonical.length) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES, "unexpected trailing bytes in canonical payload");
    }
    final byte[] key = Arrays.copyOfRange(canonical, cursor, end);
    return Optional.of(new SingleKeyPayload(curveId, key));
  }

  private static Optional<MultisigPolicyPayload> extractMultisigPayload(
      final byte[] canonical, final boolean ignoreCurveSupport) throws AccountAddressException {
    if (canonical.length < 4) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    int cursor = 0;
    final byte header = canonical[cursor++];
    decodeHeader(header);

    if (cursor >= canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final byte controllerTag = canonical[cursor++];
    if (controllerTag != 0x01) {
      return Optional.empty();
    }
    if (cursor + 5 > canonical.length) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
    }
    final int version = canonical[cursor++] & 0xFF;
    final int threshold =
        ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
    cursor += 2;
    final int memberCount =
        ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
    cursor += 2;
    if (memberCount == 0) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_MULTISIG_POLICY, "InvalidMultisigPolicy: zero members");
    }

    final List<MultisigMemberPayload> members = new ArrayList<>(memberCount);
    for (int i = 0; i < memberCount; i++) {
      if (cursor + 5 > canonical.length) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
      }
      final int curveId = canonical[cursor++] & 0xFF;
      if (!ignoreCurveSupport) {
        ensureCurveEnabled(curveId, "curve id " + curveId);
      }
      final int weight =
          ((canonical[cursor] & 0xFF) << 8) | (canonical[cursor + 1] & 0xFF);
      cursor += 2;
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
    }
    if (cursor != canonical.length) {
      if (cursor > canonical.length) {
        throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length");
      }
      throw new AccountAddressException(
          AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES, "unexpected trailing bytes in canonical payload");
    }
    return Optional.of(new MultisigPolicyPayload(version, threshold, members));
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

  private static byte[] computeLocalDigest(final String label) {
    final byte[] digest = Blake2s.digest(label.getBytes(StandardCharsets.UTF_8), LOCAL_DOMAIN_KEY, 32);
    return Arrays.copyOf(digest, 12);
  }

  private static String encodeI105(
      final byte[] canonical, final int discriminant, final String[] alphabet)
      throws AccountAddressException {
    final int[] digits = encodeBaseN(canonical, COMPRESSED_BASE);
    final int[] checksum = compressedChecksumDigits(canonical);
    final StringBuilder sb = new StringBuilder();
    final String sentinel =
        alphabet == COMPRESSED_ALPHABET_FULLWIDTH
            ? i105SentinelFullWidth(discriminant)
            : i105Sentinel(discriminant);
    sb.append(sentinel);
    for (final int digit : digits) {
      sb.append(alphabet[digit]);
    }
    for (final int digit : checksum) {
      sb.append(alphabet[digit]);
    }
    return sb.toString();
  }

  private static byte[] decodeI105(final String encoded, final Integer expectedDiscriminant)
      throws AccountAddressException {
    final String payload;
    if (expectedDiscriminant != null) {
      payload = stripI105SentinelForDiscriminant(encoded, expectedDiscriminant.intValue());
      if (payload == null) {
        final Integer found = i105DiscriminantFromSentinel(encoded);
        if (found != null && found.intValue() != expectedDiscriminant.intValue()) {
          throw new AccountAddressException(
              AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
              "unexpected I105 discriminant: expected "
                  + expectedDiscriminant
                  + ", found "
                  + found);
        }
        throw new AccountAddressException(
            AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
            "I105 address must start with a sentinel");
      }
    } else {
      final SentinelMatch match = matchAnyI105Sentinel(encoded);
      if (match == null) {
        throw new AccountAddressException(
            AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
            "I105 address must start with a sentinel");
      }
      payload = match.payload;
    }
    if (payload.length() <= COMPRESSED_CHECKSUM_LEN) {
      throw new AccountAddressException(
          AccountAddressErrorCode.COMPRESSED_TOO_SHORT, "I105 address is too short");
    }
    final int[] digits = new int[payload.length()];
    for (int i = 0; i < payload.length(); i++) {
      final String symbol = String.valueOf(payload.charAt(i));
      final Integer value = COMPRESSED_INDEX.get(symbol);
      if (value == null) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_COMPRESSED_CHAR,
            "invalid I105 alphabet symbol: " + symbol);
      }
      digits[i] = value;
    }
    final int[] dataDigits = Arrays.copyOf(digits, digits.length - COMPRESSED_CHECKSUM_LEN);
    final int[] checksumDigits = Arrays.copyOfRange(digits, digits.length - COMPRESSED_CHECKSUM_LEN, digits.length);
    final byte[] canonical = decodeBaseN(dataDigits, COMPRESSED_BASE);
    final int[] expected = compressedChecksumDigits(canonical);
    if (!Arrays.equals(checksumDigits, expected)) {
      throw new AccountAddressException(
          AccountAddressErrorCode.CHECKSUM_MISMATCH, "I105 checksum mismatch");
    }
    return canonical;
  }

  private static boolean hasI105Sentinel(final String encoded) {
    return i105DiscriminantFromSentinel(encoded) != null;
  }

  private static String i105Sentinel(final int discriminant) {
    if (discriminant == DEFAULT_I105_DISCRIMINANT) {
      return COMPRESSED_SENTINEL;
    }
    if (discriminant == 369) {
      return "test";
    }
    if (discriminant == 0) {
      return "dev";
    }
    return "n" + discriminant;
  }

  private static String i105SentinelFullWidth(final int discriminant) {
    return asciiToFullWidth(i105Sentinel(discriminant));
  }

  private static String stripI105SentinelForDiscriminant(
      final String encoded, final int discriminant) {
    final String sentinel = i105Sentinel(discriminant);
    if (encoded.startsWith(sentinel)) {
      return encoded.substring(sentinel.length());
    }
    final String fullWidthSentinel = i105SentinelFullWidth(discriminant);
    if (encoded.startsWith(fullWidthSentinel)) {
      return encoded.substring(fullWidthSentinel.length());
    }
    return null;
  }

  private static Integer i105DiscriminantFromSentinel(final String encoded) {
    for (final int discriminant : new int[] {DEFAULT_I105_DISCRIMINANT, 369, 0}) {
      if (stripI105SentinelForDiscriminant(encoded, discriminant) != null) {
        return discriminant;
      }
    }
    if (encoded.startsWith("n")) {
      return parseNumericI105Sentinel(encoded.substring(1), false);
    }
    if (encoded.startsWith("ｎ")) {
      return parseNumericI105Sentinel(encoded.substring(1), true);
    }
    return null;
  }

  private static Integer parseNumericI105Sentinel(final String input, final boolean fullWidthDigits) {
    final StringBuilder digits = new StringBuilder();
    for (int i = 0; i < input.length() && digits.length() < 5; i++) {
      final char ch = input.charAt(i);
      final char asciiDigit;
      if (fullWidthDigits) {
        asciiDigit = fullWidthDigitToAscii(ch);
        if (asciiDigit == 0) {
          break;
        }
      } else {
        if (ch < '0' || ch > '9') {
          break;
        }
        asciiDigit = ch;
      }
      digits.append(asciiDigit);
    }
    if (digits.length() == 0) {
      return null;
    }
    return Integer.parseInt(digits.toString());
  }

  private static String asciiToFullWidth(final String ascii) {
    final StringBuilder builder = new StringBuilder(ascii.length());
    for (int i = 0; i < ascii.length(); i++) {
      builder.append(asciiCharToFullWidth(ascii.charAt(i)));
    }
    return builder.toString();
  }

  private static char asciiCharToFullWidth(final char ch) {
    if (ch == ' ') {
      return '\u3000';
    }
    if (ch >= 0x21 && ch <= 0x7E) {
      return (char) (ch + 0xFEE0);
    }
    return ch;
  }

  private static char fullWidthDigitToAscii(final char ch) {
    if (ch >= '０' && ch <= '９') {
      return (char) ('0' + (ch - '０'));
    }
    return 0;
  }

  private static final class SentinelMatch {
    final String payload;

    SentinelMatch(final String payload) {
      this.payload = payload;
    }
  }

  private static SentinelMatch matchAnyI105Sentinel(final String encoded) {
    final Integer discriminant = i105DiscriminantFromSentinel(encoded);
    if (discriminant == null) {
      return null;
    }
    final String payload = stripI105SentinelForDiscriminant(encoded, discriminant);
    if (payload == null) {
      return null;
    }
    return new SentinelMatch(payload);
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
