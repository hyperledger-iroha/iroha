package org.hyperledger.iroha.sdk.privacy;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import org.junit.jupiter.api.Test;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.sdk.client.ConfidentialAssetToriiClient;
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor;
import org.hyperledger.iroha.sdk.client.LocalSigningContext;
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.sdk.client.ZkRootsResponse;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.core.model.NetworkId;

/** Original Java assertions against canonical Kotlin confidential owners. */
public final class ZkAssetMerklePathJavaConsumerTest {
  private static final NetworkId NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
  private static final KeyPair KEY_PAIR = generateKeyPair();

  @Test
  public void localProviderComputesAndVerifiesCurrentFrontierPath() {
    final List<byte[]> commitments = Arrays.asList(scalarBytes(1), scalarBytes(2), scalarBytes(3));
    final byte[] root = computeRoot(commitments);
    final LocalZkAssetMerklePathProvider provider =
        new LocalZkAssetMerklePathProvider(Arrays.asList(root), commitments);

    final ZkAssetMerklePath path =
        provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    assert path.leafIndex == 1L : "leaf index mismatch";
    assert path.getSiblings().size() == LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2
        : "sibling count mismatch";
    assert path.getDirections().length == LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2
        : "direction count mismatch";
    assert Arrays.equals(root, path.getRootAtHeight()) : "root mismatch";
    assert path.verify(commitments.get(1), root)
        : "path must verify";
    assert !path.verify(commitments.get(1), scalarBytes(9))
        : "path must reject wrong root";
  }

  @Test
  public void localProviderRejectsAmbiguousOrMismatchedFrontiers() {
    final byte[] repeated = scalarBytes(4);
    final LocalZkAssetMerklePathProvider duplicateProvider =
        new LocalZkAssetMerklePathProvider(Arrays.asList(), Arrays.asList(repeated, repeated));
    try {
      duplicateProvider.getMerklePathForCommitment("usd#bank", repeated).join();
      throw new AssertionError("expected duplicate commitment rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
    }

    final LocalZkAssetMerklePathProvider mismatchProvider =
        new LocalZkAssetMerklePathProvider(Arrays.asList(scalarBytes(9)), Arrays.asList(scalarBytes(1)));
    try {
      mismatchProvider.getMerklePathForCommitment("usd#bank", scalarBytes(1)).join();
      throw new AssertionError("expected root mismatch rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
    }

    assertThrows(
        () -> new ZkAssetMerklePath(1, Arrays.asList(new byte[32]), new byte[] {0}, new byte[32], 1));
    assertThrows(
        () -> new ZkAssetMerklePath(2, Arrays.asList(new byte[32]), new byte[] {0}, new byte[32], 1));
  }

  @Test
  public void toriiProviderFetchesAndValidatesNodeEndpointPaths() {
    final List<byte[]> commitments = Arrays.asList(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(Arrays.asList(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments.get(1))
            .join();
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, commitments.get(1), localPath));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    final ToriiZkAssetMerklePathProvider provider =
        new ToriiZkAssetMerklePathProvider(client, canonicalAuth());

    final ZkAssetMerklePath path =
        provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    assert "/v1/zk/merkle-path".equals(executor.lastRequest.uri.getPath())
        : "path mismatch";
    assert ("{\"asset_id\":\"usd#bank\",\"commitments\":[\"" + hex(commitments.get(1)) + "\"]}")
            .equals(executor.lastBody)
        : "request body mismatch: " + executor.lastBody;
    assert path.leafIndex == 1L : "leaf index mismatch";
    assert Arrays.equals(root, path.getRootAtHeight()) : "root mismatch";
    assert path.verify(commitments.get(1), root)
        : "path must verify";
  }

  @Test
  public void toriiProviderRejectsPathCountDriftAndReorderedNodeResponses() {
    final List<byte[]> commitments = Arrays.asList(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final LocalZkAssetMerklePathProvider localProvider =
        new LocalZkAssetMerklePathProvider(Arrays.asList(root), commitments);
    final ZkAssetMerklePath firstPath =
        localProvider.getMerklePathForCommitment("usd#bank", commitments.get(0)).join();
    final ZkAssetMerklePath secondPath =
        localProvider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    final ToriiZkAssetMerklePathProvider shortProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                Arrays.asList(new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      shortProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected short path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("Torii returned 1 Merkle paths for 2 commitments")
          : "wrong message";
    }

    final ToriiZkAssetMerklePathProvider longProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                Arrays.asList(
                    new MerklePathResponseEntry(commitments.get(0), firstPath),
                    new MerklePathResponseEntry(commitments.get(1), secondPath),
                    new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      longProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected long path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("Torii returned 3 Merkle paths for 2 commitments")
          : "wrong message";
    }

    final ToriiZkAssetMerklePathProvider reorderedProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                Arrays.asList(
                    new MerklePathResponseEntry(commitments.get(1), secondPath),
                    new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      reorderedProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected reordered path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("commitment mismatch at index 0")
          : "wrong message";
    }
  }

  @Test
  public void toriiProviderRejectsReusableExplicitFreshness() {
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(new CapturingExecutor("{}"))
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    assertThrows(
        () ->
            new ToriiZkAssetMerklePathProvider(
                client,
                new ToriiCanonicalRequestAuth(
                    "alice@universal",
                    ZkAssetMerklePathJavaConsumerTest::sign,
                    Long.valueOf(1_700_000_000_000L),
                    "reused-provider-nonce")));
  }

  @Test
  public void toriiProviderRejectsMismatchedNodeCommitment() {
    final byte[] requested = scalarBytes(1);
    final byte[] root = computeRoot(Arrays.asList(requested));
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(Arrays.asList(root), Arrays.asList(requested))
            .getMerklePathForCommitment("usd#bank", requested)
            .join();
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, scalarBytes(2), localPath));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    final ToriiZkAssetMerklePathProvider provider =
        new ToriiZkAssetMerklePathProvider(client, canonicalAuth());
    try {
      provider.getMerklePathForCommitment("usd#bank", requested).join();
      throw new AssertionError("expected commitment mismatch");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("commitment mismatch")
          : "wrong message";
    }
  }

  @Test
  public void toriiProviderRejectsNonVerifyingNodePath() {
    final List<byte[]> commitments = Arrays.asList(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(Arrays.asList(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments.get(1))
            .join();
    final List<byte[]> badSiblings = localPath.getSiblings();
    badSiblings.set(0, scalarBytes(9));
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, commitments.get(1), localPath, badSiblings));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    final ToriiZkAssetMerklePathProvider provider =
        new ToriiZkAssetMerklePathProvider(client, canonicalAuth());
    try {
      provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();
      throw new AssertionError("expected non-verifying path rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("does not verify")
          : "wrong message";
    }
  }

  @Test
  public void pathAccessorsReturnDefensiveCopies() {
    final byte[] commitment = scalarBytes(1);
    final LocalZkAssetMerklePathProvider provider =
        new LocalZkAssetMerklePathProvider(Arrays.asList(), Arrays.asList(commitment));
    final ZkAssetMerklePath path = provider.getMerklePathForCommitment("usd#bank", commitment).join();
    final byte[] root = path.getRootAtHeight();
    root[0] = 99;
    final byte[] sibling = path.getSiblings().get(0);
    sibling[0] = 88;
    final byte[] directions = path.getDirections();
    directions[0] = 1;

    assert path.verify(commitment, path.getRootAtHeight())
        : "defensive copies were not preserved";
  }

  private static byte[] scalarBytes(final int value) {
    final byte[] out = new byte[32];
    out[0] = (byte) value;
    return out;
  }

  private static String hex(final byte[] bytes) {
    return ZkRootsResponse.encodeHex(bytes, "bytes");
  }

  private static ToriiZkAssetMerklePathProvider toriiProviderWithResponse(final String responseBody) {
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(new CapturingExecutor(responseBody))
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    return new ToriiZkAssetMerklePathProvider(client, canonicalAuth());
  }

  private static String merklePathResponse(
      final byte[] root, final byte[] commitment, final ZkAssetMerklePath path) {
    return merklePathResponse(root, commitment, path, path.getSiblings());
  }

  private static String merklePathResponse(
      final byte[] root,
      final byte[] commitment,
      final ZkAssetMerklePath path,
      final List<byte[]> siblingsOverride) {
    return merklePathResponse(root, Arrays.asList(new MerklePathResponseEntry(commitment, path, siblingsOverride)));
  }

  private static String merklePathResponse(
      final byte[] root, final List<MerklePathResponseEntry> entries) {
    final int treeDepth = entries.isEmpty() ? 0 : entries.get(0).path.getSiblings().size();
    final ArrayList<String> paths = new ArrayList<>(entries.size());
    for (final MerklePathResponseEntry entry : entries) {
      final String siblings = quotedHexList(entry.siblings);
      final String directions = directionList(entry.path.getDirections());
      final String witnessNodes = quotedHexList(entry.path.getSiblings());
      paths.add(
          String.format("    {\n      \"commitment\": \"%s\",\n      \"leaf_index\": %d,\n      \"siblings\": [%s],\n      \"directions\": [%s],\n      \"witness_nodes\": [%s],\n      \"root\": \"%s\"\n    }\n", hex(entry.commitment), entry.path.leafIndex, siblings, directions, witnessNodes, hex(root)));
    }
    return String.format("    {\n      \"root\": \"%s\",\n      \"frontier_len\": 2,\n      \"tree_depth\": %d,\n      \"next_zero_path\": null,\n      \"paths\": [%s],\n      \"evaluated_block_height\": 7,\n      \"evaluated_block_hash\": \"%s\"\n    }\n", hex(root), treeDepth, String.join(",", paths), String.join("", Collections.nCopies(32, "0a")));
  }

  private static String quotedHexList(final List<byte[]> values) {
    final ArrayList<String> out = new ArrayList<>(values.size());
    for (final byte[] value : values) {
      out.add("\"" + hex(value) + "\"");
    }
    return String.join(",", out);
  }

  private static String directionList(final byte[] directions) {
    final ArrayList<String> out = new ArrayList<>(directions.length);
    for (final byte direction : directions) {
      out.add(Integer.toString(direction & 0xff));
    }
    return String.join(",", out);
  }

  private static void assertThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected path.
    }
  }

  private static ToriiCanonicalRequestAuth canonicalAuth() {
    return new ToriiCanonicalRequestAuth(
        "alice@universal", ZkAssetMerklePathJavaConsumerTest::sign);
  }

  private static KeyPair generateKeyPair() {
    try {
      return KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create signing key fixture", ex);
    }
  }

  private static byte[] sign(final byte[] message) {
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(KEY_PAIR.getPrivate());
      signer.update(message);
      return signer.sign();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to sign request fixture", ex);
    }
  }

  private static final class MerklePathResponseEntry {
    private final byte[] commitment;
    private final ZkAssetMerklePath path;
    private final List<byte[]> siblings;

    private MerklePathResponseEntry(final byte[] commitment, final ZkAssetMerklePath path) {
      this(commitment, path, path.getSiblings());
    }

    private MerklePathResponseEntry(
        final byte[] commitment, final ZkAssetMerklePath path, final List<byte[]> siblings) {
      this.commitment = commitment.clone();
      this.path = path;
      this.siblings = new ArrayList<>(siblings);
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final String responseBody;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private CapturingExecutor(final String responseBody) {
      this.responseBody = responseBody;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.getBody(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(
          new TransportResponse(
              200, responseBody.getBytes(StandardCharsets.UTF_8), "", Collections.emptyMap(),
              request.uri,
              false));
    }
  }

  private static byte[] computeRoot(final List<byte[]> commitments) {
    return new LocalZkAssetMerklePathProvider(Collections.emptyList(), commitments)
        .getMerklePathForCommitment("usd#bank", commitments.get(0)).join().getRootAtHeight();
  }
}
