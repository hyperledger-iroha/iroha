package org.hyperledger.iroha.android.model.instructions;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.connect.ConnectCrypto;
import org.hyperledger.iroha.android.connect.ConnectProtocolException;
import org.hyperledger.iroha.android.multisig.MultisigSpec;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.junit.Test;

/** Regression tests for strict encoded-only account/asset literal handling. */
public final class AccountLiteralHardCutTests {

  public static void main(final String[] args) throws Exception {
    final AccountLiteralHardCutTests tests = new AccountLiteralHardCutTests();
    tests.accountBuildersRejectDomainSuffixedLiterals();
    tests.accountTargetInstructionsRejectDomainSuffixedLiterals();
    tests.persistCouncilRejectsDomainSuffixedMembers();
    tests.uaidPortfolioQueryAcceptsAssetSelectorsAndRejectsMalformedSelectors();
    tests.offlineListParamsAcceptsAssetSelectorsAndRejectsMalformedSelectors();
    tests.connectApprovePreimageRejectsDomainSuffix();
    System.out.println("[IrohaAndroid] AccountLiteralHardCutTests passed.");
  }

  @Test
  public void accountBuildersRejectDomainSuffixedLiterals() throws Exception {
    final String account = sampleI105(0x11);
    final String nonCanonical = account + "@banka.dataspace";

    expectIllegalArgument(() -> GrantRoleInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> RevokeRoleInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> RegisterRoleInstruction.builder().setOwnerAccountId(nonCanonical));
    expectIllegalArgument(() -> CastPlainBallotInstruction.builder().setOwnerAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferDomainInstruction.builder().setSourceAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferDomainInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(
        () -> TransferAssetDefinitionInstruction.builder().setSourceAccountId(nonCanonical));
    expectIllegalArgument(
        () -> TransferAssetDefinitionInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferNftInstruction.builder().setSourceAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferNftInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferRwaInstruction.builder().setSourceAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferRwaInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> ForceTransferRwaInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> TransferAssetInstruction.builder().setDestinationAccountId(nonCanonical));
    expectIllegalArgument(() -> RegisterAccountInstruction.builder().setAccountId(nonCanonical));
    expectIllegalArgument(() -> MultisigRegisterInstruction.builder().setAccountId(nonCanonical));
    expectIllegalArgument(() -> MultisigSpec.builder().addSignatory(nonCanonical, 1));
  }

  @Test
  public void accountTargetInstructionsRejectDomainSuffixedLiterals() throws Exception {
    final String account = sampleI105(0x22);
    final String nonCanonical = account + "@banka.dataspace";

    expectIllegalArgument(() -> SetKeyValueInstruction.builder().setAccountId(nonCanonical));
    expectIllegalArgument(() -> RemoveKeyValueInstruction.builder().setAccountId(nonCanonical));
    expectIllegalArgument(() -> UnregisterInstruction.builder().setAccountId(nonCanonical));
  }

  @Test
  public void persistCouncilRejectsDomainSuffixedMembers() throws Exception {
    final String account = sampleI105(0x33);
    expectIllegalArgument(
        () -> PersistCouncilForEpochInstruction.builder().addMember(account + "@banka.dataspace"));
    expectIllegalArgument(
        () -> PersistCouncilForEpochInstruction.builder().addAlternate(account + "@banka.dataspace"));
  }

  @Test
  public void uaidPortfolioQueryAcceptsAssetSelectorsAndRejectsMalformedSelectors() {
    final String asset = "61CtjvNd9T3THAR65GsMVHr82Bjc";
    final UaidPortfolioQuery query =
        UaidPortfolioQuery.builder().setAsset(asset).setScope("global").build();
    final String normalizedAsset = query.toQueryParameters().get("asset");
    final String normalizedScope = query.toQueryParameters().get("scope");
    assert asset.equals(normalizedAsset) : "asset selector must be preserved";
    assert "global".equals(normalizedScope) : "scope must be preserved";

    expectIllegalArgument(() -> UaidPortfolioQuery.builder().setAsset("not:an-asset"));
    expectIllegalArgument(() -> UaidPortfolioQuery.builder().setAsset(" " + asset));
    expectIllegalArgument(() -> UaidPortfolioQuery.builder().setAsset(asset + " "));
    expectIllegalArgument(() -> UaidPortfolioQuery.builder().setAsset(asset).setScope(" global"));
  }

  @Test
  public void offlineListParamsAcceptsAssetSelectorsAndRejectsMalformedSelectors() {
    final String asset = "pkr#paynet";
    final OfflineListParams params = OfflineListParams.builder().assetId(asset).build();
    assert asset.equals(params.toQueryParameters().get("asset_id"))
        : "offline list asset selector must be preserved";

    expectIllegalArgument(
        () -> OfflineListParams.builder().assetId(" " + asset).build().toQueryParameters());
    expectIllegalArgument(
        () -> OfflineListParams.builder().assetId(asset + " ").build().toQueryParameters());
    expectIllegalArgument(
        () -> OfflineListParams.builder().assetId(" ").build().toQueryParameters());
  }

  @Test
  public void connectApprovePreimageRejectsDomainSuffix() throws Exception {
    final byte[] sessionId = fill(0x10, 32);
    final byte[] appPublic = fill(0x20, 32);
    final byte[] walletPublic = fill(0x30, 32);
    final String account = sampleI105(0x44);

    final byte[] preimage =
        ConnectCrypto.buildApprovePreimage(sessionId, appPublic, walletPublic, account, null, null);
    final ByteBuffer preimageReader = ByteBuffer.wrap(preimage).order(ByteOrder.LITTLE_ENDIAN);
    assert "iroha-connect|approve|v1".equals(readTaggedUtf8(preimageReader, "domain"))
        : "preimage domain mismatch";
    assert Arrays.equals(sessionId, readTagged(preimageReader, "sid"))
        : "preimage session id mismatch";
    assert Arrays.equals(appPublic, readTagged(preimageReader, "app_pk"))
        : "preimage app key mismatch";
    assert Arrays.equals(walletPublic, readTagged(preimageReader, "wallet_pk"))
        : "preimage wallet key mismatch";
    assert account.equals(readTaggedUtf8(preimageReader, "account_id"))
        : "preimage account mismatch";
    assert !preimageReader.hasRemaining() : "preimage has unexpected trailing fields";

    try {
      ConnectCrypto.buildApprovePreimage(
          sessionId, appPublic, walletPublic, account + "@banka.dataspace", null, null);
      throw new AssertionError("expected ConnectProtocolException");
    } catch (final ConnectProtocolException expected) {
      assert expected.getMessage().contains("canonical I105 encoded")
          : "unexpected error: " + expected.getMessage();
    }
  }

  private static void expectIllegalArgument(final Runnable op) {
    try {
      op.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static byte[] fill(final int value, final int size) {
    final byte[] out = new byte[size];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static String readTaggedUtf8(final ByteBuffer reader, final String expectedTag) {
    return new String(readTagged(reader, expectedTag), StandardCharsets.UTF_8);
  }

  private static byte[] readTagged(final ByteBuffer reader, final String expectedTag) {
    final int tagLength = Short.toUnsignedInt(reader.getShort());
    final byte[] tagBytes = new byte[tagLength];
    reader.get(tagBytes);
    final String tag = new String(tagBytes, StandardCharsets.UTF_8);
    assert expectedTag.equals(tag) : "expected tag " + expectedTag + " got " + tag;
    final long valueLength = reader.getLong();
    assert valueLength <= Integer.MAX_VALUE : "tagged value too large";
    final byte[] value = new byte[Math.toIntExact(valueLength)];
    reader.get(value);
    return value;
  }

  private static String sampleI105(final int fill) throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fill);
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }
}
