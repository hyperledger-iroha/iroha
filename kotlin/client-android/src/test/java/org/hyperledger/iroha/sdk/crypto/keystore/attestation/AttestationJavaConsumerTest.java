package org.hyperledger.iroha.sdk.crypto.keystore.attestation;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.util.Collections;
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation;
import org.junit.jupiter.api.Test;

/** Java callers use the Kotlin verifier with explicit challenge, policy, roots and time. */
class AttestationJavaConsumerTest {
    private static final long EVALUATION_TIME = 1764547200000L;

    @Test
    void verifiesSharedStrongBoxFixtureWithExplicitTrustInputs() throws Exception {
        KeyAttestation evidence = evidence();
        byte[] challenge = challenge();
        byte[] retainedChallenge = challenge.clone();
        AttestationResult result = verifier(EVALUATION_TIME).verify(evidence, challenge);
        assertTrue(result.isStrongBoxAttestation());
        assertEquals("java-wallet", result.alias);
        assertArrayEquals(retainedChallenge, challenge);
        assertArrayEquals(retainedChallenge, result.attestationChallenge());
        byte[] exposedChallenge = result.attestationChallenge();
        exposedChallenge[0] ^= 0x01;
        assertArrayEquals(retainedChallenge, result.attestationChallenge());
    }

    @Test
    void verificationRequiresAnExplicitNonemptyMatchingChallenge() throws Exception {
        AttestationVerifier verifier = verifier(EVALUATION_TIME);
        KeyAttestation evidence = evidence();
        assertThrows(AttestationVerificationException.class, () -> verifier.verify(evidence, null));
        assertThrows(AttestationVerificationException.class, () -> verifier.verify(evidence, new byte[0]));
        byte[] mismatched = challenge();
        mismatched[0] ^= 0x01;
        assertThrows(AttestationVerificationException.class, () -> verifier.verify(evidence, mismatched));
    }

    @Test
    void requiredPolicyInputsCannotSubstituteForTrustedRoots() throws Exception {
        AttestationVerifier.Builder builder = AttestationVerifier.builder(policy(), EVALUATION_TIME);
        assertThrows(IllegalStateException.class, builder::build);
        assertTrue(builder.addTrustedRoot(root()).requireStrongBox(true).build()
                .verify(evidence(), challenge()).isStrongBoxAttestation());
    }

    @Test
    void separatelyConstructedVerifierUsesItsOwnExplicitEvaluationTime() throws Exception {
        AttestationVerifier current = verifier(EVALUATION_TIME);
        AttestationVerifier expired = verifier(EVALUATION_TIME + 86400000L);
        assertTrue(current.verify(evidence(), challenge()).isStrongBoxAttestation());
        assertThrows(AttestationVerificationException.class,
                () -> expired.verify(evidence(), challenge()));
        assertTrue(current.verify(evidence(), challenge()).isStrongBoxAttestation());
    }

    private static AttestationVerifier verifier(long evaluationTime) throws Exception {
        return AttestationVerifier.builder(policy(), evaluationTime)
                .addTrustedRoot(root()).requireStrongBox(true).build();
    }

    private static AndroidAttestationRevocationPolicyV1 policy() {
        return AndroidAttestationRevocationTestFixtures.INSTANCE.policy(
                EVALUATION_TIME, 86400L, Collections.emptyList(), Collections.emptyList());
    }

    private static X509Certificate root() throws Exception {
        try (InputStream source = Files.newInputStream(fixture("trust_root_huawei.pem"))) {
            return (X509Certificate) CertificateFactory.getInstance("X.509").generateCertificate(source);
        }
    }

    private static KeyAttestation evidence() throws Exception {
        KeyAttestation.Builder builder = KeyAttestation.builder().setAlias("java-wallet");
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
