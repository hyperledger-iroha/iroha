package org.hyperledger.iroha.android.crypto.keystore;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.security.Key;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.PrivateKey;
import java.security.ProviderException;
import java.security.spec.AlgorithmParameterSpec;
import java.time.Duration;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Reflection-driven {@link AndroidKeystoreBackend} implementation that bridges to the platform
 * Android Keystore runtime when the SDK is running on Android hardware.
 *
 * <p>The implementation avoids compile-time dependencies on {@code android.*} packages so the
 * desktop JVM build remains compilable. When the runtime does not expose the Android Keystore
 * classes (for example, on desktop tests), the factory returns {@link Optional#empty()}.
 *
 * <p>Alias existence is the tri-state {@code KeyStore.getKey(alias, null)} probe ({@link
 * #probeAlias}). {@code containsAlias}, {@code getEntry}, {@code aliases}, {@code isKeyEntry} and
 * {@code getCertificate*} turn every Keystore error into "absent" (AOSP {@code
 * AndroidKeyStoreSpi}), and generating under an occupied alias replaces its key, so {@link #load}
 * returns empty only for a definitive keystore2 absence and throws when the Keystore cannot
 * answer. The backend is offered only on keystore2 (API 31+), where that absence is definitive.
 */
final class SystemAndroidKeystoreBackend implements AndroidKeystoreBackend {

  /** Lowest API level with keystore2, whose {@code KeyStore.getKey} distinguishes "no key" from an error. */
  static final int KEYSTORE2_MIN_API = 31;

  private static final String ANDROID_KEYSTORE = "AndroidKeyStore";
  private static final String KEY_GEN_SPEC_BUILDER_CLASS =
      "android.security.keystore.KeyGenParameterSpec$Builder";
  private static final String KEY_PROPERTIES_CLASS =
      "android.security.keystore.KeyProperties";
  private static final String ANDROID_STRONGBOX_UNAVAILABLE_EXCEPTION =
      "android.security.keystore.StrongBoxUnavailableException";

  private final KeyProviderMetadata metadata;

  private SystemAndroidKeystoreBackend(final KeyProviderMetadata metadata) {
    this.metadata = metadata;
  }

  static Optional<AndroidKeystoreBackend> create() {
    // keystore1 (API < 31) cannot prove that an alias is empty, so the platform backend is not
    // offered there. Its default algorithm, Ed25519, needs API 33 for Keystore keys.
    if (androidApiLevel() < KEYSTORE2_MIN_API) {
      return Optional.empty();
    }
    try {
      KeyStore.getInstance(ANDROID_KEYSTORE).load(null);
    } catch (final GeneralSecurityException | IOException ex) {
      return Optional.empty();
    }

    final boolean supportsStrongBox = detectStrongBoxSupport();

    final KeyProviderMetadata.Builder metadataBuilder =
        KeyProviderMetadata.builder("android-keystore")
            .setSupportsAttestationCertificates(true);
    if (supportsStrongBox) {
      metadataBuilder.setStrongBoxBacked(true);
    } else {
      metadataBuilder
          .setHardwareBacked(true)
          .setSecurityLevel(KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT);
    }
    final KeyProviderMetadata metadata = metadataBuilder.build();

    return Optional.of(new SystemAndroidKeystoreBackend(metadata));
  }

  /** The platform API level ({@code android.os.Build.VERSION.SDK_INT}), or 0 off Android. */
  private static int androidApiLevel() {
    try {
      return Class.forName("android.os.Build$VERSION").getField("SDK_INT").getInt(null);
    } catch (final ReflectiveOperationException | RuntimeException | LinkageError ignored) {
      return 0;
    }
  }

  /** {@code KeyStore.getKey(alias, null)}: a key, null for no key entry, or a throw. */
  @FunctionalInterface
  interface KeyLookup {
    Key getKey(String alias) throws Exception;
  }

  /**
   * Tri-state alias probe. On keystore2 (API 31+) a key is present, null is {@code KEY_NOT_FOUND}
   * and any throw means the Keystore did not answer. keystore1 {@code getKey} returns null whenever
   * its {@code KeyStore.contains} probe fails, so a null there is never definitive.
   *
   * @return the present key, or empty for a definitive absence
   * @throws KeyManagementException when the lookup throws, or below API 31 when it reports no key
   */
  static Optional<Key> probeAlias(
      final int apiLevel, final String alias, final KeyLookup lookup, final String failure)
      throws KeyManagementException {
    final Key key;
    try {
      key = lookup.getKey(alias);
    } catch (final Exception ex) {
      throw new KeyManagementException(
          failure + ": the Keystore could not answer, so the alias is not treated as absent", ex);
    }
    if (key == null && apiLevel < KEYSTORE2_MIN_API) {
      throw new KeyManagementException(
          failure + ": keystore1 (API " + apiLevel + ") cannot prove that an alias is empty");
    }
    return Optional.ofNullable(key);
  }

  private static Optional<Key> probeAlias(final String alias, final String failure)
      throws KeyManagementException {
    return probeAlias(androidApiLevel(), alias, a -> loadKeyStore().getKey(a, null), failure);
  }

  @Override
  public Optional<KeyPair> load(final String alias) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    if (alias.trim().isEmpty()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    final Optional<Key> key = probeAlias(alias, "Failed to load key from Android Keystore");
    if (key.isEmpty()) {
      return Optional.empty();
    }
    if (!(key.get() instanceof PrivateKey privateKey)) {
      throw new KeyManagementException("Android Keystore alias does not hold a private key");
    }
    final java.security.cert.Certificate[] chain;
    try {
      chain = loadKeyStore().getCertificateChain(alias);
    } catch (final GeneralSecurityException | IOException ex) {
      throw new KeyManagementException("Failed to load key from Android Keystore", ex);
    }
    if (chain == null || chain.length == 0) {
      throw new KeyManagementException("Android Keystore key has no certificate");
    }
    return Optional.of(new KeyPair(chain[0].getPublicKey(), privateKey));
  }

  @Override
  public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
      throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(parameters, "parameters");
    if (alias.trim().isEmpty()) {
      throw new IllegalArgumentException("alias must not be blank");
    }

    if (parameters.requireStrongBox() && !metadata.strongBoxBacked()) {
      throw new KeyManagementException("StrongBox required but backend is not StrongBox-capable");
    }
    final KeyGenParameters effective;
    if (parameters.preferStrongBox() && !metadata.strongBoxBacked()) {
      effective =
          parameters.toBuilder().setRequireStrongBox(false).setPreferStrongBox(false).build();
    } else {
      effective = parameters;
    }
    return generateWithPreferredStrongBoxFallback(
        effective,
        request -> {
          final KeyPairGenerator generator = createKeyPairGenerator(request.algorithm());
          final boolean strongBoxRequested =
              request.requireStrongBox() || request.preferStrongBox();
          return generateInternal(generator, alias, request, strongBoxRequested);
        });
  }

  static KeyGenerationResult generateWithPreferredStrongBoxFallback(
      final KeyGenParameters parameters, final GenerationAttempt attempt)
      throws KeyManagementException {
    try {
      final KeyGenerationResult result = attempt.generate(parameters);
      if (parameters.requireStrongBox() && !result.strongBoxBacked()) {
        throw new KeyManagementException(
            "StrongBox required but Android Keystore produced a weaker security level");
      }
      return result;
    } catch (final KeyManagementException strongBoxFailure) {
      if (!parameters.preferStrongBox()
          || parameters.requireStrongBox()
          || !isStrongBoxUnavailableFailure(strongBoxFailure)) {
        throw strongBoxFailure;
      }
      final KeyGenParameters fallback =
          parameters.toBuilder().setRequireStrongBox(false).setPreferStrongBox(false).build();
      try {
        return attempt.generate(fallback);
      } catch (final KeyManagementException fallbackFailure) {
        fallbackFailure.addSuppressed(strongBoxFailure);
        throw fallbackFailure;
      }
    }
  }

  private static boolean isStrongBoxUnavailableFailure(final Throwable failure) {
    Throwable current = failure;
    while (current != null) {
      if (current instanceof StrongBoxUnavailableFailure
          || ANDROID_STRONGBOX_UNAVAILABLE_EXCEPTION.equals(current.getClass().getName())) {
        return true;
      }
      current = current.getCause();
    }
    return false;
  }

  @FunctionalInterface
  interface GenerationAttempt {
    KeyGenerationResult generate(KeyGenParameters parameters) throws KeyManagementException;
  }

  private KeyGenerationResult generateInternal(
      final KeyPairGenerator generator,
      final String alias,
      final KeyGenParameters parameters,
      final boolean strongBoxRequested)
      throws KeyManagementException {
    AlgorithmParameterSpec spec;
    try {
      spec = buildKeyGenParameterSpec(alias, parameters, strongBoxRequested);
    } catch (final GeneralSecurityException ex) {
      throw new KeyManagementException("Failed to prepare Android Keystore parameters", ex);
    }

    try {
      generator.initialize(spec);
      final KeyPair pair = generator.generateKeyPair();
      return new KeyGenerationResult(pair, keyMetadata(alias, pair).strongBoxBacked());
    } catch (final ProviderException ex) {
      throw new KeyManagementException("Android Keystore generation failed", ex);
    } catch (final GeneralSecurityException ex) {
      if (parameters.algorithm().equalsIgnoreCase("Ed25519")) {
        throw new KeyManagementException(
            "Android Keystore does not support hardware Ed25519 key generation on this device",
            ex);
      }
      throw new KeyManagementException("Android Keystore generation failed", ex);
    }
  }

  @Override
  public KeyPair generateEphemeral(final KeyGenParameters parameters) throws KeyManagementException {
    // Android Keystore persists generated keys; generating/discarding aliases would leave state
    // behind. Defer to software providers for ephemeral operations.
    throw new KeyManagementException("Android Keystore does not support unmanaged ephemeral keys");
  }

  @Override
  public KeyProviderMetadata metadata() {
    return metadata;
  }

  @Override
  public KeyProviderMetadata keyMetadata(final String alias, final KeyPair keyPair) {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(keyPair, "keyPair");
    if (alias.trim().isEmpty()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    final KeyProviderMetadata.HardwareSecurityLevel level = keySecurityLevel(keyPair);
    final KeyProviderMetadata.Builder builder =
        KeyProviderMetadata.builder(metadata.name())
            .setSupportsAttestationCertificates(metadata.supportsAttestationCertificates());
    switch (level) {
      case STRONGBOX:
        builder.setStrongBoxBacked(true);
        break;
      case TRUSTED_ENVIRONMENT:
        builder
            .setHardwareBacked(true)
            .setSecurityLevel(KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT);
        break;
      case SECURE_ELEMENT:
        builder.setSecureElementBacked(true);
        break;
      case NONE:
      default:
        builder.setSecurityLevel(KeyProviderMetadata.HardwareSecurityLevel.NONE);
        break;
    }
    return builder.build();
  }

  @SuppressWarnings({"rawtypes", "unchecked"})
  private static KeyProviderMetadata.HardwareSecurityLevel keySecurityLevel(
      final KeyPair keyPair) {
    try {
      final Class keyInfoClass = Class.forName("android.security.keystore.KeyInfo");
      final KeyFactory keyFactory =
          KeyFactory.getInstance(keyPair.getPrivate().getAlgorithm(), ANDROID_KEYSTORE);
      final Object keyInfo = keyFactory.getKeySpec(keyPair.getPrivate(), keyInfoClass);
      try {
        final int securityLevel =
            ((Number) keyInfoClass.getMethod("getSecurityLevel").invoke(keyInfo)).intValue();
        final Class<?> keyProperties = Class.forName(KEY_PROPERTIES_CLASS);
        final int strongBox = keyProperties.getField("SECURITY_LEVEL_STRONGBOX").getInt(null);
        final int trustedEnvironment =
            keyProperties.getField("SECURITY_LEVEL_TRUSTED_ENVIRONMENT").getInt(null);
        final int unknownSecure =
            keyProperties.getField("SECURITY_LEVEL_UNKNOWN_SECURE").getInt(null);
        if (securityLevel == strongBox) {
          return KeyProviderMetadata.HardwareSecurityLevel.STRONGBOX;
        }
        if (securityLevel == trustedEnvironment || securityLevel == unknownSecure) {
          return KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT;
        }
        return KeyProviderMetadata.HardwareSecurityLevel.NONE;
      } catch (final NoSuchMethodException ignored) {
        final boolean hardwareBacked =
            (Boolean) keyInfoClass.getMethod("isInsideSecureHardware").invoke(keyInfo);
        return hardwareBacked
            ? KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT
            : KeyProviderMetadata.HardwareSecurityLevel.NONE;
      }
    } catch (final GeneralSecurityException | ReflectiveOperationException | RuntimeException ex) {
      return KeyProviderMetadata.HardwareSecurityLevel.NONE;
    }
  }

  @Override
  public String name() {
    return metadata.name();
  }

  @Override
  public Optional<KeyAttestation> attestation(final String alias) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    try {
      return loadAttestationBundle(alias);
    } catch (final GeneralSecurityException | IOException ex) {
      throw new KeyManagementException("Failed to read Android Keystore attestation", ex);
    }
  }

  @Override
  public Optional<KeyAttestation> generateAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    if (alias.trim().isEmpty()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    final byte[] challengeCopy = challenge == null ? new byte[0] : challenge.clone();
    if (probeAlias(alias, "Failed to read Android Keystore attestation").isEmpty()) {
      return Optional.empty();
    }
    try {
      final KeyStore keyStore = loadKeyStore();
      if (challengeCopy.length > 0) {
        throw new KeyManagementException(
            "Android Keystore cannot re-attest an existing alias; provision a new alias with "
                + "KeyGenParameters.setAttestationChallenge");
      }
      return loadAttestationBundle(alias, keyStore);
    } catch (final GeneralSecurityException | IOException ex) {
      throw new KeyManagementException("Failed to read Android Keystore attestation", ex);
    }
  }

  private static KeyStore loadKeyStore() throws GeneralSecurityException, IOException {
    final KeyStore keyStore = KeyStore.getInstance(ANDROID_KEYSTORE);
    keyStore.load(null);
    return keyStore;
  }

  private static KeyPairGenerator createKeyPairGenerator(final String algorithm)
      throws KeyManagementException {
    final String resolvedAlgorithm = algorithm == null ? "Ed25519" : algorithm;
    try {
      return KeyPairGenerator.getInstance(resolvedAlgorithm, ANDROID_KEYSTORE);
    } catch (final GeneralSecurityException ex) {
      throw new KeyManagementException(
          "Android Keystore does not support algorithm " + resolvedAlgorithm, ex);
    }
  }

  private static AlgorithmParameterSpec buildKeyGenParameterSpec(
      final String alias, final KeyGenParameters parameters, final boolean strongBox)
      throws GeneralSecurityException {
    try {
      final Class<?> builderClass = Class.forName(KEY_GEN_SPEC_BUILDER_CLASS);
      final Class<?> keyPropertiesClass = Class.forName(KEY_PROPERTIES_CLASS);

      final int purposeSign = keyPropertiesClass.getField("PURPOSE_SIGN").getInt(null);
      final int purposeVerify = keyPropertiesClass.getField("PURPOSE_VERIFY").getInt(null);

      final Object builder =
          builderClass
              .getConstructor(String.class, int.class)
              .newInstance(alias, purposeSign | purposeVerify);

      setAlgorithmParameterSpecIfNeeded(builder, parameters.algorithm());

      invoke(builder, "setDigests", new Class<?>[] {String[].class}, new Object[] {new String[] {
        keyPropertiesClass.getField("DIGEST_NONE").get(null).toString()
      }});

      if (parameters.userAuthenticationRequired()) {
        invoke(builder, "setUserAuthenticationRequired", new Class<?>[] {boolean.class}, true);
        final Duration timeout = parameters.userAuthenticationTimeout();
        long seconds = timeout == null ? 0L : timeout.getSeconds();
        if (seconds < 0L) {
          seconds = 0L;
        }
        final int safeSeconds =
            (int) Math.max(0L, Math.min(Integer.MAX_VALUE, seconds));
        invokeIfPresent(
            builder,
            "setUserAuthenticationValidityDurationSeconds",
            new Class<?>[] {int.class},
            safeSeconds);
      }

      if (strongBox) {
        try {
          invoke(builder, "setIsStrongBoxBacked", new Class<?>[] {boolean.class}, true);
        } catch (final NoSuchMethodException ex) {
          throw new StrongBoxUnavailableFailure(
              "StrongBox selection is unavailable on this Android API level", ex);
        }
      } else {
        invokeIfPresent(builder, "setIsStrongBoxBacked", new Class<?>[] {boolean.class}, false);
      }

      final byte[] challenge = parameters.attestationChallenge();
      if (challenge != null && challenge.length > 0) {
        try {
          invoke(
              builder, "setAttestationChallenge", new Class<?>[] {byte[].class}, challenge.clone());
        } catch (final NoSuchMethodException ex) {
          throw new GeneralSecurityException(
              "Attestation challenges are not supported on this Android API level", ex);
        }
      }

      final Integer usageCountLimit = parameters.usageCountLimit();
      if (usageCountLimit != null) {
        try {
          invoke(builder, "setMaxUsageCount", new Class<?>[] {int.class}, usageCountLimit);
        } catch (final NoSuchMethodException ex) {
          throw new GeneralSecurityException(
              "Usage count limits are not supported on this Android API level", ex);
        }
      }

      final Object spec = invoke(builder, "build", new Class<?>[0]);
      if (!(spec instanceof AlgorithmParameterSpec algorithmParameterSpec)) {
        throw new GeneralSecurityException("Android KeyGenParameterSpec build returned unexpected type");
      }
      return algorithmParameterSpec;
    } catch (final ReflectiveOperationException ex) {
      throw new GeneralSecurityException("Failed to construct Android KeyGenParameterSpec", ex);
    }
  }

  private static Object invoke(
      final Object target,
      final String name,
      final Class<?>[] parameterTypes,
      final Object... args)
      throws ReflectiveOperationException {
    final Class<?> clazz = target.getClass();
    final java.lang.reflect.Method method = clazz.getMethod(name, parameterTypes);
    return method.invoke(target, args);
  }

  private static void setAlgorithmParameterSpecIfNeeded(final Object builder, final String algorithm)
      throws ReflectiveOperationException {
    if (algorithm == null || !algorithm.equalsIgnoreCase("Ed25519")) {
      return;
    }
    final Class<?> namedParameterSpecClass = Class.forName("java.security.spec.NamedParameterSpec");
    final Object namedParameterSpec =
        namedParameterSpecClass.getConstructor(String.class).newInstance("Ed25519");
    invoke(
        builder,
        "setAlgorithmParameterSpec",
        new Class<?>[] {AlgorithmParameterSpec.class},
        namedParameterSpec);
  }

  private static void invokeIfPresent(
      final Object target,
      final String name,
      final Class<?>[] parameterTypes,
      final Object... args)
      throws ReflectiveOperationException {
    try {
      invoke(target, name, parameterTypes, args);
    } catch (final NoSuchMethodException ignored) {
      // Method not available on this API level; ignore when optional.
    }
  }

  // `initialize` only validates the spec and never creates an entry, so there is no probe alias
  // to clean up (and no entry this backend did not create is ever deleted).
  private static boolean detectStrongBoxSupport() {
    try {
      final KeyPairGenerator generator = KeyPairGenerator.getInstance("Ed25519", ANDROID_KEYSTORE);
      final KeyGenParameters parameters =
          KeyGenParameters.builder().setRequireStrongBox(true).build();
      final AlgorithmParameterSpec spec =
          buildKeyGenParameterSpec("__iroha_strongbox_probe__", parameters, true);
      generator.initialize(spec);
      return true;
    } catch (final ProviderException ex) {
      return false;
    } catch (final GeneralSecurityException ex) {
      return false;
    }
  }

  private Optional<KeyAttestation> loadAttestationBundle(final String alias)
      throws GeneralSecurityException, IOException {
    final KeyStore keyStore = loadKeyStore();
    return loadAttestationBundle(alias, keyStore);
  }

  private Optional<KeyAttestation> loadAttestationBundle(final String alias, final KeyStore keyStore)
      throws GeneralSecurityException {
    final java.security.cert.Certificate[] chain = keyStore.getCertificateChain(alias);
    return buildAttestation(alias, chain);
  }

  private Optional<KeyAttestation> buildAttestation(
      final String alias, final java.security.cert.Certificate[] chain) {
    if (chain == null || chain.length == 0) {
      return Optional.empty();
    }

    final KeyAttestation.Builder builder = KeyAttestation.builder().setAlias(alias);
    for (final java.security.cert.Certificate certificate : chain) {
      if (certificate instanceof java.security.cert.X509Certificate x509Certificate) {
        builder.addCertificate(x509Certificate);
      }
    }
    return Optional.of(builder.build());
  }
}
