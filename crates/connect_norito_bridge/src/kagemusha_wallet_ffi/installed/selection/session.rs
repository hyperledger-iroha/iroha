//! FI session originals authenticated by the existing signed application-release owner.
use super::*;
use p256::{
    EncodedPoint, FieldBytes,
    ecdsa::{Signature as P256Signature, VerifyingKey, signature::Verifier as _},
};
use zeroize::Zeroizing;

mod bpng;
pub(super) use bpng::{Authority as BpngAuthority, authority as bpng_authority};

const FIS: [&str; 6] = ["anz", "bred", "bsp", "ezipei", "m-selen", "pob"];
const FIELDS: &[&str] = &[
    "schema",
    "algorithm",
    "verificationKeyEd25519Base64",
    "issuer",
    "audience",
    "fiOriginalBase64",
    "dataspaceId",
    "androidClientApp",
    "appleClientApp",
    "requestOrigin",
    "externalPathPrefix",
    "enrollmentChallengePath",
    "releaseOriginalBase64",
];
pub(super) struct FiAuthority {
    key: PublicKey,
    issuer: String,
    dataspace: String,
    origin: String,
    release: Vec<u8>,
}
fn exact_text<'a>(map: &'a Map, name: &str, maximum: usize) -> Result<&'a str> {
    let value = text(map, name)?;
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(value)
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    BlobV1::of(bytes).sha256
}
pub(super) fn authorities(
    value: &Value,
    release_digest: &[u8; 32],
    android: &KagemushaWalletEnrollmentPolicyV1,
    apple: &KagemushaWalletEnrollmentPolicyV1,
) -> Result<Vec<FiAuthority>> {
    let selection = exact(value, &["schema", "sessions", "enrollmentCollection"])?;
    let collection = exact(
        field(selection, "enrollmentCollection")?,
        &[
            "androidAttestationRootDERBase64",
            "appleAttestationRootDERBase64",
            "androidPlayIntegrityCloudProjectNumber",
        ],
    )?;
    for (name, policy) in [
        ("androidAttestationRootDERBase64", android),
        ("appleAttestationRootDERBase64", apple),
    ] {
        let root = raw(collection, name, 16_384)?;
        let pinned = match policy.platform {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256,
                ..
            }
            | KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256,
            } => attestation_root_sha256,
        };
        if hash(&root) != pinned {
            return Err(invalid());
        }
    }
    let project = text(collection, "androidPlayIntegrityCloudProjectNumber")?;
    if project.is_empty()
        || project.len() > 19
        || project.starts_with('0')
        || !project.bytes().all(|byte| byte.is_ascii_digit())
        || project.parse::<i64>().ok().is_none_or(|value| value <= 0)
    {
        return Err(invalid());
    }
    if text(selection, "schema")? != "cbsi.fi-session-enrollment-selection.v1" {
        return Err(invalid());
    }
    let rows = field(selection, "sessions")?.as_array().ok_or(invalid())?;
    if rows.len() != FIS.len() {
        return Err(invalid());
    }
    let mut keys = std::collections::BTreeSet::new();
    rows.iter()
        .zip(FIS)
        .map(|(row, fi)| {
            let row = exact(row, FIELDS)?;
            let dataspace = format!("{fi}.cbsi");
            let issuer = format!("cbsi-fi-core-{fi}");
            let origin = format!("https://bokolo-{fi}.soramitsu.io");
            if text(row, "schema")? != "iroha.fi-session-authentication-original.v1"
                || text(row, "algorithm")? != "EdDSA"
                || text(row, "issuer")? != issuer
                || text(row, "audience")? != dataspace
                || text(row, "dataspaceId")? != dataspace
                || raw(row, "fiOriginalBase64", 128)? != dataspace.as_bytes()
                || text(row, "androidClientApp")? != "com.soramitsu.bokolocash"
                || text(row, "appleClientApp")? != "jp.co.soramitsu.bokolo"
                || text(row, "requestOrigin")? != origin
                || text(row, "externalPathPrefix")? != "/api"
                || text(row, "enrollmentChallengePath")?
                    != "/v1/retail/kagemusha/enrollment/challenge"
            {
                return Err(invalid());
            }
            let key: [u8; 32] = raw(row, "verificationKeyEd25519Base64", 32)?
                .try_into()
                .map_err(|_| invalid())?;
            if key == [0; 32] || !keys.insert(key) {
                return Err(invalid());
            }
            let release = raw(row, "releaseOriginalBase64", 16_384)?;
            if kagemusha_enrollment_permit_scope_digest_v1(
                KagemushaEnrollmentPermitScopeRoleV1::Release,
                &release,
            )
            .map_err(|_| invalid())?
                != *release_digest
            {
                return Err(invalid());
            }
            Ok(FiAuthority {
                key: PublicKey::from_bytes(Algorithm::Ed25519, &key).map_err(|_| invalid())?,
                issuer,
                dataspace,
                origin,
                release,
            })
        })
        .collect()
}
fn segment(original: &str) -> Result<Vec<u8>> {
    if original.is_empty() || original.contains('=') {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(original).map_err(|_| invalid())?;
    if URL_SAFE_NO_PAD.encode(&bytes) != original {
        return Err(invalid());
    }
    Ok(bytes)
}
fn compact(bytes: &[u8], maximum: usize) -> Result<(&str, &str, &str)> {
    bounded(bytes, maximum)?;
    let value = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    if !value.is_ascii() {
        return Err(invalid());
    }
    let mut parts = value.split('.');
    let h = parts.next().ok_or(invalid())?;
    let c = parts.next().ok_or(invalid())?;
    let s = parts.next().ok_or(invalid())?;
    if h.is_empty() || c.is_empty() || s.is_empty() || parts.next().is_some() {
        return Err(invalid());
    }
    Ok((h, c, s))
}
fn seconds(m: &Map, key: &str) -> Result<u64> {
    field(m, key)?
        .as_u64()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
        .ok_or(invalid())
}
fn uuid(value: &str) -> Result<()> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err(invalid());
    }
    Ok(())
}
/// Secret compact originals stay only in this live owner and are never formatted or persisted.
pub(crate) struct Session {
    token: Zeroizing<Vec<u8>>,
    proof: Zeroizing<Vec<u8>>,
    root: Vec<u8>,
    pub(crate) config: iroha_core_zk::kagemusha_wallet_enrollment_v1::EnrollmentConfigV1,
}
impl Session {
    pub(crate) fn matches(&self, originals: [&[u8]; 3]) -> bool {
        originals
            == [
                self.token.as_slice(),
                self.proof.as_slice(),
                self.root.as_slice(),
            ]
    }
}
impl Selection {
    pub(in super::super) fn enrollment_session(
        &self,
        android: bool,
        originals: [&[u8]; 3],
    ) -> Result<Session> {
        if let Some(selected) = &self.bpng_session {
            return bpng::authenticate(self, selected, android, originals);
        }
        let [token, proof, root] = originals;
        bounded(root, 16_384)?;
        let (h, c, s) = compact(token, 16_384)?;
        let header = json(&segment(h)?, 1024, false)?;
        let header = exact(&header, &["alg", "typ"])?;
        if text(header, "alg")? != "EdDSA" || text(header, "typ")? != "JWT" {
            return Err(invalid());
        }
        let signature = segment(s)?;
        if signature.len() != 64 {
            return Err(invalid());
        }
        let claims = json(&segment(c)?, 16_384, false)?;
        let claims = exact(
            &claims,
            &[
                "sub",
                "dataspace_id",
                "roles",
                "iat",
                "nbf",
                "exp",
                "iss",
                "aud",
                "device_id",
                "client_app",
                "cnf",
            ],
        )?;
        let selected = self
            .fi_sessions
            .iter()
            .find(|row| {
                text(claims, "iss").ok() == Some(row.issuer.as_str())
                    && text(claims, "dataspace_id").ok() == Some(row.dataspace.as_str())
            })
            .ok_or(invalid())?;
        Signature::from_bytes(&signature)
            .verify(&selected.key, format!("{h}.{c}").as_bytes())
            .map_err(|_| invalid())?;
        if text(claims, "aud")? != selected.dataspace
            || text(claims, "client_app")?
                != if android {
                    "com.soramitsu.bokolocash"
                } else {
                    "jp.co.soramitsu.bokolo"
                }
        {
            return Err(invalid());
        }
        let actor = exact_text(claims, "sub", 512)?.as_bytes().to_vec();
        uuid(text(claims, "device_id")?)?;
        let roles = field(claims, "roles")?.as_array().ok_or(invalid())?;
        if roles.is_empty() || roles.len() > 5 {
            return Err(invalid());
        }
        let mut seen = std::collections::BTreeSet::new();
        for role in roles {
            let role = role.as_str().ok_or(invalid())?;
            if !matches!(
                role,
                "FI_SIGNER" | "FI_BRANCH_MANAGER" | "FI_COMPLIANCE" | "FI_ADMIN" | "RETAIL_USER"
            ) || !seen.insert(role)
            {
                return Err(invalid());
            }
        }
        if !seen.contains("RETAIL_USER") {
            return Err(invalid());
        }
        let cnf = exact(field(claims, "cnf")?, &["jkt"])?;
        let jkt = text(cnf, "jkt")?;
        if jkt.len() != 43 || segment(jkt)?.len() != 32 {
            return Err(invalid());
        }
        let iat = seconds(claims, "iat")?;
        let nbf = seconds(claims, "nbf")?;
        let exp = seconds(claims, "exp")?;
        if iat > nbf || nbf >= exp {
            return Err(invalid());
        }
        let (h, c, s) = compact(proof, 4096)?;
        let header = json(&segment(h)?, 1024, false)?;
        let header = exact(&header, &["typ", "alg", "jwk"])?;
        if text(header, "typ")? != "dpop+jwt" || text(header, "alg")? != "ES256" {
            return Err(invalid());
        }
        let jwk = exact(field(header, "jwk")?, &["kty", "crv", "x", "y"])?;
        if text(jwk, "kty")? != "EC" || text(jwk, "crv")? != "P-256" {
            return Err(invalid());
        }
        let xt = text(jwk, "x")?;
        let yt = text(jwk, "y")?;
        let x: [u8; 32] = segment(xt)?.try_into().map_err(|_| invalid())?;
        let y: [u8; 32] = segment(yt)?.try_into().map_err(|_| invalid())?;
        let point = EncodedPoint::from_affine_coordinates(
            FieldBytes::from_slice(&x),
            FieldBytes::from_slice(&y),
            false,
        );
        let key = VerifyingKey::from_encoded_point(&point).map_err(|_| invalid())?;
        let jwk = format!("{{\"crv\":\"P-256\",\"kty\":\"EC\",\"x\":\"{xt}\",\"y\":\"{yt}\"}}");
        if URL_SAFE_NO_PAD.encode(hash(jwk.as_bytes())) != jkt {
            return Err(invalid());
        }
        let payload = json(&segment(c)?, 4096, false)?;
        let payload = exact(&payload, &["htm", "htu", "iat", "jti", "ath"])?;
        if text(payload, "htm")? != "POST"
            || text(payload, "htu")?
                != format!(
                    "{}/api/v1/retail/kagemusha/enrollment/challenge",
                    selected.origin
                )
            || text(payload, "ath")? != URL_SAFE_NO_PAD.encode(hash(token))
        {
            return Err(invalid());
        }
        uuid(text(payload, "jti")?)?;
        let proof_iat = seconds(payload, "iat")?;
        let sig = segment(s)?;
        let sig = P256Signature::from_slice(&sig).map_err(|_| invalid())?;
        key.verify(format!("{h}.{c}").as_bytes(), &sig)
            .map_err(|_| invalid())?;
        // FI permits a 60-second DPoP skew. Convert its inclusive second window to an
        // exclusive millisecond upper bound, then intersect with exact JWT expiration.
        let valid_from = nbf
            .max(proof_iat.saturating_sub(60))
            .checked_mul(1000)
            .ok_or(invalid())?;
        let valid_until = exp
            .min(proof_iat.checked_add(61).ok_or(invalid())?)
            .checked_mul(1000)
            .ok_or(invalid())?;
        if valid_from >= valid_until {
            return Err(invalid());
        }
        let (app, policy) = if android {
            &self.android_enrollment
        } else {
            &self.apple_enrollment
        };
        let pinned = match policy.platform {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256,
                ..
            }
            | KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256,
            } => attestation_root_sha256,
        };
        if hash(root) != pinned {
            return Err(invalid());
        }
        Ok(Session {
            token: Zeroizing::new(token.to_vec()),
            proof: Zeroizing::new(proof.to_vec()),
            root: root.to_vec(),
            config: iroha_core_zk::kagemusha_wallet_enrollment_v1::EnrollmentConfigV1 {
                scheme: self.scheme,
                app: app.clone(),
                policy: *policy,
                attestation_root_der: root.to_vec(),
                installation: self.installation,
                enrollment_certificate: self.enrollment_certificate,
                service_origin: selected.origin.as_bytes().to_vec(),
                fi: selected.dataspace.as_bytes().to_vec(),
                actor,
                release: selected.release.clone(),
                session_valid_from_ms: valid_from,
                session_expires_at_ms: valid_until,
            },
        })
    }
}
