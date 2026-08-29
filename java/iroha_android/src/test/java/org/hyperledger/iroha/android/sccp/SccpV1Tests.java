package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.testing.TestAccountIds;

/** Shared-vector and adversarial tests for the closed exact SCCP V1 layout. */
public final class SccpV1Tests {
  private SccpV1Tests() {}

  public static void main(final String[] args) throws Exception {
    replayForestMatchesFinalV1CrossLanguageGolden();
    soraReplayPrincipalRequiresExactCanonicalAccountIdPayload();
    closedInventoryReservesRetiredTagsAndAliases();
    tonMainnetBindsCanonicalZeroState();
    allSharedMainnetTransferVectorsMatchRust();
    governedHashRotationPreservesReplayIdentityButChangesCommitment();
    payloadDecoderRejectsRetiredAndNoncanonicalForms();
    transferRejectsRetiredDomainsCodecsAndInvalidWidths();
    canonicalTextAcceptsExactI105AndRejectsUnicodeSubstitutions();
    contextAndCommitmentRejectZeroOrAliasedHashRoles();
    commitmentDecoderRejectsTamperingCollisionsAndTrailingBytes();
    payloadAndContextDefensivelyCopyBuffers();
    System.out.println("[IrohaAndroid] exact SCCP V1 tests passed.");
  }

  private static void replayForestMatchesFinalV1CrossLanguageGolden() {
    final SccpReplayV1.Boundary boundary = SccpReplayV1.Boundary.SORA_OUTBOUND_LOCK;
    final byte[] domain =
        SccpReplayV1.domainHash(
            SccpNetworkV1.SORA_TAIRA,
            SccpNetworkV1.ETHEREUM_MAINNET,
            boundary,
            7,
            hash(0x44),
            SccpReplayV1.Actor.route());
    assert SccpV1.encodeLowerHex(domain)
        .equals("de11cbd183f55063fe715fcf120773d799dfb1185e057f758c126306832fdc3d");
    final byte[] key = SccpReplayV1.replayKey(domain, hash(0x11));
    assert SccpV1.encodeLowerHex(key)
        .equals("139f57881d055a13ecf390d7441dadfc065ded40181c42a7aa3ab0a27469f17b");
    final byte[] record =
        SccpReplayV1.recordDigest(
            boundary,
            hash(0x11),
            hash(0x22),
            BigInteger.valueOf(9),
            SccpReplayV1.Principal.evm(repeated(0x33, 20)),
            hash(0x55));
    assert SccpV1.encodeLowerHex(record)
        .equals("31e4f2267d63d21101ab070e04aefe660df9681d3e12b263b61676e07c6f4aa5");
    final List<byte[]> empty = SccpReplayV1.emptyHashes();
    assert empty.size() == 249;
    assert SccpV1.encodeLowerHex(empty.get(248))
        .equals("cefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f");
    final byte[] zero = new byte[32];
    final SccpReplayV1.WitnessRoot nonMembership =
        SccpReplayV1.rootFromWitness(
            key, null, new SccpReplayV1.Witness(empty.get(248), zero, zero, List.of()));
    assert nonMembership.matchesExpectedRoot();
    assert nonMembership.shard() == 19;
    final byte[] occupied =
        SccpV1.decodeLowerHex(
            "d9c75ee102ec40076d903d6d5a0c3b0f9a9fa006ea9a2638274be11712ffb849");
    final SccpReplayV1.WitnessRoot membership =
        SccpReplayV1.rootFromWitness(
            key,
            record,
            new SccpReplayV1.Witness(occupied, record, zero, List.of()));
    assert membership.matchesExpectedRoot();
    assert Arrays.equals(occupied, membership.root());
    final byte[] reservedBitmap = new byte[32];
    reservedBitmap[0] = 1;
    expectFailure(
        () ->
            SccpReplayV1.rootFromWitness(
                key,
                null,
                new SccpReplayV1.Witness(
                    empty.get(248), zero, reservedBitmap, List.of(hash(0x77)))));
    final byte[] explicitDefaultBitmap = new byte[32];
    explicitDefaultBitmap[31] = 1;
    expectFailure(
        () ->
            SccpReplayV1.rootFromWitness(
                key,
                null,
                new SccpReplayV1.Witness(
                    empty.get(248), zero, explicitDefaultBitmap, List.of(empty.get(0)))));
    expectFailure(
        () ->
            SccpReplayV1.recordDigest(
                boundary,
                hash(0x11),
                hash(0x22),
                BigInteger.ONE.shiftLeft(128),
                SccpReplayV1.Principal.evm(repeated(0x33, 20)),
                hash(0x55)));
  }

  private static void soraReplayPrincipalRequiresExactCanonicalAccountIdPayload() {
    final String account = TestAccountIds.ed25519Authority(0x61);
    final byte[] canonical = TransferWirePayloadEncoder.encodeAccountIdPayload(account);
    SccpReplayV1.Principal.soraAccount(canonical);

    final byte[] nonCompact = nonCompactSingleAccountPayload(canonical);
    assert TransferWirePayloadEncoder.decodeAccountIdPayload(
            nonCompact, SccpV1.TAIRA_I105_DISCRIMINANT_V1, 0)
        .equals(
            TransferWirePayloadEncoder.decodeAccountIdPayload(
                canonical, SccpV1.TAIRA_I105_DISCRIMINANT_V1));
    final byte[] wrongController = canonical.clone();
    wrongController[0] = 2;
    final byte[] wrongAlgorithm = canonical.clone();
    wrongAlgorithm[14] = 0x7f;
    for (final byte[] invalid :
        List.of(
            new byte[0],
            new byte[] {0, 0, 0},
            nonCompact,
            wrongController,
            wrongAlgorithm,
            Arrays.copyOf(canonical, canonical.length + 1))) {
      expectFailure(() -> SccpReplayV1.Principal.soraAccount(invalid));
    }
  }

  private static void closedInventoryReservesRetiredTagsAndAliases() {
    assert Arrays.stream(SccpNetworkV1.values()).map(SccpNetworkV1::tag).toList()
        .equals(List.of(0x40, 0x41, 0x42, 0x43, 0x44));
    assert SccpNetworkV1.fromProfileKey("sora-taira") == SccpNetworkV1.SORA_TAIRA;
    assert SccpNetworkV1.SORA_TAIRA.isProduction();
    for (int tag = 0; tag <= 0xff; tag++) {
      if (tag < 0x40 || tag > 0x44) assert SccpNetworkV1.fromTag(tag) == null;
    }
    for (final String alias :
        List.of(
            "sora-nexus",
            "sora_nexus",
            "solana-mainnet-beta",
            "ethereum-sepolia",
            "bsc-testnet",
            "tron-nile",
            "tron-shasta",
            "ton-testnet",
            "solana-testnet",
            "SORA-TAIRA",
            "sora_taira",
            "sora-taira ",
            "bsc",
            "tron")) {
      assert SccpNetworkV1.fromProfileKey(alias) == null : alias;
    }
    assert SccpHubMessageKindV1.TRANSFER.tag() == 0;
    assert SccpHubMessageKindV1.fromTag(0) == SccpHubMessageKindV1.TRANSFER;
    assert SccpHubMessageKindV1.fromTag(5) == null;
  }

  private static void tonMainnetBindsCanonicalZeroState() {
    assert SccpNetworkV1.fromProfileKey("ton-mainnet") == SccpNetworkV1.TON_MAINNET;
    assert SccpNetworkV1.fromProfileKey("ton-testnet") == null;
    assert SccpNetworkV1.fromTag(0x44) == SccpNetworkV1.TON_MAINNET;
    assert SccpNetworkV1.fromTag(0x45) == null;

    final byte[] mainnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET);
    assert mainnet.length == 90;
    assert (mainnet[0] & 0xff) == 1 && (mainnet[1] & 0xff) == 0x44;
    assert Arrays.equals(Arrays.copyOfRange(mainnet, 2, 6), new byte[] {4, 0, 0, 0});
    assert Arrays.equals(
        Arrays.copyOfRange(mainnet, 6, 10), new byte[] {0x11, (byte) 0xff, (byte) 0xff, (byte) 0xff});
  }

  private static void allSharedMainnetTransferVectorsMatchRust() throws Exception {
    final Map<String, Object> fixture = fixture("native_transfer_event_v1.json");
    assert intValue(fixture, "version") == 1;
    int supported = 0;
    for (final Object raw : list(fixture, "vectors")) {
      final Map<String, Object> vector = object(raw);
      final SccpNetworkV1 source =
          SccpNetworkV1.fromProfileKey(string(vector, "source_profile"));
      final SccpNetworkV1 target =
          SccpNetworkV1.fromProfileKey(string(vector, "target_profile"));
      if (source == null || target == null) {
        throw new AssertionError("fixture contains a retired SCCP network profile");
      }
      supported++;
      final SccpLaneIdV1 lane =
          new SccpLaneIdV1(source, target);
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
    assert supported == 4;
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
    for (final int discriminant : List.of(1, 2, 3, 4, 5, 255)) {
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
    for (final int domain : List.of(5, 6, -1)) {
      expectFailure(
          () ->
              transfer(
                  domain,
                  0,
                  1,
                  0,
                  text("xor"),
                  BigInteger.ONE,
                  0,
                  text("alice"),
                  0,
                  text("bob"),
                  text("route")));
    }
    for (final int codec : List.of(4, 5, 6, 7, 255)) {
      expectFailure(
          () ->
              transfer(
                  0,
                  2,
                  1,
                  codec,
                  repeated(1, 32),
                  BigInteger.ONE,
                  0,
                  text("alice"),
                  1,
                  repeated(1, 20),
                  text("route")));
    }
    expectFailure(
        () ->
            transfer(
                0,
                2,
                0,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                repeated(1, 20),
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                0x1_0000_0000L,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                repeated(1, 20),
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                1,
                0,
                text("xor"),
                BigInteger.ZERO,
                0,
                text("alice"),
                1,
                repeated(1, 20),
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                1,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                repeated(1, 19),
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                1,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                new byte[20],
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                1,
                0,
                text("contains space"),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                repeated(1, 20),
                text("route")));
    expectFailure(
        () ->
            transfer(
                0,
                2,
                1,
                0,
                repeated('a', 257),
                BigInteger.ONE,
                0,
                text("alice"),
                1,
                repeated(1, 20),
                text("route")));
    final byte[] badTron = repeated(1, 21);
    badTron[0] = 0x42;
    expectFailure(
        () ->
            transfer(
                0,
                3,
                1,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                2,
                badTron,
                text("route")));

    final byte[] tonAccount = repeated(0x31, 36);
    Arrays.fill(tonAccount, 0, 4, (byte) 0);
    final SccpTransferPayloadV1 tonTransfer =
        transfer(
            0,
            4,
            1,
            0,
            text("xor"),
            BigInteger.ONE,
            0,
            text("alice"),
            3,
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
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                3,
                nonBasechain,
                text("taira_ton_xor")));
    final byte[] zeroAccount = new byte[36];
    expectFailure(
        () ->
            transfer(
                0,
                4,
                1,
                0,
                text("xor"),
                BigInteger.ONE,
                0,
                text("alice"),
                3,
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
            0,
            text("xor"),
            BigInteger.ONE,
            1,
            repeated(1, 20),
            0,
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
                  0,
                  text("xor"),
                  BigInteger.ONE,
                  1,
                  repeated(1, 20),
                  0,
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
        new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.TRON_MAINNET);
    final byte[] tron = repeated(4, 21);
    tron[0] = 0x41;
    final SccpTransferPayloadV1 payload =
        transfer(
            0,
            3,
            1,
            0,
            text("xor"),
            BigInteger.ONE,
            0,
            text("alice"),
            2,
            tron,
            text("taira_tron_xor"));
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
        transfer(
            0,
            2,
            1,
            0,
            asset,
            BigInteger.ONE,
            0,
            text("alice"),
            1,
            repeated(1, 20),
            text("route"));
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
    return transfer(
        0,
        2,
        1,
        0,
        text("xor"),
        BigInteger.ONE,
        0,
        text("alice@taira"),
        1,
        repeated(1, 20),
        text("taira_bsc_xor"));
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
        0,
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

  private static byte[] nonCompactSingleAccountPayload(final byte[] compact) {
    if (compact.length <= 14 || (compact[4] & 0xff) != compact.length - 5) {
      throw new AssertionError("unexpected compact AccountId fixture layout");
    }
    final long count = readU64Le(compact, 5);
    if (count <= 0 || count > Integer.MAX_VALUE) {
      throw new AssertionError("unexpected AccountId public-key length");
    }
    int offset = 13;
    final byte[] elements = new byte[(int) count];
    for (int index = 0; index < elements.length; index++) {
      if ((compact[offset] & 0xff) != 1) {
        throw new AssertionError("unexpected compact AccountId element length");
      }
      elements[index] = compact[offset + 1];
      offset += 2;
    }
    if (offset != compact.length) {
      throw new AssertionError("unexpected compact AccountId suffix");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(compact, 0, 4);
    writeU64Le(out, 8L + 9L * elements.length);
    writeU64Le(out, elements.length);
    for (final byte element : elements) {
      writeU64Le(out, 1);
      out.write(element);
    }
    return out.toByteArray();
  }

  private static long readU64Le(final byte[] value, final int offset) {
    long result = 0;
    for (int index = 0; index < 8; index++) {
      result |= (long) (value[offset + index] & 0xff) << (index * 8);
    }
    return result;
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final long value) {
    for (int index = 0; index < 8; index++) out.write((int) (value >>> (index * 8)) & 0xff);
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
