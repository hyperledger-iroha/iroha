package org.hyperledger.iroha.android.sccp;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.testing.TestAccountIds;

/** Shared-vector and adversarial tests for the closed exact SCCP V1 layout. */
public final class SccpV1Tests {
  private SccpV1Tests() {}

  public static void main(final String[] args) throws Exception {
    closedInventoryReservesRetiredTagsAndAliases();
    tonProfilesBindCanonicalZeroStates();
    allSharedEthBscTronTransferVectorsMatchRust();
    governedHashRotationPreservesReplayIdentityButChangesCommitment();
    payloadDecoderRejectsRetiredAndNoncanonicalForms();
    transferRejectsRetiredDomainsCodecsAndInvalidWidths();
    canonicalTextAcceptsExactI105AndRejectsUnicodeSubstitutions();
    contextAndCommitmentRejectZeroOrAliasedHashRoles();
    commitmentDecoderRejectsTamperingCollisionsAndTrailingBytes();
    payloadAndContextDefensivelyCopyBuffers();
    System.out.println("[IrohaAndroid] exact SCCP V1 tests passed.");
  }

  private static void closedInventoryReservesRetiredTagsAndAliases() {
    assert Arrays.stream(SccpNetworkV1.values()).map(SccpNetworkV1::tag).toList()
        .equals(List.of(1, 2, 3, 4, 5, 10, 11, 12, 14, 15));
    assert SccpNetworkV1.fromProfileKey("sora-taira") == SccpNetworkV1.SORA_TAIRA;
    assert SccpNetworkV1.SORA_TAIRA.isProduction();
    assert SccpNetworkV1.fromTag(0) == null;
    for (int tag = 6; tag <= 9; tag++) assert SccpNetworkV1.fromTag(tag) == null;
    for (final String alias :
        List.of(
            "sora-nexus",
            "sora_nexus",
            "solana-mainnet-beta",
            "SORA-TAIRA",
            "sora_taira",
            "sora-taira ",
            "bsc",
            "tron")) {
      assert SccpNetworkV1.fromProfileKey(alias) == null : alias;
    }
  }

  private static void tonProfilesBindCanonicalZeroStates() {
    assert SccpNetworkV1.fromProfileKey("ton-mainnet") == SccpNetworkV1.TON_MAINNET;
    assert SccpNetworkV1.fromProfileKey("ton-testnet") == SccpNetworkV1.TON_TESTNET;
    assert SccpNetworkV1.fromTag(14) == SccpNetworkV1.TON_MAINNET;
    assert SccpNetworkV1.fromTag(15) == SccpNetworkV1.TON_TESTNET;

    final byte[] mainnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET);
    final byte[] testnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_TESTNET);
    assert mainnet.length == 90;
    assert testnet.length == 90;
    assert (mainnet[0] & 0xff) == 1 && (mainnet[1] & 0xff) == 14;
    assert (testnet[0] & 0xff) == 1 && (testnet[1] & 0xff) == 15;
    assert Arrays.equals(Arrays.copyOfRange(mainnet, 2, 6), new byte[] {4, 0, 0, 0});
    assert Arrays.equals(Arrays.copyOfRange(testnet, 2, 6), new byte[] {4, 0, 0, 0});
    assert Arrays.equals(
        Arrays.copyOfRange(mainnet, 6, 10), new byte[] {0x11, (byte) 0xff, (byte) 0xff, (byte) 0xff});
    assert Arrays.equals(
        Arrays.copyOfRange(testnet, 6, 10), new byte[] {(byte) 0xfd, (byte) 0xff, (byte) 0xff, (byte) 0xff});
  }

  private static void allSharedEthBscTronTransferVectorsMatchRust() throws Exception {
    final Map<String, Object> fixture = fixture("native_transfer_event_v1.json");
    assert intValue(fixture, "version") == 1;
    for (final Object raw : list(fixture, "vectors")) {
      final Map<String, Object> vector = object(raw);
      final SccpLaneIdV1 lane =
          new SccpLaneIdV1(
              profile(string(vector, "source_profile")),
              profile(string(vector, "target_profile")));
      final byte[] payloadBytes =
          SccpV1.decodeLowerHex(string(vector, "canonical_payload_hex"));
      final SccpTransferPayloadV1 payload = SccpV1.decodeCanonicalPayload(payloadBytes);
      assert Arrays.equals(payloadBytes, payload.canonicalBytes());
      assert string(vector, "payload_hash_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.payloadHash(payload)));
      assert string(vector, "canonical_lane_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)));
      assert string(vector, "lane_hash_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.laneHash(lane)));
      assert string(vector, "message_id_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.messageId(lane, payload)));
      assert string(vector, "source_event_digest_hex")
          .equals(
              SccpV1.encodeLowerHex(
                  SccpV1.sourceEventDigest(
                      lane,
                      SccpV1.decodeLowerHex(string(vector, "message_id_hex")),
                      SccpV1.decodeLowerHex(string(vector, "payload_hash_hex")))));
    }
  }

  private static void governedHashRotationPreservesReplayIdentityButChangesCommitment() {
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET);
    final SccpTransferPayloadV1 payload = outboundPayload();
    final SccpHubCommitmentV1 base =
        SccpV1.commitment(new SccpOutboundMessageContextV1(lane, hash(0x21), hash(0x22)), payload);
    final SccpHubCommitmentV1 bindingRotation =
        SccpV1.commitment(new SccpOutboundMessageContextV1(lane, hash(0x23), hash(0x22)), payload);
    final SccpHubCommitmentV1 configurationRotation =
        SccpV1.commitment(new SccpOutboundMessageContextV1(lane, hash(0x21), hash(0x24)), payload);
    assert Arrays.equals(base.messageId(), bindingRotation.messageId());
    assert Arrays.equals(base.messageId(), configurationRotation.messageId());
    assert !Arrays.equals(SccpV1.commitmentRoot(base), SccpV1.commitmentRoot(bindingRotation));
    assert !Arrays.equals(SccpV1.commitmentRoot(base), SccpV1.commitmentRoot(configurationRotation));
    final SccpHubCommitmentV1 decoded =
        SccpV1.decodeCanonicalCommitment(SccpV1.canonicalCommitmentBytes(base));
    assert Arrays.equals(base.messageId(), decoded.messageId());
    assert Arrays.equals(hash(0x22), decoded.context().routeConfigurationHash());
  }

  private static void payloadDecoderRejectsRetiredAndNoncanonicalForms() {
    final byte[] canonical = outboundPayload().canonicalBytes();
    for (final int discriminant : List.of(0, 1, 3, 4, 5, 255)) {
      final byte[] hostile = canonical.clone();
      hostile[0] = (byte) discriminant;
      expectFailure(() -> SccpV1.decodeCanonicalPayload(hostile));
    }
    for (final int length : List.of(0, 1, canonical.length - 1)) {
      expectFailure(() -> SccpV1.decodeCanonicalPayload(Arrays.copyOf(canonical, length)));
    }
    expectFailure(() -> SccpV1.decodeCanonicalPayload(Arrays.copyOf(canonical, canonical.length + 1)));
    final byte[] zeroRevision = canonical.clone();
    Arrays.fill(zeroRevision, 18, 22, (byte) 0);
    expectFailure(() -> SccpV1.decodeCanonicalPayload(zeroRevision));
    final byte[] wrongVersion = canonical.clone();
    wrongVersion[1] = 2;
    expectFailure(() -> SccpV1.decodeCanonicalPayload(wrongVersion));
  }

  private static void transferRejectsRetiredDomainsCodecsAndInvalidWidths() {
    for (final int domain : List.of(3, 6, -1)) {
      expectFailure(
          () ->
              transfer(
                  domain,
                  0,
                  1,
                  1,
                  text("xor"),
                  BigInteger.ONE,
                  1,
                  text("alice"),
                  1,
                  text("bob"),
                  text("route")));
    }
    for (final int codec : List.of(3, 4, 6, 0, 255)) {
      expectFailure(
          () ->
              transfer(
                  0,
                  2,
                  1,
                  codec,
                  repeated(1, 32),
                  BigInteger.ONE,
                  1,
                  text("alice"),
                  2,
                  repeated(1, 20),
                  text("route")));
    }
    expectFailure(() -> transfer(0, 2, 0, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 2, repeated(1, 20), text("route")));
    expectFailure(() -> transfer(0, 2, 0x1_0000_0000L, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 2, repeated(1, 20), text("route")));
    expectFailure(() -> transfer(0, 2, 1, 1, text("xor"), BigInteger.ZERO, 1, text("alice"), 2, repeated(1, 20), text("route")));
    expectFailure(() -> transfer(0, 2, 1, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 2, repeated(1, 19), text("route")));
    expectFailure(() -> transfer(0, 2, 1, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 2, new byte[20], text("route")));
    expectFailure(() -> transfer(0, 2, 1, 1, text("contains space"), BigInteger.ONE, 1, text("alice"), 2, repeated(1, 20), text("route")));
    expectFailure(() -> transfer(0, 2, 1, 1, repeated('a', 257), BigInteger.ONE, 1, text("alice"), 2, repeated(1, 20), text("route")));
    final byte[] badTron = repeated(1, 21);
    badTron[0] = 0x42;
    expectFailure(() -> transfer(0, 5, 1, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 5, badTron, text("route")));

    final byte[] tonAccount = repeated(0x31, 36);
    Arrays.fill(tonAccount, 0, 4, (byte) 0);
    final SccpTransferPayloadV1 tonTransfer =
        transfer(
            0,
            4,
            1,
            1,
            text("xor"),
            BigInteger.ONE,
            1,
            text("alice"),
            7,
            tonAccount,
            text("taira_ton_xor"));
    assert Arrays.equals(tonAccount, tonTransfer.recipient());

    final byte[] nonBasechain = tonAccount.clone();
    nonBasechain[3] = 1;
    expectFailure(
        () ->
            transfer(
                0,
                4,
                1,
                1,
                text("xor"),
                BigInteger.ONE,
                1,
                text("alice"),
                7,
                nonBasechain,
                text("taira_ton_xor")));
    final byte[] zeroAccount = new byte[36];
    expectFailure(
        () ->
            transfer(
                0,
                4,
                1,
                1,
                text("xor"),
                BigInteger.ONE,
                1,
                text("alice"),
                7,
                zeroAccount,
                text("taira_ton_xor")));
  }

  private static void canonicalTextAcceptsExactI105AndRejectsUnicodeSubstitutions() {
    final String canonical = TestAccountIds.ed25519Authority(0x55);
    assert canonical.chars().anyMatch(value -> value > 0x7f)
        : "fixture must use non-ASCII I105 digits";
    final SccpTransferPayloadV1 accepted =
        transfer(
            1,
            0,
            1,
            1,
            text("xor"),
            BigInteger.ONE,
            2,
            repeated(1, 20),
            1,
            text(canonical),
            text("taira_eth_xor"));
    assert Arrays.equals(text(canonical), accepted.recipient());

    final String checksumAlias =
        canonical.substring(0, canonical.length() - 1)
            + (canonical.endsWith("1") ? "2" : "1");
    for (final byte[] invalid :
        List.of(
            text(checksumAlias),
            text("ｲ"),
            text("two words"),
            text("line\nbreak"),
            new byte[] {(byte) 0xff},
            repeated(0x21, 257))) {
      expectFailure(
          () ->
              transfer(
                  1,
                  0,
                  1,
                  1,
                  text("xor"),
                  BigInteger.ONE,
                  2,
                  repeated(1, 20),
                  1,
                  invalid,
                  text("taira_eth_xor")));
    }
  }

  private static void contextAndCommitmentRejectZeroOrAliasedHashRoles() {
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET);
    final SccpTransferPayloadV1 payload = outboundPayload();
    expectFailure(() -> new SccpOutboundMessageContextV1(lane, new byte[32], hash(2)));
    expectFailure(() -> new SccpOutboundMessageContextV1(lane, hash(1), hash(1)));
    for (final byte[] collision :
        List.of(SccpV1.laneHash(lane), SccpV1.messageId(lane, payload), SccpV1.payloadHash(payload))) {
      expectFailure(() -> SccpV1.commitment(new SccpOutboundMessageContextV1(lane, collision, hash(0x7f)), payload));
      expectFailure(() -> SccpV1.commitment(new SccpOutboundMessageContextV1(lane, hash(0x7e), collision), payload));
    }
  }

  private static void commitmentDecoderRejectsTamperingCollisionsAndTrailingBytes() {
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.TRON_NILE);
    final byte[] tron = repeated(4, 21);
    tron[0] = 0x41;
    final SccpTransferPayloadV1 payload =
        transfer(0, 5, 1, 1, text("xor"), BigInteger.ONE, 1, text("alice"), 5, tron, text("taira_tron_xor"));
    final byte[] encoded =
        SccpV1.canonicalCommitmentBytes(
            SccpV1.commitment(
                new SccpOutboundMessageContextV1(lane, hash(0x31), hash(0x32)), payload));
    assert encoded.length == 132;
    for (int offset = 0; offset <= 3; offset++) {
      final byte[] hostile = encoded.clone();
      hostile[offset] = 0x7f;
      expectFailure(() -> SccpV1.decodeCanonicalCommitment(hostile));
    }
    expectFailure(() -> SccpV1.decodeCanonicalCommitment(Arrays.copyOf(encoded, 133)));
    for (final int[] pair : List.of(new int[] {4, 36}, new int[] {4, 68}, new int[] {36, 68}, new int[] {68, 100})) {
      final byte[] collision = encoded.clone();
      System.arraycopy(encoded, pair[0], collision, pair[1], 32);
      expectFailure(() -> SccpV1.decodeCanonicalCommitment(collision));
    }
  }

  private static void payloadAndContextDefensivelyCopyBuffers() {
    final byte[] asset = text("xor");
    final byte[] binding = hash(0x41);
    final byte[] configuration = hash(0x42);
    final SccpTransferPayloadV1 payload =
        transfer(0, 2, 1, 1, asset, BigInteger.ONE, 1, text("alice"), 2, repeated(1, 20), text("route"));
    final SccpOutboundMessageContextV1 context =
        new SccpOutboundMessageContextV1(
            new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET),
            binding,
            configuration);
    Arrays.fill(asset, (byte) 0);
    Arrays.fill(binding, (byte) 0);
    Arrays.fill(configuration, (byte) 0);
    assert Arrays.equals(text("xor"), payload.assetId());
    assert !allZero(context.destinationBindingHash());
    assert !allZero(context.routeConfigurationHash());
    final byte[] exposed = context.destinationBindingHash();
    Arrays.fill(exposed, (byte) 0);
    assert !allZero(context.destinationBindingHash());
  }

  private static SccpTransferPayloadV1 outboundPayload() {
    return transfer(0, 2, 1, 1, text("xor"), BigInteger.ONE, 1, text("alice@taira"), 2, repeated(1, 20), text("taira_bsc_xor"));
  }

  private static SccpTransferPayloadV1 transfer(
      final int source,
      final int destination,
      final long routeRevision,
      final int assetCodec,
      final byte[] asset,
      final BigInteger amount,
      final int senderCodec,
      final byte[] sender,
      final int recipientCodec,
      final byte[] recipient,
      final byte[] route) {
    return new SccpTransferPayloadV1(
        source,
        destination,
        BigInteger.ONE,
        routeRevision,
        0,
        assetCodec,
        asset,
        amount,
        senderCodec,
        sender,
        recipientCodec,
        recipient,
        1,
        route);
  }

  private static byte[] text(final String value) {
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] hash(final int value) {
    return repeated(value, 32);
  }

  private static byte[] repeated(final int value, final int length) {
    final byte[] out = new byte[length];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) if (item != 0) return false;
    return true;
  }

  private static SccpNetworkV1 profile(final String value) {
    final SccpNetworkV1 result = SccpNetworkV1.fromProfileKey(value);
    if (result == null) throw new AssertionError(value);
    return result;
  }

  private static void expectFailure(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected rejection");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture(final String name) throws Exception {
    Path path = null;
    for (final String candidate :
        List.of(
            "fixtures/sccp/" + name,
            "../fixtures/sccp/" + name,
            "../../fixtures/sccp/" + name,
            "../../../fixtures/sccp/" + name)) {
      final Path resolved = Paths.get(candidate);
      if (Files.isRegularFile(resolved)) {
        path = resolved;
        break;
      }
    }
    if (path == null) throw new IllegalStateException("unable to locate fixture " + name);
    return (Map<String, Object>)
        JsonParser.parse(new String(Files.readAllBytes(path), StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Map<String, Object> value, final String key) {
    return (List<Object>) value.get(key);
  }

  private static String string(final Map<String, Object> value, final String key) {
    return (String) value.get(key);
  }

  private static int intValue(final Map<String, Object> value, final String key) {
    return ((Number) value.get(key)).intValue();
  }
}
