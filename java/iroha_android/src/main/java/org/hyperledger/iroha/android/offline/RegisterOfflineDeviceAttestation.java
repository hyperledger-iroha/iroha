package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Canonical request that registers one finalized Offline device attestation on chain. */
public final class RegisterOfflineDeviceAttestation {

  private final String chainId;
  private final String authority;
  private final OfflineNoteV2.DeviceAttestationRegistrationV2 registration;
  private final long creationTimeMs;
  private final Long timeToLiveMs;
  private final Integer nonce;
  private final Map<String, JsonValue> metadata;

  public RegisterOfflineDeviceAttestation(
      final String chainId,
      final String authority,
      final OfflineNoteV2.DeviceAttestationRegistrationV2 registration,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Integer nonce,
      final Map<String, JsonValue> metadata) {
    this.chainId = requireExactText(chainId, "chainId");
    this.authority = requireExactText(authority, "authority");
    this.registration = Objects.requireNonNull(registration, "registration");
    if (creationTimeMs < 0) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
    if (timeToLiveMs != null && timeToLiveMs <= 0) {
      throw new IllegalArgumentException("timeToLiveMs must be positive when present");
    }
    if (nonce != null && nonce <= 0) {
      throw new IllegalArgumentException("nonce must be positive when present");
    }
    this.creationTimeMs = creationTimeMs;
    this.timeToLiveMs = timeToLiveMs;
    this.nonce = nonce;
    this.metadata = Collections.unmodifiableMap(
        new LinkedHashMap<>(metadata == null ? Collections.emptyMap() : metadata));
  }

  public RegisterOfflineDeviceAttestation(
      final String chainId,
      final String authority,
      final OfflineNoteV2.DeviceAttestationRegistrationV2 registration,
      final long creationTimeMs) {
    this(chainId, authority, registration, creationTimeMs, null, null, Collections.emptyMap());
  }

  /** Exact native instruction carried by this transaction. */
  public InstructionBox instruction() {
    return OfflineNoteV2.registerDeviceAttestationInstruction(registration);
  }

  /** Build the canonical transaction payload without exporting account key material. */
  public TransactionPayload transactionPayload() {
    return TransactionPayload.builder()
        .setChainId(chainId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(Executable.instructions(List.of(instruction())))
        .setTimeToLiveMs(timeToLiveMs)
        .setNonce(nonce)
        .setMetadata(metadata)
        .build();
  }

  /** Encode and sign with the SDK's canonical transaction builder. */
  public SignedTransaction encodeAndSign(final TransactionBuilder builder, final Signer signer)
      throws NoritoException, SigningException {
    return Objects.requireNonNull(builder, "builder")
        .encodeAndSign(transactionPayload(), Objects.requireNonNull(signer, "signer"));
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
