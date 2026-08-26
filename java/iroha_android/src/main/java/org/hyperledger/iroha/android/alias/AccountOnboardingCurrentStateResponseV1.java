package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.NetworkId;

/** One internally consistent atomic account-onboarding state observation. */
public final class AccountOnboardingCurrentStateResponseV1 extends AliasJsonValue {
  /** Current and only first-release layout. */
  public static final int VERSION = 1;

  private final int version;
  private final NetworkId networkId;
  private final String accountId;
  private final String alias;
  private final boolean accountExists;
  private final String aliasTargetAccountId;
  private final BigInteger observedBlockHeight;
  private final AccountOnboardingBlockHashV1 observedBlockHash;

  /** Constructs and validates one closed response. */
  public AccountOnboardingCurrentStateResponseV1(
      final int version,
      final NetworkId networkId,
      final String accountId,
      final String alias,
      final boolean accountExists,
      final String aliasTargetAccountId,
      final BigInteger observedBlockHeight,
      final AccountOnboardingBlockHashV1 observedBlockHash) {
    if (version != VERSION) throw new IllegalArgumentException("version must be " + VERSION);
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) throw new IllegalArgumentException("alias must be canonical");
    this.version = version;
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.alias = alias;
    this.accountExists = accountExists;
    this.aliasTargetAccountId =
        aliasTargetAccountId == null
            ? null
            : AccountIdLiteral.requireCanonicalI105Address(
                aliasTargetAccountId, "aliasTargetAccountId");
    this.observedBlockHeight =
        AccountOnboardingCurrentStateV1.requirePositiveU64(
            observedBlockHeight, "observedBlockHeight");
    this.observedBlockHash = Objects.requireNonNull(observedBlockHash, "observedBlockHash");
    if (!accountExists
        && this.aliasTargetAccountId != null
        && AccountOnboardingReceiptVerifier.sameAccountIdentity(
            this.accountId, this.aliasTargetAccountId)) {
      throw new IllegalArgumentException(
          "alias target cannot equal an account reported absent in the same snapshot");
    }
  }

  public int version() {
    return version;
  }

  public NetworkId networkId() {
    return networkId;
  }

  public String accountId() {
    return accountId;
  }

  public String alias() {
    return alias;
  }

  public boolean accountExists() {
    return accountExists;
  }

  public String aliasTargetAccountId() {
    return aliasTargetAccountId;
  }

  public BigInteger observedBlockHeight() {
    return observedBlockHeight;
  }

  public AccountOnboardingBlockHashV1 observedBlockHash() {
    return observedBlockHash;
  }

  /** Validates every trust pin and returns the exact closed classification. */
  public AccountOnboardingCurrentStateV1 classify(
      final AccountOnboardingCurrentStateRequestV1 request,
      final NetworkId expectedNetworkId) {
    if (!networkId.equals(expectedNetworkId)) {
      throw new IllegalArgumentException(
          "account onboarding current-state response changed networkId");
    }
    if (!accountId.equals(request.accountId()) || !alias.equals(request.alias())) {
      throw new IllegalArgumentException(
          "account onboarding current-state response did not echo the exact request");
    }
    if (!accountExists) {
      throw new IllegalArgumentException(
          "account onboarding current-state response reports the expected account absent");
    }
    final AccountOnboardingCurrentStateV1.Outcome outcome;
    if (aliasTargetAccountId == null) {
      outcome = AccountOnboardingCurrentStateV1.Outcome.ALIAS_ABSENT;
    } else if (AccountOnboardingReceiptVerifier.sameAccountIdentity(
        request.accountId(), aliasTargetAccountId)) {
      outcome = AccountOnboardingCurrentStateV1.Outcome.APPLIED;
    } else {
      outcome = AccountOnboardingCurrentStateV1.Outcome.ALIAS_CONFLICT;
    }
    return new AccountOnboardingCurrentStateV1(
        outcome, observedBlockHeight, observedBlockHash);
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", Integer.valueOf(version));
    map.put("network_id", networkId.literal());
    map.put("account_id", accountId);
    map.put("alias", alias);
    map.put("account_exists", Boolean.valueOf(accountExists));
    map.put("alias_target_account_id", aliasTargetAccountId);
    map.put("observed_block_height", observedBlockHeight);
    map.put("observed_block_hash", observedBlockHash.literal());
    return map;
  }
}
