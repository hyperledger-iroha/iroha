package org.hyperledger.iroha.android.tx;

import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

/**
 * Holds a fully encoded and signed transaction ready for submission to an Iroha node, including
 * optional multisig signature bundles.
 */
public final class SignedTransaction {
  private final byte[] encodedPayload;
  private final byte[] signature;
  private final byte[] publicKey;
  private final byte[] blsPublicKey;
  private final String schemaName;
  private final String keyAlias;
  private final byte[] exportedKeyBundle;
  private final MultisigSignatures multisigSignatures;

  public SignedTransaction(
      final byte[] encodedPayload,
      final byte[] signature,
      final byte[] publicKey,
      final String schemaName) {
    this(encodedPayload, signature, publicKey, schemaName, null, null, null, null);
  }

  public SignedTransaction(
      final byte[] encodedPayload,
      final byte[] signature,
      final byte[] publicKey,
      final String schemaName,
      final String keyAlias) {
    this(encodedPayload, signature, publicKey, schemaName, keyAlias, null, null, null);
  }

  public SignedTransaction(
      final byte[] encodedPayload,
      final byte[] signature,
      final byte[] publicKey,
      final String schemaName,
      final String keyAlias,
      final byte[] exportedKeyBundle) {
    this(
        encodedPayload,
        signature,
        publicKey,
        schemaName,
        keyAlias,
        exportedKeyBundle,
        null,
        null);
  }

  private SignedTransaction(
      final byte[] encodedPayload,
      final byte[] signature,
      final byte[] publicKey,
      final String schemaName,
      final String keyAlias,
      final byte[] exportedKeyBundle,
      final byte[] blsPublicKey,
      final MultisigSignatures multisigSignatures) {
    this.encodedPayload =
        Arrays.copyOf(Objects.requireNonNull(encodedPayload, "encodedPayload"), encodedPayload.length);
    this.signature = Arrays.copyOf(Objects.requireNonNull(signature, "signature"), signature.length);
    this.publicKey = Arrays.copyOf(Objects.requireNonNull(publicKey, "publicKey"), publicKey.length);
    this.schemaName = Objects.requireNonNull(schemaName, "schemaName");
    this.keyAlias = keyAlias;
    this.exportedKeyBundle =
        exportedKeyBundle == null ? null : Arrays.copyOf(exportedKeyBundle, exportedKeyBundle.length);
    this.blsPublicKey =
        blsPublicKey == null ? null : Arrays.copyOf(blsPublicKey, blsPublicKey.length);
    this.multisigSignatures = multisigSignatures;
  }

  public byte[] encodedPayload() {
    return Arrays.copyOf(encodedPayload, encodedPayload.length);
  }

  public byte[] signature() {
    return Arrays.copyOf(signature, signature.length);
  }

  public byte[] publicKey() {
    return Arrays.copyOf(publicKey, publicKey.length);
  }

  public Optional<byte[]> blsPublicKey() {
    if (blsPublicKey == null) {
      return Optional.empty();
    }
    return Optional.of(Arrays.copyOf(blsPublicKey, blsPublicKey.length));
  }

  public String schemaName() {
    return schemaName;
  }

  public Optional<String> keyAlias() {
    return Optional.ofNullable(keyAlias);
  }

  public Optional<byte[]> exportedKeyBundle() {
    if (exportedKeyBundle == null) {
      return Optional.empty();
    }
    return Optional.of(Arrays.copyOf(exportedKeyBundle, exportedKeyBundle.length));
  }

  public Optional<MultisigSignatures> multisigSignatures() {
    return Optional.ofNullable(multisigSignatures);
  }

  public Builder toBuilder() {
    return builder()
        .setEncodedPayload(encodedPayload())
        .setSignature(signature())
        .setPublicKey(publicKey())
        .setSchemaName(schemaName)
        .setKeyAlias(keyAlias)
        .setExportedKeyBundle(exportedKeyBundle().orElse(null))
        .setBlsPublicKey(blsPublicKey().orElse(null))
        .setMultisigSignatures(multisigSignatures().orElse(null));
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private byte[] encodedPayload;
    private byte[] signature;
    private byte[] publicKey;
    private String schemaName;
    private String keyAlias;
    private byte[] exportedKeyBundle;
    private byte[] blsPublicKey;
    private MultisigSignatures multisigSignatures;

    private static byte[] cloneArray(final byte[] input) {
      return input == null ? null : input.clone();
    }

    public Builder setEncodedPayload(final byte[] encodedPayload) {
      this.encodedPayload = cloneArray(encodedPayload);
      return this;
    }

    public Builder setSignature(final byte[] signature) {
      this.signature = cloneArray(signature);
      return this;
    }

    public Builder setPublicKey(final byte[] publicKey) {
      this.publicKey = cloneArray(publicKey);
      return this;
    }

    public Builder setSchemaName(final String schemaName) {
      this.schemaName = schemaName;
      return this;
    }

    public Builder setKeyAlias(final String keyAlias) {
      this.keyAlias = keyAlias;
      return this;
    }

    public Builder setExportedKeyBundle(final byte[] exportedKeyBundle) {
      this.exportedKeyBundle = cloneArray(exportedKeyBundle);
      return this;
    }

    public Builder setBlsPublicKey(final byte[] blsPublicKey) {
      this.blsPublicKey = cloneArray(blsPublicKey);
      return this;
    }

    public Builder setMultisigSignatures(final MultisigSignatures multisigSignatures) {
      this.multisigSignatures = multisigSignatures;
      return this;
    }

    public Builder setMultisigSignatures(final List<MultisigSignature> signatures) {
      if (signatures == null) {
        this.multisigSignatures = null;
      } else {
        this.multisigSignatures = MultisigSignatures.of(signatures);
      }
      return this;
    }

    public SignedTransaction build() {
      if (encodedPayload == null || signature == null || publicKey == null || schemaName == null) {
        throw new IllegalStateException(
            "encodedPayload, signature, publicKey, and schemaName must be provided");
      }
      return new SignedTransaction(
          encodedPayload,
          signature,
          publicKey,
          schemaName,
          keyAlias,
          exportedKeyBundle,
          blsPublicKey,
          multisigSignatures);
    }
  }
}
