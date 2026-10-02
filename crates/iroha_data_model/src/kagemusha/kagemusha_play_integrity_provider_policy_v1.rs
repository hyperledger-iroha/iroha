//! Data-only projections of the exact governed Play Integrity provider-policy original.
//! The native holder independently pins its original SHA through the admitted trust policy;
//! parsing these public fields does not authenticate a policy or a Google verdict.

use crate::DeriveJsonDeserialize;

/// Sole provider-policy JSON schema selected by ordinary trust-policy original digests.
pub const KAGEMUSHA_PLAY_INTEGRITY_PROVIDER_POLICY_SCHEMA_V1: &str =
    "iroha.kagemusha.play-integrity-verification-policy.v1";

#[derive(DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Project {
    id: String,
    number: u64,
}
#[derive(DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CredentialSubject {
    email: String,
    #[norito(rename = "clientId")]
    client_id: String,
}
#[derive(DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Original {
    schema: String,
    version: u16,
    #[norito(rename = "cloudProject")]
    cloud_project: Project,
    #[norito(rename = "packageName")]
    package_name: String,
    #[norito(rename = "packageVersion")]
    package_version: u64,
    #[norito(rename = "appSigningCertificateSha256Hex")]
    app_signing_certificate_sha256_hex: String,
    #[norito(rename = "credentialSubject")]
    credential_subject: CredentialSubject,
}

/// Public selected provider fields, without any policy, key or Google-token authority.
pub struct KagemushaPlayIntegrityProviderPolicyProjectionV1 {
    /// Positive project number from the full original, never from a caller token object.
    pub cloud_project_number: u64,
    /// Exact package selected by the same original.
    pub package_name: String,
    /// Exact positive package version selected by that original.
    pub package_version: u64,
    /// Lowercase SHA-256 of the selected app signing certificate.
    pub app_signing_certificate_sha256_hex: String,
}

/// Parse one bounded original using the same closed grammar as the server decoder adapter.
/// Native callers separately compare its raw SHA and retain it under current original custody.
/// # Errors
/// Rejects duplicate/unknown/missing fields, invalid public selectors or an oversized original.
pub fn kagemusha_play_integrity_provider_policy_projection_v1(
    raw: &[u8],
) -> Result<KagemushaPlayIntegrityProviderPolicyProjectionV1, String> {
    if raw.is_empty() || raw.len() > 16 * 1024 {
        return Err("Play Integrity provider original outside bound".into());
    }
    let value: Original = norito::json::from_slice(raw)
        .map_err(|_| "Play Integrity provider original grammar rejected")?;
    let project = &value.cloud_project;
    let first = project.id.as_bytes().first().copied().unwrap_or(0);
    let last = project.id.as_bytes().last().copied().unwrap_or(0);
    let project_character = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let package = &value.package_name;
    let package_valid = !package.is_empty()
        && package.len() <= 255
        && package.contains('.')
        && package.split('.').all(|part| {
            !part.is_empty()
                && (part.as_bytes()[0].is_ascii_alphabetic() || part.as_bytes()[0] == b'_')
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        });
    let email_suffix = format!("@{}.iam.gserviceaccount.com", project.id);
    let principal = value
        .credential_subject
        .email
        .strip_suffix(&email_suffix)
        .unwrap_or("");
    let client = &value.credential_subject.client_id;
    if value.schema != KAGEMUSHA_PLAY_INTEGRITY_PROVIDER_POLICY_SCHEMA_V1
        || value.version != 1
        || !(6..=30).contains(&project.id.len())
        || !first.is_ascii_lowercase()
        || !project_character(last)
        || !project
            .id
            .bytes()
            .all(|c| project_character(c) || c == b'-')
        || project.number == 0
        || !package_valid
        || value.package_version == 0
        || value.app_signing_certificate_sha256_hex.len() != 64
        || !value
            .app_signing_certificate_sha256_hex
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !(6..=30).contains(&principal.len())
        || !principal.as_bytes()[0].is_ascii_lowercase()
        || !project_character(*principal.as_bytes().last().unwrap_or(&0))
        || !principal.bytes().all(|c| project_character(c) || c == b'-')
        || client.is_empty()
        || client.len() > 40
        || client.as_bytes()[0] == b'0'
        || !client.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("Play Integrity provider public selectors rejected".into());
    }
    Ok(KagemushaPlayIntegrityProviderPolicyProjectionV1 {
        cloud_project_number: project.number,
        package_name: value.package_name,
        package_version: value.package_version,
        app_signing_certificate_sha256_hex: value.app_signing_certificate_sha256_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGINAL: &str = r#"{"schema":"iroha.kagemusha.play-integrity-verification-policy.v1","version":1,"cloudProject":{"id":"digital-kina-play-integrity","number":642560099159},"packageName":"pg.bpng.digitalkina","packageVersion":28,"appSigningCertificateSha256Hex":"2bba0e04aaa3932e7da556effad457a95e2caf53499d3c027081d0573231cabd","credentialSubject":{"email":"kina-integrity-decoder@digital-kina-play-integrity.iam.gserviceaccount.com","clientId":"116801097826214108702"}}"#;
    #[test]
    fn provider_projection_reads_original_project_and_rejects_grammar_substitutions() {
        let selected =
            kagemusha_play_integrity_provider_policy_projection_v1(ORIGINAL.as_bytes()).unwrap();
        assert_eq!(selected.cloud_project_number, 642_560_099_159);
        assert_eq!(selected.package_name, "pg.bpng.digitalkina");
        for changed in [
            ORIGINAL.replace("642560099159", "0"),
            ORIGINAL.replace("\"version\":1", "\"version\":1,\"version\":1"),
            ORIGINAL.replace("\"version\":1", "\"version\":1,\"unknown\":1"),
            ORIGINAL.replace("\"number\"", "\"cloudProjectNumber\""),
            ORIGINAL.replace("@digital-kina-play-integrity", "@foreign-project"),
        ] {
            assert!(
                kagemusha_play_integrity_provider_policy_projection_v1(changed.as_bytes()).is_err()
            );
        }
        assert!(
            kagemusha_play_integrity_provider_policy_projection_v1(&vec![b' '; 16385]).is_err()
        );
    }
}
