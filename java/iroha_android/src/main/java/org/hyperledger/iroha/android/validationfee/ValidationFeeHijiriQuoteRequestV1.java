package org.hyperledger.iroha.android.validationfee;

import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;

/** Typed input for one exact aggregate Hijiri validation-fee quote. */
public final class ValidationFeeHijiriQuoteRequestV1 {
  /** Frozen request layout version. */
  public static final int VERSION = 1;

  /** Maximum aggregate transfer count accepted by the V1 quote route. */
  public static final int MAX_QUALIFYING_TRANSFERS = 100_000;

  /** Maximum canonical V1 request size accepted by Torii. */
  public static final int MAX_REQUEST_BYTES = 4 * 1024;

  private final String accountId;
  private final int qualifyingTransferCount;

  /** Constructs a validated V1 quote request. */
  public ValidationFeeHijiriQuoteRequestV1(
      final String accountId, final int qualifyingTransferCount) {
    this.accountId = requireCanonicalAccountId(accountId, "accountId");
    if (qualifyingTransferCount < 1 || qualifyingTransferCount > MAX_QUALIFYING_TRANSFERS) {
      throw new IllegalArgumentException(
          "qualifyingTransferCount must be between 1 and " + MAX_QUALIFYING_TRANSFERS);
    }
    this.qualifyingTransferCount = qualifyingTransferCount;
  }

  /** Returns the frozen request layout version. */
  public int version() {
    return VERSION;
  }

  /** Returns the canonical universal account whose Hijiri risk is priced. */
  public String accountId() {
    return accountId;
  }

  /** Returns the transfer count aggregated before the single Q16 ceiling. */
  public int qualifyingTransferCount() {
    return qualifyingTransferCount;
  }

  /** Encodes this request with the authoritative native Norito codec. */
  public byte[] toNoritoBytes() {
    return ValidationFeeHijiriQuoteBridge.encodeRequestV1(this);
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof ValidationFeeHijiriQuoteRequestV1)) {
      return false;
    }
    final ValidationFeeHijiriQuoteRequestV1 request =
        (ValidationFeeHijiriQuoteRequestV1) other;
    return accountId.equals(request.accountId)
        && qualifyingTransferCount == request.qualifyingTransferCount;
  }

  @Override
  public int hashCode() {
    return 31 * accountId.hashCode() + qualifyingTransferCount;
  }

  static String requireCanonicalAccountId(final String value, final String field) {
    final String account = Objects.requireNonNull(value, field);
    if (account.isEmpty() || !account.equals(account.trim()) || account.indexOf('@') >= 0) {
      throw new IllegalArgumentException(
          field + " must use one canonical domainless I105 account id");
    }
    final AccountAddress address;
    try {
      address = AccountAddress.parseEncodedIgnoringCurveSupport(account, null);
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          field + " must use one canonical domainless I105 account id", error);
    }
    final Integer discriminant = AccountAddress.detectI105Discriminant(account);
    if (discriminant == null) {
      throw new IllegalArgumentException(
          field + " must use one canonical domainless I105 account id");
    }
    final String canonical;
    try {
      canonical = address.toI105(discriminant.intValue());
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          field + " must use one canonical domainless I105 account id", error);
    }
    if (!canonical.equals(account)) {
      throw new IllegalArgumentException(
          field + " must use one canonical domainless I105 account id");
    }
    return canonical;
  }
}
