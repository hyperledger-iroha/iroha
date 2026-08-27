package org.hyperledger.iroha.android.alias;

import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Independently trusted first-release faucet identity and exact issuance policy. */
public final class AccountFaucetPolicyV1 {
  private final String faucetAuthority;
  private final String assetDefinitionId;
  private final NumericV1.QuantityValue amount;

  /** Constructs one exact local faucet trust policy. */
  public AccountFaucetPolicyV1(
      final String faucetAuthority,
      final String assetDefinitionId,
      final NumericV1.QuantityValue amount) {
    this.faucetAuthority =
        AccountIdLiteral.requireCanonicalI105Address(faucetAuthority, "faucetAuthority");
    try {
      if (!AccountAddress.parseEncoded(this.faucetAuthority, null)
          .singleKeyPayloadIgnoringCurveSupport()
          .isPresent()) {
        throw new IllegalArgumentException(
            "faucetAuthority must be a single-signatory account");
      }
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          "faucetAuthority must be a canonical single-signatory account", error);
    }
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(assetDefinitionId)) {
      throw new IllegalArgumentException(
          "assetDefinitionId must be a canonical asset-definition address");
    }
    this.assetDefinitionId = assetDefinitionId;
    this.amount = Objects.requireNonNull(amount, "amount");
    if (amount.mantissa().signum() <= 0) {
      throw new IllegalArgumentException("faucet policy amount must be positive");
    }
  }

  public String faucetAuthority() { return faucetAuthority; }
  public String assetDefinitionId() { return assetDefinitionId; }
  public NumericV1.QuantityValue amount() { return amount; }
}
