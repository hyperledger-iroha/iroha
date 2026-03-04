package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Response payload for issuing or renewing an offline certificate. */
public final class OfflineCertificateIssueResponse {
  private final String certificateIdHex;
  private final OfflineWalletCertificate certificate;

  public OfflineCertificateIssueResponse(
      final String certificateIdHex, final OfflineWalletCertificate certificate) {
    this.certificateIdHex = Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    this.certificate = Objects.requireNonNull(certificate, "certificate");
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public OfflineWalletCertificate certificate() {
    return certificate;
  }
}
