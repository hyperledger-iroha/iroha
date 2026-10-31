//! Holder-side presentation bounds derived from exact private certificate and CRL DER.
//!
//! This is interval preflight, not credential proof verification. The returned
//! existing data-model type keeps private dates redacted from Debug output.
use super::{
    certificate::{ParsedCertificateV1, ParsedCrlV1, parse_certificate_v1, parse_crl_v1},
    errors::ZkX509RelationErrorV1,
};
use iroha_data_model::privacy::{
    PrivacyZkX509CertificateValidityV1, PrivacyZkX509CrlUpdateIntervalV1,
    PrivacyZkX509PresentationBoundsV1,
};
/// Bounds that every certificate of one parsed path places on the window.
pub(super) fn certificate_path_presentation_bounds_v1(
    chain: &[ParsedCertificateV1<'_>],
) -> Result<PrivacyZkX509PresentationBoundsV1, ZkX509RelationErrorV1> {
    // A fixed array keeps the private dates off the heap.
    let mut validities = [PrivacyZkX509CertificateValidityV1::new(0, 0); 3];
    if chain.len() > validities.len() {
        return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
    }
    for (validity, certificate) in validities.iter_mut().zip(chain) {
        *validity =
            PrivacyZkX509CertificateValidityV1::new(certificate.not_before, certificate.not_after);
    }
    PrivacyZkX509PresentationBoundsV1::from_certificate_path(&validities[..chain.len()])
        .map_err(|_| ZkX509RelationErrorV1::CertificateNotValid)
}
/// Bounds that the complete signed CRL places on the window.
pub(super) fn crl_presentation_bounds_v1(
    crl: &ParsedCrlV1<'_>,
) -> Result<PrivacyZkX509PresentationBoundsV1, ZkX509RelationErrorV1> {
    PrivacyZkX509PresentationBoundsV1::from_crl(PrivacyZkX509CrlUpdateIntervalV1::new(
        crl.this_update,
        crl.next_update,
    ))
    .map_err(|_| ZkX509RelationErrorV1::InvalidCrl)
}
/// Derive the admissible presentation bounds from the exact private DER.
///
/// The start of every admissible window is at least the latest `notBefore` in
/// the path and the CRL `thisUpdate`; its end is at most the earliest
/// `notAfter` in the path, strictly before the CRL `nextUpdate` and at most 300
/// seconds after `thisUpdate`. The relation admits exactly the well-shaped
/// windows inside the returned bounds, because both consume the same canonical
/// definition.
///
/// The complete closed-profile DER structure is checked before its signed
/// interval fields are projected. Signature authentication, path linkage,
/// revocation and governance binding remain the complete relation's responsibility.
///
/// This helper is available in ordinary builds. It uses the same complete
/// closed-profile DER parser as the reference relation; it does not enable
/// proof construction or establish signature, path, or governance validity.
///
/// # Errors
///
/// Rejects DER outside the closed profile, a path outside the admitted depth,
/// and signed intervals that share no presentation second.
pub fn derive_zk_x509_presentation_bounds_v1(
    certificate_chain_der: &[Vec<u8>],
    crl_der: &[u8],
) -> Result<PrivacyZkX509PresentationBoundsV1, ZkX509RelationErrorV1> {
    if !(2..=3).contains(&certificate_chain_der.len()) {
        return Err(ZkX509RelationErrorV1::WitnessMismatch);
    }
    let mut chain = Vec::with_capacity(certificate_chain_der.len());
    for certificate in certificate_chain_der {
        chain.push(parse_certificate_v1(certificate)?);
    }
    let crl = parse_crl_v1(crl_der)?;
    certificate_path_presentation_bounds_v1(&chain)?
        .intersect(crl_presentation_bounds_v1(&crl)?)
        .map_err(|_| ZkX509RelationErrorV1::CertificateNotValid)
}
