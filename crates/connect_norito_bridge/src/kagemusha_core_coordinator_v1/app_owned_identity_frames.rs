//! Data-only method19/20/21 grammar. These projections authenticate no key or capability.
//! Opaque native owners perform actual signature/policy/time/custody admission before dispatch.
use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1 as E_DOMAIN,
    KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1 as W_DOMAIN,
    KAGEMUSHA_ORDINARY_APP_ENROLLMENT_CHALLENGE_DOMAIN_V1 as C_DOMAIN, KagemushaDevicePublicKeyV1,
    KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
};
type Result<T> = std::result::Result<T, KagemushaCoreCoordinatorFrameErrorV1>;
fn check(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
    }
}
fn number(bytes: &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        KagemushaCoreCoordinatorFrameErrorV1::Field
    })?))
}
fn digest(bytes: &[u8]) -> Result<()> {
    check(bytes.len() == 32 && bytes.iter().any(|b| *b != 0))
}
fn point(bytes: &[u8]) -> Result<()> {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(bytes)
        .map(|_| ())
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)
}
fn reference(bytes: &[u8]) -> Result<()> {
    let s = std::str::from_utf8(bytes).map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    check(!s.is_empty() && s.len() <= 255 && !s.as_bytes().contains(&0))
}
fn count(fields: &[Vec<u8>], n: usize) -> Result<()> {
    check(fields.len() == n)
}
fn ticket(bytes: &[u8]) -> Result<()> {
    check(bytes.len() == 8 && bytes.iter().any(|b| *b != 0))
}
fn signing_body<'a>(bytes: &'a [u8], domain: &[u8], size: usize) -> Result<&'a [u8]> {
    check(bytes.len() == domain.len() + 8 + size && bytes.starts_with(domain))?;
    check(bytes[domain.len()..domain.len() + 8] == (size as u64).to_le_bytes())?;
    let b = &bytes[domain.len() + 8..];
    check(b[..2] == 1u16.to_le_bytes())?;
    Ok(b)
}
fn c_body(bytes: &[u8]) -> Result<&[u8]> {
    let b = signing_body(bytes, C_DOMAIN, 451)?;
    check(matches!(b[2], 1 | 2))?;
    for i in 0..13 {
        digest(&b[3 + 32 * i..3 + 32 * (i + 1)])?;
    }
    check(b[35..67] != b[67..99])?;
    for offset in [419, 427] {
        check(u64::from_le_bytes(b[offset..offset + 8].try_into().unwrap()) > 0)?;
    }
    interval(&b[435..451])?;
    Ok(b)
}
fn interval(b: &[u8]) -> Result<()> {
    let start = u64::from_le_bytes(b[..8].try_into().unwrap());
    let end = u64::from_le_bytes(b[8..16].try_into().unwrap());
    check(start > 0 && end > start && end - start <= 120000)
}
fn platform_metadata(platform: &[u8], mask: &[u8], floor: &[u8]) -> Result<()> {
    check(platform.len() == 1 && mask.len() == 1)?;
    match platform[0] {
        5 => check(matches!(mask[0], 1 | 2 | 3) && floor.is_empty()),
        4 => check(mask[0] == 0 && floor.len() == 4),
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}
fn alias(c: &[u8], platform: &[u8], key: Option<&[u8]>, original: &[u8]) -> Result<()> {
    reference(original)?;
    match platform[0] {
        5 => {
            let mut h = Sha256::new();
            h.update(b"iroha:kagemusha:v1:ordinary-app-key-alias\0");
            h.update(c);
            check(
                original
                    == format!("kagemusha-ordinary-app-v1-{}", hex::encode(h.finalize()))
                        .as_bytes(),
            )
        }
        4 => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let decoded = STANDARD
                .decode(original)
                .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            digest(&decoded)?;
            check(
                STANDARD.encode(&decoded).as_bytes() == original
                    && key.is_none_or(|k| k == decoded),
            )
        }
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}
pub(super) fn validate_request(
    method: KagemushaCoreCoordinatorMethodV1,
    f: &[Vec<u8>],
) -> Result<()> {
    check(!f.is_empty())?;
    let phase = number(&f[0])?;
    if method == KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity {
        if matches!(phase, 11 | 12) {
            return count(f, 1);
        }
        if phase == 1 {
            return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
        }
    }
    // Method19 phase8 is the distinct zero-State bootstrap entry: [LE32(8), operationID32].
    // Its response uses a separate Bootstrap-only projection; ordinary phase1 stays cash-only.
    if phase == 1
        || (method == KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval && phase == 8)
    {
        count(f, 2)?;
        return digest(&f[1]);
    }
    check(f.len() >= 2)?;
    ticket(&f[1])?;
    if method == KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity {
        match phase {
            2 | 4 | 7 | 8 | 9 | 14 => count(f, 2),
            6 => {
                count(f, 3)?;
                check(f[2].len() == 314)
            }
            13 => {
                count(f, 3)?;
                KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&f[2])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
                Ok(())
            }
            3 => {
                count(f, 3)?;
                reference(&f[2])
            }
            5 => {
                count(f, 5)?;
                point(&f[2])?;
                check(
                    !f[3].is_empty()
                        && f[3].len() <= 65536
                        && f[4].len() <= 65536
                        && (f[4].is_empty() || f[3].len() == 65536),
                )
            }
            10 => {
                count(f, 3)?;
                check(number(&f[2])? <= 1)
            }
            _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
        }
    } else {
        match phase {
            9 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession => {
                count(f, 4)?;
                check(!f[2].is_empty() && f[2].len() <= 32 * 1024)?;
                digest(&f[3])
            }
            10 | 13 | 14
                if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession =>
            {
                count(f, 2)
            }
            11 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession => {
                count(f, 3)?;
                check(f[2].len() == 64)
            }
            12 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession => {
                count(f, 3)?;
                check(!f[2].is_empty() && f[2].len() <= 16 * 1024)
            }
            8 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession => {
                count(f, 3)?;
                check(!f[2].is_empty() && f[2].len() <= 16 * 1024)
            }
            2 | 4 | 5 | 6 | 7 => count(f, 2),
            3 => {
                count(f, 3)?;
                check(!f[2].is_empty() && f[2].len() <= 4096)
            }
            _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
        }
    }
}
pub(super) fn validate_response(
    method: KagemushaCoreCoordinatorMethodV1,
    q: &[Vec<u8>],
    r: &[Vec<u8>],
) -> Result<()> {
    let phase = number(&q[0])?;
    if method == KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity {
        return c_response(phase, q, r);
    }
    if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession && phase >= 9 {
        return match phase {
            9 => {
                count(r, 5)?;
                ticket(&r[0])?;
                check(r[1] == q[2] && r[2] == q[3])?;
                digest(&r[3])?;
                digest(&r[4])
            }
            10 => {
                count(r, 2)?;
                check(r[0] == [1] && r[1].is_empty() || r[0] == [2] && r[1].len() == 64)
            }
            11 => {
                count(r, 1)?;
                check(r[0] == Sha256::digest(&q[2])[..])
            }
            12 => {
                count(r, 2)?;
                digest(&r[0])?;
                digest(&r[1])
            }
            13 => {
                count(r, 3)?;
                check(r[0].len() == 1)?;
                match r[0][0] {
                    0 | 1 => check(r[1].is_empty() && r[2].is_empty()),
                    2 => check(r[1].len() == 64 && r[2].is_empty()),
                    3 => check(r[1].len() == 64 && !r[2].is_empty() && r[2].len() <= 16 * 1024),
                    _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                }
            }
            14 => count(r, 0),
            _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
        };
    }
    match phase {
        1 => approval_projection(method, &q[1], r),
        8 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval => {
            bootstrap_approval_projection(&q[1], r)
        }
        2 | 5 => {
            count(r, 3)?;
            check(r[0].len() == 1)?;
            let tag = r[0][0];
            let (empty, raw, done) = if phase == 2 { (1, 2, 3) } else { (0, 1, 2) };
            match tag {
                t if t == empty => check(r[1].is_empty() && r[2].is_empty()),
                t if t == raw => check(!r[1].is_empty() && r[1].len() <= 4096 && r[2].is_empty()),
                t if t == done => {
                    check(!r[1].is_empty() && r[1].len() <= 4096)?;
                    receipt(method, &q[1], &r[2])?;
                    check(r[2][115..147] == Sha256::digest(&r[1])[..])
                }
                _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
            }
        }
        3 => {
            count(r, 1)?;
            check(r[0] == Sha256::digest(&q[2])[..])
        }
        4 => {
            count(r, 1)?;
            receipt(method, &q[1], &r[0])
        }
        6 => {
            count(r, 2)?;
            digest(&r[0])?;
            digest(&r[1])
        }
        8 if method == KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession => {
            count(r, 2)?;
            digest(&r[0])?;
            digest(&r[1])
        }
        7 => count(r, 0),
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}
fn approval_projection(
    method: KagemushaCoreCoordinatorMethodV1,
    id: &[u8],
    r: &[Vec<u8>],
) -> Result<()> {
    count(r, 14)?;
    ticket(&r[0])?;
    let c = c_body(&r[7])?;
    point(&r[5])?;
    digest(&r[6])?;
    check(r[6] == Sha256::digest(&r[5])[..] && r[4] == Sha256::digest(&r[7])[..])?;
    digest(&r[9])?;
    digest(&r[12])?;
    platform_metadata(&r[2], &r[11], &r[10])?;
    check(c[2] == if r[2][0] == 5 { 1 } else { 2 })?;
    alias(&r[7], &r[2], Some(&r[6]), &r[3])?;
    if method == KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval {
        digest(&r[8])?;
        let w = signing_body(&r[1], W_DOMAIN, 275)?;
        check(w[2] == 1)?;
        for i in 0..8 {
            digest(&w[3 + i * 32..3 + (i + 1) * 32])?;
        }
        interval(&w[259..275])?;
        check(
            &w[3..35] == id
                && w[67..99] == c[99..131]
                && w[99..131] == c[323..355]
                && w[131..163] == r[6]
                && w[163..195] == r[8],
        )?;
        require_app_attest_selection_subject_v1(&r[13])?;
        use KagemushaHardwareSelectionSigningLayoutV1 as S;
        check(
            r[13][S::OPERATION_TAG.start] != 0
                && r[13][S::CREDENTIAL_ID] == r[8]
                && r[13][S::HARDWARE_EPOCH_GENERATION] == c[427..435]
                && w[195..227] == Sha256::digest(&r[13])[..],
        )
    } else {
        check(r[8].is_empty() && r[13].is_empty())?;
        let e = signing_body(&r[1], E_DOMAIN, 371)?;
        check(e[2] == 1)?;
        for i in 0..11 {
            digest(&e[3 + i * 32..3 + (i + 1) * 32])?;
        }
        check(
            &e[3..35] == id
                && e[3..35] == Sha256::digest(&r[7])[..]
                && e[35..131] == c[35..131]
                && e[131..163] == c[131..163]
                && e[163..195] == c[323..355]
                && e[195..227] == c[195..227]
                && e[227..259] == c[227..259]
                && e[259..291] == c[163..195]
                && e[291..323] == r[6]
                && e[355..371] == c[435..451],
        )
    }
}
/// Separate zero-State bootstrap projection. Ordinary phase1 never accepts this subject.
/// These are only public byte correlations; the retained Native owner admits actual authority.
fn bootstrap_approval_projection(id: &[u8], r: &[Vec<u8>]) -> Result<()> {
    use KagemushaHardwareSelectionSigningLayoutV1 as S;
    count(r, 14)?;
    ticket(&r[0])?;
    let c = c_body(&r[7])?;
    point(&r[5])?;
    digest(&r[6])?;
    check(r[6] == Sha256::digest(&r[5])[..] && r[4] == Sha256::digest(&r[7])[..])?;
    digest(&r[8])?;
    digest(&r[9])?;
    digest(&r[12])?;
    platform_metadata(&r[2], &r[11], &r[10])?;
    check(c[2] == if r[2][0] == 5 { 1 } else { 2 })?;
    alias(&r[7], &r[2], Some(&r[6]), &r[3])?;
    let w = signing_body(&r[1], W_DOMAIN, 275)?;
    check(w[2] == 1)?;
    for i in 0..8 {
        digest(&w[3 + i * 32..3 + (i + 1) * 32])?;
    }
    interval(&w[259..275])?;
    require_app_attest_selection_subject_v1(&r[13])?;
    // The model-owned S grammar above enforces zero bootstrap indexes and zero outgoing
    // commitments. Neither a valid cash S nor a differently scoped C can enter phase8.
    check(
        &w[3..35] == id
            && w[67..99] == c[99..131]
            && w[99..131] == c[323..355]
            && w[131..163] == r[6]
            && w[163..195] == r[8]
            && w[195..227] == Sha256::digest(&r[13])[..]
            && r[13][S::OPERATION_TAG.start] == 0
            && r[13][S::CREDENTIAL_ID] == r[8]
            && r[13][S::RELEASE_ID] == c[195..227]
            && r[13][S::NETWORK_ID] == c[131..163]
            && r[13][S::LANE_COMMITMENT] == c[163..195]
            && r[13][S::HARDWARE_PROFILE_ID] == c[227..259]
            && r[13][S::POLICY_EPOCH] == c[419..427]
            && r[13][S::HARDWARE_EPOCH_GENERATION] == c[427..435],
    )
}
fn receipt(method: KagemushaCoreCoordinatorMethodV1, ticket: &[u8], r: &[u8]) -> Result<()> {
    check(
        r.len() == 184
            && &r[..8] == b"KGMAPP1\0"
            && r[8..10] == 1u16.to_le_bytes()
            && r[10]
                == if method == KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval {
                    1
                } else {
                    2
                }
            && &r[11..19] == ticket,
    )?;
    for i in 0..5 {
        digest(&r[19 + 32 * i..19 + 32 * (i + 1)])?;
    }
    check((r[179] == 0 && r[180..184] == [0; 4]) || (r[179] == 1 && r[180..184] != [0; 4]))
}
fn raw_metadata(point_bytes: &[u8], hash: &[u8], len: &[u8], present: bool) -> Result<()> {
    let total = number(len)?;
    if present {
        point(point_bytes)?;
        digest(hash)?;
        check(total > 0 && total <= 131072)
    } else {
        check(point_bytes.is_empty() && hash.is_empty() && total == 0)
    }
}
fn c_response(phase: u32, q: &[Vec<u8>], r: &[Vec<u8>]) -> Result<()> {
    match phase {
        11 => {
            count(r, 1)?;
            digest(&r[0])
        }
        12 => {
            count(r, 8)?;
            ticket(&r[0])?;
            let account = std::str::from_utf8(&r[1])
                .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            check(
                !account.is_empty() && account.len() <= 2048 && !account.as_bytes().contains(&0),
            )?;
            for i in 2..7 {
                digest(&r[i])?;
            }
            let mut uuid = r[2][..16].to_vec();
            uuid[6] = (uuid[6] & 0x0f) | 0x40;
            uuid[8] = (uuid[8] & 0x3f) | 0x80;
            let u = hex::encode(uuid);
            check(
                r[7] == format!(
                    "{}-{}-{}-{}-{}",
                    &u[..8],
                    &u[8..12],
                    &u[12..16],
                    &u[16..20],
                    &u[20..]
                )
                .as_bytes(),
            )
        }
        14 => {
            count(r, 1)?;
            check(r[0].len() <= 16 * 1024)
        }
        13 => {
            count(r, 8)?;
            ticket(&r[0])?;
            check(r[1].len() == 515)?;
            let signed =
                KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&r[1])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            let c = &signed.challenge;
            check(
                r[1] == q[2]
                    && c.canonical_signing_bytes()
                        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?
                        == r[2]
                    && c.attestation_challenge()
                        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?
                        .as_slice()
                        == r[3],
            )?;
            let b = c_body(&r[2])?;
            check(r[4] == vec![if b[2] == 1 { 5 } else { 4 }] && r[6].len() == 1)?;
            digest(&r[7])?;
            if r[4][0] == 5 {
                check(matches!(r[6][0], 1 | 2 | 3))?;
                alias(&r[2], &r[4], None, &r[5])
            } else {
                check(r[5].is_empty() && r[6] == [0])
            }
        }
        2 => {
            count(r, 2)?;
            check(r[0].len() == 1)?;
            match r[0][0] {
                1 => check(r[1].is_empty()),
                2 => reference(&r[1]),
                _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
            }
        }
        3 => {
            count(r, 1)?;
            check(r[0] == Sha256::digest(&q[2])[..])
        }
        4 => {
            count(r, 4)?;
            check(r[0].len() == 1 && matches!(r[0][0], 1 | 2))?;
            if r[0][0] == 1 {
                check(r[1].is_empty() && r[2].is_empty() && r[3].is_empty())
            } else {
                raw_metadata(&r[1], &r[2], &r[3], true)
            }
        }
        5 => {
            count(r, 2)?;
            point(&q[2])?;
            let mut h = Sha256::new();
            h.update(&q[3]);
            h.update(&q[4]);
            check(r[0] == h.finalize()[..] && r[1] == Sha256::digest(&q[2])[..])
        }
        6 => {
            count(r, 2)?;
            digest(&r[0])?;
            digest(&r[1])
        }
        7 => {
            count(r, 7)?;
            check(r[0].len() == 1 && r[0][0] <= 5)?;
            let state = r[0][0];
            if state < 2 {
                check(r[1].is_empty())?;
            } else {
                reference(&r[1])?;
            }
            raw_metadata(&r[2], &r[3], &r[4], state >= 4)?;
            if state == 5 {
                check(r[5].len() == 314)?;
                digest(&r[6])?;
            } else {
                check(r[5].is_empty() && r[6].is_empty())?;
            }
            Ok(())
        }
        8 => {
            count(r, 2)?;
            digest(&r[0])?;
            digest(&r[1])
        }
        9 => count(r, 0),
        10 => {
            count(r, 4)?;
            check(r[0] == q[2])?;
            digest(&r[2])?;
            let size = number(&r[3])? as usize;
            let index = number(&r[0])? as usize;
            check(
                size > 0
                    && size <= 131072
                    && index * 65536 < size
                    && r[1].len() == (size - index * 65536).min(65536),
            )
        }
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn signed_approval_projection(
        fixture: &iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
        challenge: iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1,
        apple: bool,
    ) -> Vec<Vec<u8>> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use iroha_data_model::kagemusha::{
            KagemushaAppOperationApprovalEvidenceV1 as Evidence, KagemushaAppOperationApprovalV1,
            kagemusha_ordinary_android_app_key_alias_v1,
        };
        use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
        let enrollment = fixture.verify(300).unwrap();
        let credential = enrollment.app_credential();
        let subject = credential.subject();
        let c = &fixture.selection.preparation.challenge;
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let message = challenge.canonical_signing_bytes().unwrap();
        // Maintained known-public fixture key and actual production model equation.
        // This signs only a public codec sample; it creates no monetary or device authority.
        let evidence = if apple {
            let mut auth = [0; 37];
            auth[..32].copy_from_slice(&subject.app_signing_identity_digest);
            auth[32] = 0x40;
            auth[33..].copy_from_slice(&17u32.to_be_bytes());
            let mut nonce = Sha256::new();
            nonce.update(auth);
            nonce.update(Sha256::digest(&message));
            let signature: Signature = key.sign(&nonce.finalize());
            let der = signature.to_der();
            let mut raw = vec![0xa2, 0x69];
            raw.extend_from_slice(b"signature");
            raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
            raw.extend_from_slice(der.as_bytes());
            raw.push(0x71);
            raw.extend_from_slice(b"authenticatorData");
            raw.extend_from_slice(&[0x58, 37]);
            raw.extend(auth);
            Evidence::AppleAppAttest { raw_assertion: raw }
        } else {
            let signature: Signature = key.sign(&message);
            Evidence::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            }
        };
        let approval = KagemushaAppOperationApprovalV1 {
            challenge,
            evidence,
        };
        let floor = enrollment.possession().app_attest_counter();
        let admitted = approval
            .authenticate(&challenge, credential, floor, 301)
            .unwrap();
        assert_eq!(
            admitted.app_attest_counter(),
            if apple { Some(17) } else { None }
        );
        vec![
            1u64.to_le_bytes().to_vec(),
            message,
            vec![if apple { 4 } else { 5 }],
            if apple {
                STANDARD.encode(subject.attested_key_id).into_bytes()
            } else {
                kagemusha_ordinary_android_app_key_alias_v1(c)
                    .unwrap()
                    .into_bytes()
            },
            c.attestation_challenge().unwrap().to_vec(),
            subject.app_public_key.as_sec1_bytes().to_vec(),
            subject.attested_key_id.to_vec(),
            c.canonical_signing_bytes().unwrap(),
            credential.digest().to_vec(),
            vec![92; 32],
            floor.map_or_else(Vec::new, |n| n.to_le_bytes().to_vec()),
            vec![if apple { 0 } else { 1 }],
            subject.app_signing_identity_digest.to_vec(),
            challenge.canonical_subject_signing_bytes().unwrap(),
        ]
    }

    #[test]
    fn bootstrap_phase_eight_accepts_signed_native_zero_state_and_rejects_cash_subjects() {
        use iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryLogicalApprovalJournalV1 as Journal;
        use iroha_data_model::{
            kagemusha::{KagemushaHardwareSelectionSigningLayoutV1 as S, KagemushaOperationKindV1},
            testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture,
        };
        let method = KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval;
        for apple in [false, true] {
            let fixture = Fixture::new(apple);
            let enrollment = fixture.verify(300).unwrap();
            let challenge = Journal::test_only_bootstrap_challenge_v1(
                &enrollment,
                fixture.release.clone(),
                [43; 32],
                [44; 32],
                [45; 32],
                300,
            )
            .unwrap();
            assert_eq!(
                challenge.subject.operation_kind,
                KagemushaOperationKindV1::Bootstrap
            );
            let projection = signed_approval_projection(&fixture, challenge, apple);
            let q = vec![8u32.to_le_bytes().to_vec(), challenge.operation_id.to_vec()];
            let request = kagemusha_core_coordinator_encode_request_v1(&q).unwrap();
            let response = kagemusha_core_coordinator_encode_response_v1(&projection).unwrap();
            kagemusha_core_coordinator_validate_method_request_v1(method, &request).unwrap();
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response)
                .unwrap();
            let ordinary = vec![1u32.to_le_bytes().to_vec(), challenge.operation_id.to_vec()];
            validate_request(method, &ordinary).unwrap();
            assert!(validate_response(method, &ordinary, &projection).is_err());

            // A separately model-authenticated cash sample remains ordinary-only, even with
            // matching credential, W/S digest and original C scope. It is no financial owner.
            let mut cash = challenge;
            cash.subject.operation_kind = KagemushaOperationKindV1::Rotate;
            cash.subject.secure_index_after = 1;
            cash.subject_signing_digest =
                Sha256::digest(cash.canonical_subject_signing_bytes().unwrap()).into();
            let cash_projection = signed_approval_projection(&fixture, cash, apple);
            validate_response(method, &ordinary, &cash_projection).unwrap();
            assert!(validate_response(method, &q, &cash_projection).is_err());

            // Rehashing S into W cannot hide mixed C scope or a nonzero bootstrap index.
            for range in [
                S::RELEASE_ID,
                S::NETWORK_ID,
                S::LANE_COMMITMENT,
                S::HARDWARE_PROFILE_ID,
                S::POLICY_EPOCH,
                S::HARDWARE_EPOCH_GENERATION,
                S::CREDENTIAL_ID,
                S::SECURE_INDEX_AFTER,
                S::CANDIDATE_ENVELOPE_DIGEST,
                S::TERMINAL_BODY_COMMITMENT,
            ] {
                let mut changed = projection.clone();
                changed[13][range.start] ^= 1;
                let hash = Sha256::digest(&changed[13]);
                let start = W_DOMAIN.len() + 8;
                changed[1][start + 195..start + 227].copy_from_slice(&hash);
                assert!(validate_response(method, &q, &changed).is_err());
            }
            let mut wrong_purpose = projection.clone();
            wrong_purpose[1][W_DOMAIN.len() + 8 + 2] = 2;
            assert!(validate_response(method, &q, &wrong_purpose).is_err());
            let wrong_id = vec![8u32.to_le_bytes().to_vec(), vec![46; 32]];
            assert!(validate_response(method, &wrong_id, &projection).is_err());
            for invalid in [
                vec![8u32.to_le_bytes().to_vec(), vec![0; 32]],
                vec![8u32.to_le_bytes().to_vec(), 1u64.to_le_bytes().to_vec()],
                vec![
                    8u32.to_le_bytes().to_vec(),
                    challenge.operation_id.to_vec(),
                    vec![],
                ],
            ] {
                assert!(validate_request(method, &invalid).is_err());
            }
            assert!(
                validate_request(
                    KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession,
                    &q
                )
                .is_err()
            );
        }
    }
    #[test]
    fn bootstrap_approval_projection_requires_zero_indexes_and_outgoing_slots() {
        use iroha_data_model::kagemusha::{
            KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
            KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
            kagemusha_ordinary_android_app_key_alias_v1,
        };
        use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
        // Public framing only. This fixture cannot capture an approval or publish a State.
        let fixture = Fixture::new(false);
        let c = &fixture.selection.preparation.challenge;
        let credential = &fixture.selection.issuance.credential;
        let digest = credential.canonical_digest().unwrap();
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: c.release_id,
            provider_policy_root: [21; 32],
            app_policy_digest: [22; 32],
            credential_id: digest,
            network_id: fixture.release.network_id(),
            lane_commitment: c.lane_id,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
            hardware_epoch_id: [24; 32],
            hardware_epoch_generation: c.hardware_epoch,
            operation_kind: KagemushaOperationKindV1::Bootstrap,
            transition_statement_digest: [25; 32],
            candidate_envelope_digest: [0; 32],
            terminal_body_commitment: [0; 32],
            secure_index_before: 0,
            secure_index_after: 0,
        };
        let original_s = subject.canonical_signing_bytes().unwrap();
        let w = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
            operation_id: [26; 32],
            nonce: [27; 32],
            account_binding: c.account_binding,
            authority_policy_digest: c.app_authority_policy_digest,
            attested_key_id: credential.subject.attested_key_id,
            enrollment_digest: digest,
            subject_signing_digest: Sha256::digest(&original_s).into(),
            normalized_guard_digest: [28; 32],
            issued_at_ms: 300,
            expires_at_ms: 400,
            subject,
        };
        let original_c = c.canonical_signing_bytes().unwrap();
        let projection = vec![
            1u64.to_le_bytes().to_vec(),
            w.canonical_signing_bytes().unwrap(),
            vec![5],
            kagemusha_ordinary_android_app_key_alias_v1(c)
                .unwrap()
                .into_bytes(),
            Sha256::digest(&original_c).to_vec(),
            credential.subject.app_public_key.as_sec1_bytes().to_vec(),
            credential.subject.attested_key_id.to_vec(),
            original_c,
            digest.to_vec(),
            [29; 32].to_vec(),
            vec![],
            vec![3],
            credential.subject.app_signing_identity_digest.to_vec(),
            original_s,
        ];
        let method = KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval;
        bootstrap_approval_projection(&w.operation_id, &projection).unwrap();
        assert!(approval_projection(method, &w.operation_id, &projection).is_err());
        use KagemushaHardwareSelectionSigningLayoutV1 as S;
        for offset in [
            S::SECURE_INDEX_BEFORE.start,
            S::SECURE_INDEX_AFTER.start,
            S::CANDIDATE_ENVELOPE_DIGEST.start,
            S::TERMINAL_BODY_COMMITMENT.start,
            S::OPERATION_TAG.start,
        ] {
            let mut changed = projection.clone();
            changed[13][offset] = 1;
            let start = W_DOMAIN.len() + 8 + 195;
            let subject_digest = Sha256::digest(&changed[13]);
            changed[1][start..start + 32].copy_from_slice(&subject_digest);
            assert!(bootstrap_approval_projection(&w.operation_id, &changed).is_err());
        }
    }
    #[test]
    fn possession_projection_binds_full_c_digest_and_rejects_retired_selectors() {
        use iroha_data_model::kagemusha::{
            KagemushaAppEnrollmentPossessionChallengeV1,
            kagemusha_ordinary_android_app_key_alias_v1,
        };
        use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
        // This is exact public wire correlation, never a native or device admission fixture.
        let f = Fixture::new(false);
        let c = &f.selection.preparation.challenge;
        let key = &f.selection.issuance.credential.subject.app_public_key;
        let original_c = c.canonical_signing_bytes().unwrap();
        let e =
            KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(c, key, [91; 32])
                .unwrap();
        let mut projection = vec![
            1u64.to_le_bytes().to_vec(),
            e.canonical_signing_bytes().unwrap(),
            vec![5],
            kagemusha_ordinary_android_app_key_alias_v1(c)
                .unwrap()
                .into_bytes(),
            Sha256::digest(&original_c).to_vec(),
            key.as_sec1_bytes().to_vec(),
            Sha256::digest(key.as_sec1_bytes()).to_vec(),
            original_c,
            vec![],
            vec![92; 32],
            vec![],
            vec![3],
            vec![93; 32],
            vec![],
        ];
        let method = KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession;
        let selector = c.attestation_challenge().unwrap();
        approval_projection(method, &selector, &projection).unwrap();
        assert!(approval_projection(method, &c.enrollment_id, &projection).is_err());
        assert!(approval_projection(method, &Sha256::digest(selector), &projection).is_err());
        let mut retired_id_message = projection.clone();
        let first = E_DOMAIN.len() + 8 + 3;
        retired_id_message[1][first..first + 32].copy_from_slice(&c.enrollment_id);
        assert!(approval_projection(method, &selector, &retired_id_message).is_err());
        // A C-only epoch change cannot hide behind the otherwise identical explicit E fields.
        let mut changed = c.clone();
        changed.hardware_epoch += 1;
        projection[7] = changed.canonical_signing_bytes().unwrap();
        projection[4] = Sha256::digest(&projection[7]).to_vec();
        projection[3] = kagemusha_ordinary_android_app_key_alias_v1(&changed)
            .unwrap()
            .into_bytes();
        assert!(
            approval_projection(
                method,
                &changed.attestation_challenge().unwrap(),
                &projection
            )
            .is_err()
        );
    }
    #[test]
    fn identity_original_selector_read_has_no_caller_identity_or_side_effect_field() {
        let m = KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity;
        let mut q = vec![11u32.to_le_bytes().to_vec()];
        validate_request(m, &q).unwrap();
        validate_response(m, &q, &[vec![1; 32]]).unwrap();
        assert!(validate_response(m, &q, &[vec![0; 32]]).is_err());
        q.push(vec![1; 32]);
        assert!(validate_request(m, &q).is_err());
    }
    #[test]
    fn identity_chunks_fit_original_request_and_response_caps() {
        let q = vec![
            5u32.to_le_bytes().to_vec(),
            1u64.to_le_bytes().to_vec(),
            vec![4; 65],
            vec![1; 65536],
            vec![2; 65536],
        ];
        let frame = super::super::kagemusha_core_coordinator_encode_request_v1(&q).unwrap();
        assert_eq!(frame.len(), 131185);
        assert!(frame.len() < KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1);
        let r = vec![
            0u32.to_le_bytes().to_vec(),
            vec![1; 65536],
            vec![1; 32],
            131072u32.to_le_bytes().to_vec(),
        ];
        let bytes = super::super::kagemusha_core_coordinator_encode_response_v1(&r).unwrap();
        assert_eq!(bytes.len(), 65608);
        assert!(bytes.len() < KAGEMUSHA_CORE_COORDINATOR_MAX_RESPONSE_BYTES_V1);
    }
    #[test]
    fn identity_phase_six_has_no_app_admission_or_verdict() {
        let m = KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity;
        let mut f = vec![6u32.to_le_bytes().to_vec(), 1u64.to_le_bytes().to_vec()];
        assert!(validate_request(m, &f).is_err());
        f.push(vec![7; 314]);
        // These are untrusted exact-width issuer originals, never an app admission verdict.
        validate_request(m, &f).unwrap();
        f[2].pop();
        assert!(validate_request(m, &f).is_err());
    }
    #[test]
    fn identity_chunk_indices_and_boundaries_are_closed() {
        let m = KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity;
        let q = vec![
            10u32.to_le_bytes().to_vec(),
            1u64.to_le_bytes().to_vec(),
            1u32.to_le_bytes().to_vec(),
        ];
        let r = vec![
            1u32.to_le_bytes().to_vec(),
            vec![1],
            vec![1; 32],
            65537u32.to_le_bytes().to_vec(),
        ];
        validate_response(m, &q, &r).unwrap();
        let mut changed = r;
        changed[1] = vec![];
        assert!(validate_response(m, &q, &changed).is_err());
    }
    #[test]
    fn final_identity_intake_is_e20_only_and_keeps_legacy_field_limits() {
        let ticket = 7u64.to_le_bytes().to_vec();
        let request = vec![
            8u32.to_le_bytes().to_vec(),
            ticket.clone(),
            vec![23; 16 * 1024],
        ];
        let e = KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession;
        assert!(validate_request(e, &request).is_ok());
        assert!(
            validate_request(
                KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval,
                &request
            )
            .is_err()
        );
        assert!(validate_request(e, &request[..2]).is_err());
        let mut oversized = request.clone();
        oversized[2].push(23);
        assert!(validate_request(e, &oversized).is_err());
        assert!(validate_response(e, &request, &[vec![11; 32], vec![12; 32]]).is_ok());
        assert!(validate_response(e, &request, &[vec![11; 32]]).is_err());
        assert!(validate_response(e, &request, &[vec![0; 32], vec![12; 32]]).is_err());
        // Framing/shape only: these arbitrary archive bytes establish no identity or signature.
        let q = super::super::kagemusha_core_coordinator_encode_request_v1(&request).unwrap();
        assert!(q.len() <= KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1);
    }
}
