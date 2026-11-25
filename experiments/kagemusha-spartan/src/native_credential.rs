//! Shared enrollment-field validation and the signed Spartan key-table extension.
use super::*;
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaWalletCredentialBodyV1, KagemushaWalletEvidenceKindV1,
    KagemushaWalletEvidenceV1, KagemushaWalletRegulatoryPolicyV1,
};

/// Credential-only view of a release-selected Spartan installation. This owns
/// public table bases and a scheme root. It has no signing or monetary capability.
/// A service constructs it once when admitting its signed release and shares it
/// across enrollment requests.
#[derive(Clone)]
pub struct CredentialIssuerParameters {
    scheme: [u8; 32],
    root: p256::Point,
    table: Arc<Gens<Eq>>,
}

impl CredentialIssuerParameters {
    /// Construct from the manifest already authenticated by the signed release
    /// and scheme Artifact signer. Recompute the entire published generator
    /// profile before retaining only the key table bases needed by the issuer.
    pub fn from_manifest(
        manifest: &ParameterManifest,
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
    ) -> Result<Self> {
        scheme
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let (primary, secondary) = crate::native::built_in_producer_programs()
            .ok_or(Error::Unavailable("qualified Spartan compiler profile"))?;
        let network: iroha_data_model::NetworkId = manifest
            .finality_network
            .parse()
            .map_err(|_| Error::Malformed("issuer genesis"))?;
        if manifest.version != PARAMETER_MANIFEST_VERSION
            || manifest.scheme != scheme.scheme_id()
            || manifest.relation != scheme.relation_id
            || network.as_bytes() != &scheme.network_id
            || network.to_string() != manifest.finality_network
            || manifest.primary != primary
            || manifest.secondary != secondary
        {
            return Err(Error::AuthorityMismatch("installed issuer profile"));
        }
        // These are the public capacities of the closed p18-s17 producer.
        // Never allocate a caller-chosen power of two.
        let (gp, gs) = authenticate_profile(manifest, [18, 17])?;
        drop(gs);
        Ok(Self {
            scheme: manifest.scheme,
            root: super::super::point(scheme.scheme_root_key.as_ref())?,
            table: Arc::new(gp.prefix(full_table_bases())),
        })
    }

    /// Derive the issuer's payment-key table from the authenticated five-kind
    /// generation and the capacities independently selected by this build.
    pub fn from_five_manifest(
        header: &crate::native::AuthenticatedManifestHeaderV5,
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
    ) -> Result<Self> {
        let source = crate::native::FIVE_KIND_PRODUCER
            .as_ref()
            .ok_or(Error::Unavailable("qualified five-kind compiler profile"))?;
        Self::from_selected_five_manifest(
            header,
            scheme,
            &source.programs,
            source.generator_capacity_bits,
        )
    }

    pub(in crate::native) fn from_selected_five_manifest(
        header: &crate::native::AuthenticatedManifestHeaderV5,
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        programs: &crate::native::ProgramProfileV2,
        capacities: [usize; 2],
    ) -> Result<Self> {
        scheme
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let network: iroha_data_model::NetworkId = header
            .finality_network
            .parse()
            .map_err(|_| Error::Malformed("issuer genesis"))?;
        if header.programs != *programs
            || header.scheme != scheme.scheme_id()
            || header.relation != scheme.relation_id
            || network.as_bytes() != &scheme.network_id
            || network.to_string() != header.finality_network
            || header.generator_capacity_bits.map(|n| n as usize) != capacities
        {
            return Err(Error::AuthorityMismatch(
                "installed five-kind issuer profile",
            ));
        }
        let [p, s] = capacities.map(|n| 1usize << n);
        let generators = (Gens::new(p), Gens::new(s));
        header.verify_generators(&generators)?;
        let (gp, _) = generators;
        Ok(Self {
            scheme: header.scheme,
            root: super::super::point(scheme.scheme_root_key.as_ref())?,
            table: Arc::new(gp.prefix(full_table_bases())),
        })
    }

    /// Build the exact extended credential body for the enrolled payment key.
    pub fn credential_body_for_issuance(
        &self,
        fields: &KagemushaWalletCredentialBodyV1,
    ) -> Result<[u8; credential::BODY_BYTES]> {
        if self.scheme != fields.scheme_id {
            return Err(Error::AuthorityMismatch("issuer credential scheme"));
        }
        issuer_body(&self.table, fields)
    }

    /// Verify the exact signed credential, complete signer certificate,
    /// enrolled hardware key table and canonical enrollment policy fields.
    pub fn authenticate_credential_originals(
        &self,
        bytes: &[u8],
        certificate: &[u8],
    ) -> Result<Credential> {
        let credential = read_credential(bytes)?;
        let cert = read_certificate(certificate)?;
        authenticate_credential_under(&self.scheme, &self.root, &self.table, &credential, &cert)?;
        credential.enrollment_fields()?;
        Ok(credential)
    }
}

/// The exact 32-byte protocol message for ECDSA-P256-SHA256 credential issuance.
pub fn credential_signing_message_for_body(body: &[u8; credential::BODY_BYTES]) -> [u8; 32] {
    credential::signing_body(body).to_repr()
}

impl Credential {
    /// Operation ID fixed by the enrollment, independent of fresh state blinding.
    pub fn bootstrap_operation_id(&self) -> [u8; 32] {
        let limb = |offset| {
            crate::certificate::fp(&BigUint::from_bytes_le(&self.body[offset..offset + 16]))
        };
        iroha_pasta::poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &[limb(66), limb(82), Fp::from(1), limb(448), limb(464)],
        )
        .to_repr()
    }

    /// Protocol signing message of this exact original, independent of its signature.
    pub fn signing_message_bytes(&self) -> [u8; 32] {
        credential_signing_message_for_body(&self.body)
    }

    /// Reconstruct and validate the enrollment fields shared with the wallet protocol.
    /// The issuer signature and the installed generator table are checked separately.
    pub fn enrollment_fields(&self) -> Result<KagemushaWalletCredentialBodyV1> {
        let b = &self.body;
        fn evidence(b: &[u8; 56]) -> KagemushaWalletEvidenceV1 {
            KagemushaWalletEvidenceV1 {
                digest: b[..32].try_into().unwrap(),
                time_ms: u64::from_le_bytes(b[32..40].try_into().unwrap()),
                facts: u32::from_le_bytes(b[40..44].try_into().unwrap()),
                os_patch_level: u32::from_le_bytes(b[44..48].try_into().unwrap()),
                vendor_patch_level: u32::from_le_bytes(b[48..52].try_into().unwrap()),
                boot_patch_level: u32::from_le_bytes(b[52..56].try_into().unwrap()),
            }
        }
        let fields = KagemushaWalletCredentialBodyV1 {
            version: u16::from_le_bytes(b[..2].try_into().unwrap()),
            scheme_id: b[2..34].try_into().unwrap(),
            asset_digest: b[34..66].try_into().unwrap(),
            wallet_id: b[66..98].try_into().unwrap(),
            account_digest: b[98..130].try_into().unwrap(),
            payment_key: KagemushaDevicePublicKeyV1::try_from(&b[130..195])
                .map_err(|_| Error::Malformed("credential payment key"))?,
            provider_contract: b[259..291].try_into().unwrap(),
            evidence_kind: KagemushaWalletEvidenceKindV1::ALL
                .into_iter()
                .find(|k| k.tag() == b[291])
                .ok_or(Error::Malformed("credential evidence kind"))?,
            enrollment_evidence: evidence(b[292..348].try_into().unwrap()),
            fresh_evidence: evidence(b[348..404].try_into().unwrap()),
            app_policy: b[404..436].try_into().unwrap(),
            regulatory_policy: KagemushaWalletRegulatoryPolicyV1 {
                permitted_controls: u32::from_le_bytes(b[436..440].try_into().unwrap()),
                time_anchor_max_response_ms: u64::from_le_bytes(b[440..448].try_into().unwrap()),
            },
            enrollment_id: b[448..480].try_into().unwrap(),
            issued_at_ms: u64::from_le_bytes(b[480..488].try_into().unwrap()),
            renewal_sequence: u32::from_le_bytes(b[488..492].try_into().unwrap()),
            lease_expires_at_ms: u64::from_le_bytes(b[492..500].try_into().unwrap()),
            issuer_certificate: b[500..532].try_into().unwrap(),
        };
        fields
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        Ok(fields)
    }
}

impl AuthenticatedParameters {
    /// Exact unsigned 532-byte issuer body for a validated set of enrollment fields.
    /// The caller's Enrollment-role signer signs the credential signing message.
    pub fn credential_body_for_issuance(
        &self,
        fields: &KagemushaWalletCredentialBodyV1,
    ) -> Result<[u8; credential::BODY_BYTES]> {
        if fields.scheme_id != self.inner.scheme {
            return Err(Error::AuthorityMismatch("credential issuance scheme"));
        }
        issuer_body(&self.inner.generators.p, fields)
    }
}

fn issuer_body(
    generators: &Gens<Eq>,
    fields: &KagemushaWalletCredentialBodyV1,
) -> Result<[u8; credential::BODY_BYTES]> {
    fields
        .validate()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let common = fields.transcript();
    let table = payment_key_table(generators, fields.payment_key.as_ref())?;
    let mut body = [0; credential::BODY_BYTES];
    body[..195].copy_from_slice(&common[..195]);
    body[195..227].copy_from_slice(&table[0]);
    body[227..259].copy_from_slice(&table[1]);
    body[259..].copy_from_slice(&common[195..]);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_and_wallet_share_exact_credential_fields_and_table() {
        let generators = Gens::<Eq>::new(1 << 12);
        let chain = crate::chain_fixture::new(&generators);
        let input = &chain.sender_bootstrap;
        let fields = input.credential.enrollment_fields().unwrap();
        let issuer = CredentialIssuerParameters {
            scheme: fields.scheme_id,
            root: chain.sender_context.root.root.clone(),
            table: Arc::new(generators.prefix(full_table_bases())),
        };
        assert_eq!(
            issuer.credential_body_for_issuance(&fields).unwrap(),
            input.credential.body
        );
        assert_eq!(
            input.credential.signing_message_bytes(),
            credential::message(&input.credential).to_repr()
        );
        assert_eq!(
            issuer_body(&generators, &fields).unwrap(),
            input.credential.body
        );
        let mut wrong = input.credential.clone();
        wrong.body[292..324].fill(0);
        wrong.signature = bootstrap::sign(
            credential::message(&wrong),
            &BigUint::from_bytes_be(&[0x26; 32]),
            "credential test issuer",
        );
        assert!(wrong.enrollment_fields().is_err());
        // Previously issued heads and incoming proofs keep the relation's own
        // credential predicate. Stronger issuance policy runs before selection.
        assert!(
            authenticate_credential_under(
                &fields.scheme_id,
                &chain.sender_context.root.root,
                &generators,
                &wrong,
                &input.certificate
            )
            .is_ok()
        );
    }
}
