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
    if method == KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity && phase == 11 {
        return count(f, 1);
    }
    if phase == 1 {
        count(f, 2)?;
        return digest(&f[1]);
    }
    check(f.len() >= 2)?;
    ticket(&f[1])?;
    if method == KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity {
        match phase {
            2 | 4 | 6 | 7 | 8 | 9 => count(f, 2),
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
    match phase {
        1 => approval_projection(method, &q[1], r),
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
                && e[3..35] == c[3..35]
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
        1 => {
            count(r, 8)?;
            ticket(&r[0])?;
            check(r[1].len() == 515)?;
            let signed =
                KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&r[1])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            let c = &signed.challenge;
            check(
                c.enrollment_id.as_slice() == q[1]
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
        validate_request(m, &f).unwrap();
        f.push(vec![7; 314]);
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
}
