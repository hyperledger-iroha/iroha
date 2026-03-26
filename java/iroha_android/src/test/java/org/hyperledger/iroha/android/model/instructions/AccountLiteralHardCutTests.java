package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.connect.ConnectCrypto;
import org.hyperledger.iroha.android.connect.ConnectProtocolException;
import org.hyperledger.iroha.android.multisig.MultisigSpec;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.junit.Test;

/** Regression tests for strict encoded-only account/asset literal handling. */
public final class AccountLiteralHardCutTests {

  @Test
  public void accountBuildersRejectDomainSuffixedLiterals() throws Exception {
    final String account = sampleI105(0x11);
    final String legacy = account + "@wonderland";

    expectIllegalArgument(() -> GrantRoleInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> RevokeRoleInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> RegisterRoleInstruction.builder().setOwnerAccountId(legacy));
    expectIllegalArgument(() -> CastPlainBallotInstruction.builder().setOwnerAccountId(legacy));
    expectIllegalArgument(() -> TransferDomainInstruction.builder().setSourceAccountId(legacy));
    expectIllegalArgument(() -> TransferDomainInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(
        () -> TransferAssetDefinitionInstruction.builder().setSourceAccountId(legacy));
    expectIllegalArgument(
        () -> TransferAssetDefinitionInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> TransferNftInstruction.builder().setSourceAccountId(legacy));
    expectIllegalArgument(() -> TransferNftInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> TransferRwaInstruction.builder().setSourceAccountId(legacy));
    expectIllegalArgument(() -> TransferRwaInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> ForceTransferRwaInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> TransferAssetInstruction.builder().setDestinationAccountId(legacy));
    expectIllegalArgument(() -> RegisterAccountInstruction.builder().setAccountId(legacy));
    expectIllegalArgument(() -> MultisigRegisterInstruction.builder().setAccountId(legacy));
    expectIllegalArgument(() -> MultisigSpec.builder().addSignatory(legacy, 1));
  }

  @Test
  public void accountTargetInstructionsRejectDomainSuffixedLiterals() throws Exception {
    final String account = sampleI105(0x22);
    final String legacy = account + "@wonderland";

    expectIllegalArgument(() -> SetKeyValueInstruction.builder().setAccountId(legacy));
    expectIllegalArgument(() -> RemoveKeyValueInstruction.builder().setAccountId(legacy));
    expectIllegalArgument(() -> UnregisterInstruction.builder().setAccountId(legacy));
  }

  @Test
  public void persistCouncilRejectsDomainSuffixedMembers() throws Exception {
    final String account = sampleI105(0x33);
    expectIllegalArgument(
        () -> PersistCouncilForEpochInstruction.builder().addMember(account + "@wonderland"));
    expectIllegalArgument(
        () -> PersistCouncilForEpochInstruction.builder().addAlternate(account + "@wonderland"));
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
  }

  @Test
  public void connectApprovePreimageRejectsLegacyDomainSuffix() throws Exception {
    final byte[] sessionId = fill(0x10, 32);
    final byte[] appPublic = fill(0x20, 32);
    final byte[] walletPublic = fill(0x30, 32);
    final String account = sampleI105(0x44);

    final byte[] preimage =
        ConnectCrypto.buildApprovePreimage(sessionId, appPublic, walletPublic, account, null, null);
    final String marker = "iroha-connect|approve|";
    final String rendered = new String(preimage, StandardCharsets.UTF_8);
    assert rendered.startsWith(marker) : "preimage prefix mismatch";

    try {
      ConnectCrypto.buildApprovePreimage(
          sessionId, appPublic, walletPublic, account + "@wonderland", null, null);
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

  private static String sampleI105(final int fill) throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fill);
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }
}
