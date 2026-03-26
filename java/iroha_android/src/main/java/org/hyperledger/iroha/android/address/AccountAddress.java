package org.hyperledger.iroha.android.address;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Optional;

import org.hyperledger.iroha.android.crypto.Blake2s;

public final class AccountAddress {
  public static final int DEFAULT_I105_DISCRIMINANT = 753;

  private static final byte[] LOCAL_DOMAIN_KEY = "SORA-LOCAL-K:v1".getBytes(StandardCharsets.UTF_8);
  private static final int I105_DISCRIMINANT_MAX = 0xFFFF;
  private static final int I105_CHECKSUM_LEN = 6;
  private static final int BECH32M_CONST = 0x2bc830a3;
  private static final String I105_SENTINEL_SORA = "sora";
  private static final String I105_SENTINEL_TEST = "test";
  private static final String I105_SENTINEL_DEV = "dev";
  private static final String I105_SENTINEL_NUMERIC_PREFIX = "n";
  private static final String I105_SENTINEL_SORA_FULLWIDTH = "ｓｏｒａ";
  private static final String I105_SENTINEL_TEST_FULLWIDTH = "ｔｅｓｔ";
  private static final String I105_SENTINEL_DEV_FULLWIDTH = "ｄｅｖ";
  private static final String I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH = "ｎ";
  private static final String I105_WARNING =
      "i105 addresses use the canonical I105 alphabet: Base58 plus the 47 katakana from the Iroha poem. "
          + "Render and validate them with the intended chain discriminant.";

  private static final String[] BASE58_ALPHABET = {
      "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H", "J",
      "K", "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b", "c",
      "d", "e", "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v",
      "w", "x", "y", "z"
  };
  private static final String[] IROHA_POEM_KANA_FULLWIDTH = {
      "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ",
      "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ",
      "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス"
  };
  private static final String[] IROHA_POEM_KANA_HALFWIDTH = {
      "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ",
      "ﾚ", "ｿ", "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ",
      "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ"
  };

  private static final String[] I105_ALPHABET;
  private static final int I105_BASE;

  private static volatile boolean allowMlDsa;
  private static volatile boolean allowGost;
  private static volatile boolean allowSm2;

  static {
    configureCurveSupport(CurveSupportConfig.ed25519Only());
    I105_ALPHABET = new String[BASE58_ALPHABET.length + IROHA_POEM_KANA_FULLWIDTH.length];
    System.arraycopy(BASE58_ALPHABET, 0, I105_ALPHABET, 0, BASE58_ALPHABET.length);
    System.arraycopy(
        IROHA_POEM_KANA_FULLWIDTH,
        0,
        I105_ALPHABET,
        BASE58_ALPHABET.length,
        IROHA_POEM_KANA_FULLWIDTH.length);
    I105_BASE = I105_ALPHABET.length;
  }

  private final byte[] canonicalBytes;

  private AccountAddress(final byte[] canonicalBytes) {
    this.canonicalBytes = canonicalBytes;
  }

  public byte[] canonicalBytes() {
    return Arrays.copyOf(canonicalBytes, canonicalBytes.length);
  }

  public String canonicalHex() {
    return "0x" + bytesToHex(canonicalBytes);
  }

  public String toI105(final int discriminant) throws AccountAddressException {
    return encodeI105(canonicalBytes, discriminant);
  }

  /**
   * Convenience helper that surfaces canonical I105 alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats() throws AccountAddressException {
    return displayFormats(DEFAULT_I105_DISCRIMINANT);
  }

  /**
   * Convenience helper that surfaces canonical I105 alongside the shared warning string.
   * Follow {@code docs/source/sns/address_display_guidelines.md} when presenting these values.
   */
  public DisplayFormats displayFormats(final int discriminant) throws AccountAddressException {
    final String i105 = toI105(discriminant);
    return new DisplayFormats(i105, discriminant, I105_WARNING);
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

  public static String i105WarningMessage() {
    return I105_WARNING;
  }

  public static AccountAddress fromAccount(
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
   * Constructs a multisig account address from the provided policy payload.
   */
  public static AccountAddress fromMultisigPolicy(final MultisigPolicyPayload policy)
      throws AccountAddressException {
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
    final AccountAddress address = fromCanonicalBytes(canonical);
    ensureCanonicalI105Literal(encoded.trim(), address);
    return address;
  }

  public static ParseResult parseAny(final String input, final Integer expectedPrefix)
      throws AccountAddressException {
    final String trimmed = input.trim();
    if (trimmed.isEmpty()) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "address string is empty");
    }
    if (trimmed.contains("@")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "account address literals must not include @domain; use canonical Katakana i105 form");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "canonical hex account addresses are not accepted; use canonical Katakana i105 form");
    }
    return new ParseResult(fromI105(trimmed, expectedPrefix), Format.I105);
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
    if (trimmed.contains("@")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "account address literals must not include @domain; use canonical Katakana i105 form");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "canonical hex account addresses are not accepted; use canonical Katakana i105 form");
    }
    return new ParseResult(fromI105(trimmed, expectedPrefix), Format.I105);
  }

  public static Integer detectI105Discriminant(final String input) {
    if (input == null) {
      return null;
    }
    final I105SentinelPayload parsed;
    try {
      parsed = parseI105SentinelAndPayload(input.trim());
    } catch (final AccountAddressException ignored) {
      return null;
    }
    return parsed == null ? null : parsed.discriminant;
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
    if (trimmed.contains("@")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "account address literals must not include @domain; use canonical Katakana i105 form");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "canonical hex account addresses are not accepted; use canonical Katakana i105 form");
    }
    final byte[] canonical = decodeI105(trimmed, expectedPrefix);
    parseCanonical(canonical, true);
    final AccountAddress address = new AccountAddress(canonical);
    ensureCanonicalI105Literal(trimmed, address);
    return new ParseResult(address, Format.I105);
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
    INVALID_I105_PREFIX("ERR_INVALID_I105_PREFIX"),
    INVALID_LENGTH("ERR_INVALID_LENGTH"),
    CHECKSUM_MISMATCH("ERR_CHECKSUM_MISMATCH"),
    INVALID_HEX_ADDRESS("ERR_INVALID_HEX_ADDRESS"),
    UNEXPECTED_NETWORK_PREFIX("ERR_UNEXPECTED_NETWORK_PREFIX"),
    UNKNOWN_ADDRESS_CLASS("ERR_UNKNOWN_ADDRESS_CLASS"),
    UNKNOWN_DOMAIN_TAG("ERR_UNKNOWN_DOMAIN_TAG"),
    UNEXPECTED_EXTENSION_FLAG("ERR_UNEXPECTED_EXTENSION_FLAG"),
    UNKNOWN_CONTROLLER_TAG("ERR_UNKNOWN_CONTROLLER_TAG"),
    UNKNOWN_CURVE("ERR_UNKNOWN_CURVE"),
    MISSING_I105_SENTINEL("ERR_MISSING_I105_SENTINEL"),
    INVALID_I105_BASE("ERR_INVALID_I105_BASE"),
    INVALID_I105_DIGIT("ERR_INVALID_I105_DIGIT"),
    INVALID_I105_CHAR("ERR_INVALID_I105_CHAR"),
    I105_TOO_SHORT("ERR_I105_TOO_SHORT"),
    UNEXPECTED_TRAILING_BYTES("ERR_UNEXPECTED_TRAILING_BYTES"),
    MULTISIG_MEMBER_OVERFLOW("ERR_MULTISIG_MEMBER_OVERFLOW"),
    INVALID_MULTISIG_POLICY("ERR_INVALID_MULTISIG_POLICY"),
    UNSUPPORTED_ADDRESS_FORMAT("ERR_UNSUPPORTED_ADDRESS_FORMAT");

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
    public final int discriminant;
    public final String i105Warning;

    private DisplayFormats(
        final String i105,
        final int discriminant,
        final String i105Warning) {
      this.i105 = i105;
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

  private static String encodeI105(final byte[] canonical, final int discriminant)
      throws AccountAddressException {
    final int normalizedDiscriminant = normalizeI105Discriminant(discriminant, "i105 discriminant");
    final int[] digits = encodeBaseN(canonical, I105_BASE);
    final int[] checksum = i105ChecksumDigits(canonical);
    final StringBuilder sb = new StringBuilder();
    sb.append(i105SentinelForDiscriminant(normalizedDiscriminant));
    for (final int digit : digits) {
      sb.append(I105_ALPHABET[digit]);
    }
    for (final int digit : checksum) {
      sb.append(I105_ALPHABET[digit]);
    }
    return sb.toString();
  }

  private static byte[] decodeI105(final String encoded, final Integer expectedDiscriminant)
      throws AccountAddressException {
    final I105SentinelPayload parsed = parseI105SentinelAndPayload(encoded);
    if (parsed == null) {
      throw new AccountAddressException(
          AccountAddressErrorCode.MISSING_I105_SENTINEL,
          "i105 address is missing the expected chain-discriminant sentinel");
    }
    if (expectedDiscriminant != null) {
      final int normalizedExpected =
          normalizeI105Discriminant(expectedDiscriminant.intValue(), "expected i105 discriminant");
      if (parsed.discriminant != normalizedExpected) {
        throw new AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
            "unexpected i105 discriminant: expected "
                + normalizedExpected
                + ", found "
                + parsed.discriminant);
      }
    }
    return decodeI105Payload(parsed.payload);
  }

  private static byte[] decodeI105Payload(final String payload) throws AccountAddressException {
    final List<Integer> digits = new ArrayList<>();
    final DecodeAttemptState state = new DecodeAttemptState();
    final byte[] canonical = backtrackI105Payload(payload, 0, digits, state);
    if (canonical != null) {
      return canonical;
    }
    if (state.sawChecksumMismatch) {
      throw new AccountAddressException(
          AccountAddressErrorCode.CHECKSUM_MISMATCH, "i105 checksum mismatch");
    }
    if (state.sawTooShort) {
      throw new AccountAddressException(
          AccountAddressErrorCode.I105_TOO_SHORT, "i105 address is too short");
    }
    if (state.invalidChar != null) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_I105_CHAR,
          "invalid i105 alphabet symbol: " + state.invalidChar.charValue());
    }
    throw new AccountAddressException(
        AccountAddressErrorCode.CHECKSUM_MISMATCH, "i105 checksum mismatch");
  }

  private static byte[] backtrackI105Payload(
      final String payload,
      final int index,
      final List<Integer> digits,
      final DecodeAttemptState state)
      throws AccountAddressException {
    if (index == payload.length()) {
      if (digits.size() <= I105_CHECKSUM_LEN) {
        state.sawTooShort = true;
        return null;
      }
      final int splitAt = digits.size() - I105_CHECKSUM_LEN;
      final int[] dataDigits = new int[splitAt];
      for (int i = 0; i < splitAt; i++) {
        dataDigits[i] = digits.get(i);
      }
      final int[] checksumDigits = new int[I105_CHECKSUM_LEN];
      for (int i = 0; i < I105_CHECKSUM_LEN; i++) {
        checksumDigits[i] = digits.get(splitAt + i);
      }
      final byte[] canonical = decodeBaseN(dataDigits, I105_BASE);
      if (Arrays.equals(checksumDigits, i105ChecksumDigits(canonical))) {
        return canonical;
      }
      state.sawChecksumMismatch = true;
      return null;
    }

    final char currentChar = payload.charAt(index);
    boolean matched = false;
    for (int symbolLength = I105_MAX_SYMBOL_CHARS; symbolLength >= 1; symbolLength--) {
      if (index + symbolLength > payload.length()) {
        continue;
      }
      final String candidate = payload.substring(index, index + symbolLength);
      final Integer digit = lookupI105Digit(candidate);
      if (digit == null) {
        continue;
      }
      matched = true;
      digits.add(digit);
      final byte[] canonical = backtrackI105Payload(payload, index + symbolLength, digits, state);
      digits.remove(digits.size() - 1);
      if (canonical != null) {
        return canonical;
      }
    }

    if (!matched && state.invalidChar == null) {
      state.invalidChar = Character.valueOf(currentChar);
    }
    return null;
  }

  private static final class DecodeAttemptState {
    private boolean sawTooShort;
    private boolean sawChecksumMismatch;
    private Character invalidChar;
  }

  private static Integer lookupI105Digit(final String symbol) {
    for (int i = 0; i < I105_ALPHABET.length; i++) {
      if (I105_ALPHABET[i].equals(symbol)) {
        return Integer.valueOf(i);
      }
    }
    return null;
  }

  private static int normalizeI105Discriminant(final int discriminant, final String context)
      throws AccountAddressException {
    if (discriminant < 0 || discriminant > I105_DISCRIMINANT_MAX) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_I105_PREFIX,
          context + " out of range: " + discriminant);
    }
    return discriminant;
  }

  private static String i105SentinelForDiscriminant(final int discriminant) {
    switch (discriminant) {
      case DEFAULT_I105_DISCRIMINANT:
        return I105_SENTINEL_SORA;
      case 0x0171:
        return I105_SENTINEL_TEST;
      case 0x0000:
        return I105_SENTINEL_DEV;
      default:
        return I105_SENTINEL_NUMERIC_PREFIX + discriminant;
    }
  }

  private static I105SentinelPayload parseI105SentinelAndPayload(final String encoded)
      throws AccountAddressException {
    if (encoded.startsWith(I105_SENTINEL_SORA) || encoded.startsWith(I105_SENTINEL_SORA_FULLWIDTH)) {
      return new I105SentinelPayload(
          DEFAULT_I105_DISCRIMINANT,
          encoded.substring(I105_SENTINEL_SORA.length()));
    }
    if (encoded.startsWith(I105_SENTINEL_TEST) || encoded.startsWith(I105_SENTINEL_TEST_FULLWIDTH)) {
      return new I105SentinelPayload(0x0171, encoded.substring(I105_SENTINEL_TEST.length()));
    }
    if (encoded.startsWith(I105_SENTINEL_DEV) || encoded.startsWith(I105_SENTINEL_DEV_FULLWIDTH)) {
      return new I105SentinelPayload(0x0000, encoded.substring(I105_SENTINEL_DEV.length()));
    }
    if (!encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX)
        && !encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH)) {
      return null;
    }
    final String tail = encoded.substring(I105_SENTINEL_NUMERIC_PREFIX.length());
    final StringBuilder digits = new StringBuilder();
    int index = 0;
    while (index < tail.length()) {
      final Character asciiDigit = toAsciiDigit(tail.charAt(index));
      if (asciiDigit == null) {
        break;
      }
      digits.append(asciiDigit.charValue());
      index++;
    }
    if (digits.length() == 0) {
      return null;
    }
    final int discriminant;
    try {
      discriminant = Integer.parseInt(digits.toString());
    } catch (final NumberFormatException ex) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_I105_PREFIX,
          "invalid i105 discriminant sentinel: " + encoded);
    }
    return new I105SentinelPayload(
        normalizeI105Discriminant(discriminant, "i105 discriminant"),
        tail.substring(index));
  }

  private static Character toAsciiDigit(final char value) {
    if (value >= '0' && value <= '9') {
      return value;
    }
    if (value >= '０' && value <= '９') {
      return (char) (value - '０' + '0');
    }
    return null;
  }

  private static int[] encodeBaseN(final byte[] input, final int base) throws AccountAddressException {
    if (base < 2) {
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_I105_BASE,
          "invalid base for encoding");
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
      throw new AccountAddressException(
          AccountAddressErrorCode.INVALID_I105_BASE,
          "invalid base for decoding");
    }
    if (digits.length == 0) {
      throw new AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload");
    }
    for (final int digit : digits) {
      if (digit < 0 || digit >= base) {
        throw new AccountAddressException(
            AccountAddressErrorCode.INVALID_I105_DIGIT,
            "invalid digit " + digit + " for base " + base);
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

  private static int[] convertToBase32(final byte[] data) {
    int acc = 0;
    int bits = 0;
    final List<Integer> out = new ArrayList<>();
    for (final byte datum : data) {
      acc = (acc << 8) | (datum & 0xFF);
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

  private static int bech32Polymod(final int[] values) {
    final int[] generators = {
        0x3b6a57b2,
        0x26508e6d,
        0x1ea119fa,
        0x3d4233dd,
        0x2a1462b3
    };
    int chk = 1;
    for (final int value : values) {
      final int top = chk >>> 25;
      chk = ((chk & 0x1ff_ffff) << 5) ^ value;
      for (int i = 0; i < generators.length; i++) {
        if (((top >>> i) & 1) == 1) {
          chk ^= generators[i];
        }
      }
    }
    return chk;
  }

  private static int[] expandHrp(final String hrp) {
    final int[] result = new int[hrp.length() * 2 + 1];
    int cursor = 0;
    for (int i = 0; i < hrp.length(); i++) {
      result[cursor++] = hrp.charAt(i) >>> 5;
    }
    result[cursor++] = 0;
    for (int i = 0; i < hrp.length(); i++) {
      result[cursor++] = hrp.charAt(i) & 0x1F;
    }
    return result;
  }

  private static int[] bech32mChecksum(final int[] data) {
    final int[] hrp = expandHrp("snx");
    final int[] values = new int[hrp.length + data.length + I105_CHECKSUM_LEN];
    System.arraycopy(hrp, 0, values, 0, hrp.length);
    System.arraycopy(data, 0, values, hrp.length, data.length);
    final int polymod = bech32Polymod(values) ^ BECH32M_CONST;
    final int[] result = new int[I105_CHECKSUM_LEN];
    for (int i = 0; i < I105_CHECKSUM_LEN; i++) {
      result[i] = (polymod >>> (5 * (I105_CHECKSUM_LEN - 1 - i))) & 0x1F;
    }
    return result;
  }

  private static int[] i105ChecksumDigits(final byte[] canonical) {
    return bech32mChecksum(convertToBase32(canonical));
  }

  private static final class I105SentinelPayload {
    private final int discriminant;
    private final String payload;

    private I105SentinelPayload(final int discriminant, final String payload) {
      this.discriminant = discriminant;
      this.payload = payload;
    }
  }

  private static void ensureCanonicalI105Literal(
      final String literal, final AccountAddress address) throws AccountAddressException {
    final Integer discriminant = detectI105Discriminant(literal);
    if (discriminant == null) {
      return;
    }
    final String canonical = address.toI105(discriminant.intValue());
    if (!canonical.equals(literal)) {
      throw new AccountAddressException(
          AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          "account address literals must use canonical katakana i105 form");
    }
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
