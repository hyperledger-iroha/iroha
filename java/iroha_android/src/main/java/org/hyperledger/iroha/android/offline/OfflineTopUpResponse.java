package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Aggregated result for offline top-up (issue + register). */
public final class OfflineTopUpResponse {
  private final OfflineCertificateIssueResponse certificate;
  private final OfflineAllowanceRegisterResponse registration;

  public OfflineTopUpResponse(
      final OfflineCertificateIssueResponse certificate,
      final OfflineAllowanceRegisterResponse registration) {
    this.certificate = Objects.requireNonNull(certificate, "certificate");
    this.registration = Objects.requireNonNull(registration, "registration");
  }

  public OfflineCertificateIssueResponse certificate() {
    return certificate;
  }

  public OfflineAllowanceRegisterResponse registration() {
    return registration;
  }
}
