package org.hyperledger.iroha.sdk;

import static org.junit.jupiter.api.Assertions.*;

import java.io.InputStream;
import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.sdk.crypto.IrohaHash;
import org.hyperledger.iroha.sdk.crypto.KeyGenerationOutcome;
import org.hyperledger.iroha.sdk.crypto.KeyManagementException;
import org.hyperledger.iroha.sdk.crypto.KeyProvider;
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.sdk.crypto.Signer;
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm;
import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.sdk.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.sdk.crypto.keystore.KeyGenerationResult;
import org.hyperledger.iroha.sdk.crypto.keystore.KeySecurityPreference;
import org.hyperledger.iroha.sdk.crypto.keystore.KeystoreBackend;
import org.hyperledger.iroha.sdk.crypto.keystore.KeystoreKeyProvider;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidAttestationRevocationTestFixtures;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerifier;
import org.junit.jupiter.api.Test;

/** Java consumers exercise Android provider dispatch; synthetic routes do not qualify hardware. */
class IrohaKeyManagerJavaConsumerTest {
    private static final long EVALUATION_TIME = 1764547200000L;

    @Test
    void generatesDistinctEphemeralKeys() throws Exception {
        IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);
        KeyPair first = manager.generateEphemeral();
        KeyPair second = manager.generateEphemeral();
        assertNotNull(first.getPrivate());
        assertNotNull(second.getPrivate());
        assertFalse(Arrays.equals(first.getPrivate().getEncoded(), second.getPrivate().getEncoded()));
        assertFalse(Arrays.equals(first.getPublic().getEncoded(), second.getPublic().getEncoded()));
    }

    @Test
    void reusesAnAliasAcrossPreferencesWhenOnlySoftwareIsAvailable() throws Exception {
        IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);
        KeyPair created = manager.generateOrLoad("wallet", KeySecurityPreference.SOFTWARE_ONLY);
        KeyPair loaded = manager.generateOrLoad("wallet", KeySecurityPreference.HARDWARE_PREFERRED);
        assertArrayEquals(created.getPrivate().getEncoded(), loaded.getPrivate().getEncoded());
        assertArrayEquals(created.getPublic().getEncoded(), loaded.getPublic().getEncoded());
        assertFalse(manager.hasHardwareBackedProvider());
    }

    @Test
    void softwareFactoryMakesItsAlgorithmAndProviderSelectionExplicit() throws Exception {
        IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);
        assertEquals(SigningAlgorithm.ED25519, manager.signingAlgorithm());
        assertFalse(manager.hasHardwareBackedProvider());
        assertFalse(manager.hasStrongBoxProvider());
        assertEquals(1, manager.providerMetadata().size());
        assertEquals(KeyProviderMetadata.HardwareSecurityLevel.NONE,
                manager.providerMetadata().get(0).securityLevel);
        assertNotNull(manager.generateOrLoad("software", KeySecurityPreference.HARDWARE_PREFERRED).getPrivate());
    }

    @Test
    void failsWhenHardwareIsRequiredButUnavailable() {
        IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);
        assertThrows(KeyManagementException.class,
                () -> manager.generateOrLoad("strict", KeySecurityPreference.HARDWARE_REQUIRED));
    }

    @Test
    void prefersStrongBoxOverTeeAndSoftwareWhenAvailable() throws Exception {
        assertStrongBoxSelection(KeySecurityPreference.STRONGBOX_PREFERRED);
    }

    @Test
    void requiresStrongBoxWithoutQueryingWeakerProviders() throws Exception {
        assertStrongBoxSelection(KeySecurityPreference.STRONGBOX_REQUIRED);
    }

    @Test
    void missingStrongBoxDoesNotFallBackToTee() throws Exception {
        RecordingProvider tee = provider(KeyProviderMetadata.trustedEnvironment("tee"),
                KeyGenerationOutcome.Route.HARDWARE);
        IrohaKeyManager manager = IrohaKeyManager.fromProviders(
                Arrays.asList(tee, new SoftwareKeyProvider()));
        assertFalse(manager.hasStrongBoxProvider());
        assertThrows(KeyManagementException.class,
                () -> manager.generateOrLoad("strict", KeySecurityPreference.STRONGBOX_REQUIRED));
        assertEquals(0, tee.loads);
        assertEquals(0, tee.generations);
    }

    @Test
    void strictPoliciesRequirePerKeyEvidenceForLoadedAndGeneratedKeys() throws Exception {
        for (boolean existing : new boolean[] {false, true}) {
            RecordingProvider capabilityOnly = provider(KeyProviderMetadata.strongBox("claims", false),
                    KeyGenerationOutcome.Route.SOFTWARE);
            capabilityOnly.present = existing;
            IrohaKeyManager manager = IrohaKeyManager.fromProviders(Collections.singletonList(capabilityOnly));
            assertThrows(KeyManagementException.class,
                    () -> manager.generateOrLoad("strict", KeySecurityPreference.STRONGBOX_REQUIRED));
            assertThrows(KeyManagementException.class,
                    () -> manager.generateOrLoad("strict", KeySecurityPreference.HARDWARE_REQUIRED));
        }
    }

    @Test
    void rejectsNonEd25519MaterialReturnedByAProvider() throws Exception {
        RecordingProvider invalid = new RecordingProvider(KeyProviderMetadata.trustedEnvironment("invalid"),
                KeyGenerationOutcome.Route.HARDWARE, KeyPairGenerator.getInstance("EC").generateKeyPair());
        IrohaKeyManager manager = IrohaKeyManager.fromProviders(Arrays.asList(new SoftwareKeyProvider(), invalid));
        assertThrows(KeyManagementException.class,
                () -> manager.generateOrLoad("invalid", KeySecurityPreference.HARDWARE_PREFERRED));
        assertEquals(1, invalid.generations);
    }

    @Test
    void verifiesRecordedAttestationThroughTheManagerWithExplicitChallenge() throws Exception {
        AttestingBackend backend = new AttestingBackend(false);
        IrohaKeyManager manager = manager(backend);
        byte[] challenge = challenge();
        byte[] expected = challenge.clone();
        AttestationResult verified = manager.verifyAttestation("wallet", verifier(), challenge);
        assertNotNull(verified);
        assertTrue(verified.isStrongBoxAttestation());
        assertArrayEquals(expected, verified.attestationChallenge());
        assertArrayEquals(expected, challenge);
        assertEquals(1, backend.reads);
        assertEquals(0, backend.generated);
    }

    @Test
    void rejectsOneAliasBoundToDifferentKeysAcrossProviders() throws Exception {
        IrohaKeyManager manager = IrohaKeyManager.fromProviders(Arrays.asList(
                keystore(new AttestingBackend(false)), keystore(new AttestingBackend(true))));
        AttestationVerificationException error = assertThrows(AttestationVerificationException.class,
                () -> manager.verifyAttestation("wallet", verifier(), challenge()));
        assertTrue(error.getMessage().contains("different public keys"));
    }

    @Test
    void dispatchesChallengeBoundAttestationGenerationWithoutExposingCallerBytes() throws Exception {
        AttestingBackend backend = new AttestingBackend(false);
        IrohaKeyManager manager = manager(backend);
        byte[] challenge = challenge();
        byte[] retained = challenge.clone();
        KeyAttestation generated = manager.generateAttestation("wallet", challenge);
        assertNotNull(generated);
        assertEquals(1, backend.generated);
        assertArrayEquals(retained, backend.observedChallenge);
        assertArrayEquals(retained, challenge);
        assertTrue(verifier().verify(generated, retained).isStrongBoxAttestation());
        assertTrue(manager.verifyAttestation("wallet", verifier(), retained).isStrongBoxAttestation());
    }

    @Test
    void signsViaTheStoredAliasWithTheCanonicalPrehash() throws Exception {
        IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);
        Signer signer = manager.signerForAlias("signing", KeySecurityPreference.SOFTWARE_ONLY);
        byte[] message = "hello-iroha-android".getBytes(StandardCharsets.UTF_8);
        byte[] signed = signer.sign(message);
        KeyPair stored = manager.generateOrLoad("signing", KeySecurityPreference.SOFTWARE_ONLY);
        Signature verifier = Signature.getInstance("Ed25519");
        verifier.initVerify(stored.getPublic());
        verifier.update(IrohaHash.prehash(message));
        assertTrue(verifier.verify(signed));
    }

    @Test
    void rejectsEveryMlDsaHardwarePreferenceBeforeProviderDispatch() throws Exception {
        RecordingProvider provider = provider(KeyProviderMetadata.strongBox("unused", false),
                KeyGenerationOutcome.Route.STRONGBOX);
        IrohaKeyManager manager = IrohaKeyManager.fromProviders(
                Collections.singletonList(provider), SigningAlgorithm.ML_DSA);
        assertEquals(SigningAlgorithm.ML_DSA, manager.signingAlgorithm());
        for (KeySecurityPreference preference : KeySecurityPreference.values()) {
            if (preference == KeySecurityPreference.SOFTWARE_ONLY) continue;
            assertThrows(KeyManagementException.class,
                    () -> manager.generateOrLoad("ml-dsa", preference));
        }
        assertEquals(0, provider.loads);
        assertEquals(0, provider.generations);
    }

    private static void assertStrongBoxSelection(KeySecurityPreference preference) throws Exception {
        RecordingProvider strongBox = provider(KeyProviderMetadata.strongBox("strongbox", false),
                KeyGenerationOutcome.Route.STRONGBOX);
        RecordingProvider tee = provider(KeyProviderMetadata.trustedEnvironment("tee"),
                KeyGenerationOutcome.Route.HARDWARE);
        IrohaKeyManager manager = IrohaKeyManager.fromProviders(
                Arrays.asList(tee, new SoftwareKeyProvider(), strongBox));
        assertTrue(manager.hasStrongBoxProvider());
        assertTrue(manager.hasHardwareBackedProvider());
        assertSame(strongBox.pair, manager.generateOrLoad("wallet", preference));
        assertNotNull(strongBox.pair.getPrivate());
        assertEquals(1, strongBox.generations);
        assertEquals(0, tee.generations);
        // Preferred lookup may query a weaker provider for an existing alias; required lookup cannot.
        if (preference == KeySecurityPreference.STRONGBOX_REQUIRED) assertEquals(0, tee.loads);
        assertSame(strongBox.pair, manager.generateOrLoad("wallet", preference));
        assertEquals(1, strongBox.generations);
    }

    private static RecordingProvider provider(KeyProviderMetadata metadata, KeyGenerationOutcome.Route route)
            throws Exception {
        return new RecordingProvider(metadata, route, new SoftwareKeyProvider().generateEphemeral());
    }

    /** Test evidence is explicit per key; provider capability alone must not satisfy strict policies. */
    private static final class RecordingProvider implements KeyProvider {
        private final KeyProviderMetadata metadata;
        private final KeyGenerationOutcome.Route route;
        private final KeyPair pair;
        private int loads;
        private int generations;
        private boolean present;

        RecordingProvider(KeyProviderMetadata metadata, KeyGenerationOutcome.Route route, KeyPair pair) {
            this.metadata = metadata;
            this.route = route;
            this.pair = pair;
        }
        @Override public KeyPair load(String alias) { loads++; return present ? pair : null; }
        @Override public KeyPair generate(String alias) { generations++; present = true; return pair; }
        @Override public KeyPair generateEphemeral() { return pair; }
        @Override public boolean isHardwareBacked() { return metadata.hardwareBacked; }
        @Override public String name() { return metadata.name; }
        @Override public KeyProviderMetadata metadata() { return metadata; }
        @Override public KeyGenerationOutcome outcomeFor(String alias, KeyPair key) {
            return new KeyGenerationOutcome(key, route);
        }
    }

    private static IrohaKeyManager manager(AttestingBackend backend) {
        return IrohaKeyManager.fromProviders(Collections.singletonList(keystore(backend)));
    }

    private static KeystoreKeyProvider keystore(AttestingBackend backend) {
        return new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    }

    /** The shared recorded chain tests routing and trust validation, never live device attestation. */
    private static final class AttestingBackend implements KeystoreBackend {
        private final boolean conflicting;
        private int reads;
        private int generated;
        private byte[] observedChallenge;
        AttestingBackend(boolean conflicting) { this.conflicting = conflicting; }
        @Override public KeyPair load(String alias) {
            try {
                return new KeyPair(conflicting ? root().getPublicKey()
                        : CertificateFactory.getInstance("X.509").generateCertificate(
                                new ByteArrayInputStream(evidence().certificateChain().get(0))).getPublicKey(), null);
            } catch (Exception failure) { throw new IllegalStateException(failure); }
        }
        @Override public KeyGenerationResult generate(String alias, KeyGenParameters parameters) {
            throw new AssertionError("attestation dispatch must not generate a signing key");
        }
        @Override public KeyPair generateEphemeral(KeyGenParameters parameters) {
            throw new AssertionError("attestation dispatch must not generate an ephemeral key");
        }
        @Override public KeyProviderMetadata metadata() { return KeyProviderMetadata.strongBox(name(), true); }
        @Override public KeyProviderMetadata keyMetadata(String alias, KeyPair keyPair) { return metadata(); }
        @Override public String name() { return "recorded-attestation"; }
        @Override public KeyAttestation attestation(String alias) {
            reads++;
            try { return evidence(); } catch (Exception failure) { throw new IllegalStateException(failure); }
        }
        @Override public KeyAttestation generateAttestation(String alias, byte[] challenge) {
            generated++;
            observedChallenge = challenge.clone();
            Arrays.fill(challenge, (byte) 0);
            try { return evidence(); } catch (Exception failure) { throw new IllegalStateException(failure); }
        }
    }

    private static AttestationVerifier verifier() throws Exception {
        return AttestationVerifier.builder(AndroidAttestationRevocationTestFixtures.INSTANCE.policy(
                EVALUATION_TIME, 86400L, Collections.emptyList(), Collections.emptyList()), EVALUATION_TIME)
                .addTrustedRoot(root()).requireStrongBox(true).build();
    }

    private static X509Certificate root() throws Exception {
        try (InputStream source = Files.newInputStream(fixture("trust_root_huawei.pem"))) {
            return (X509Certificate) CertificateFactory.getInstance("X.509").generateCertificate(source);
        }
    }

    private static KeyAttestation evidence() throws Exception {
        KeyAttestation.Builder builder = KeyAttestation.builder().setAlias("wallet");
        try (InputStream source = Files.newInputStream(fixture("chain.pem"))) {
            for (Certificate certificate : CertificateFactory.getInstance("X.509").generateCertificates(source)) {
                builder.addCertificate((X509Certificate) certificate);
            }
        }
        return builder.build();
    }

    private static byte[] challenge() throws Exception {
        String hex = new String(Files.readAllBytes(fixture("challenge.hex")), StandardCharsets.US_ASCII).trim();
        byte[] value = new byte[hex.length() / 2];
        for (int index = 0; index < value.length; index++) {
            value[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
        }
        return value;
    }

    private static Path fixture(String name) {
        Path directory = Paths.get("").toAbsolutePath();
        while (directory != null) {
            Path fixture = directory.resolve("fixtures/android/attestation/mock_huawei").resolve(name);
            if (Files.isRegularFile(fixture)) return fixture;
            directory = directory.getParent();
        }
        throw new IllegalStateException("Shared attestation fixture missing: " + name);
    }
}
