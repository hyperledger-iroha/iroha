//! Canonical artifact replay for the existing ordinary mathematical qualification corridor.
//!
//! Generated data is checked against the retained original protocol, then its decoded
//! parameters and standalone VK are used to verify the original proof. This grants no
//! installed release, Native wallet, physical-device or monetary authority.
//! TODO: Run the complete current-candidate corridor under its unchanged native resource
//! guard; artifact roundtrips alone do not establish circuit or device qualification.

use super::*;
use halo2_base::utils::BigPrimeField;

/// Borrow the exact emitted helper artifacts and the separately retained native protocol.
pub(super) struct Originals<'a, C: CurveAffine> {
    pub(super) parameters: &'a [u8],
    pub(super) proving_key: &'a [u8],
    pub(super) verifying_key: &'a [u8],
    pub(super) proving_role: KagemushaArtifactRoleV1,
    pub(super) verifying_role: KagemushaArtifactRoleV1,
    pub(super) protocol: &'a PlonkProtocol<C>,
}

/// Read canonical parameters only after exact fixed-SRS identity has been established.
/// In particular, an offered `k` cannot control the unchecked parameter reader's allocation.
fn read_parameters<C>(
    bytes: &[u8],
    canonical: &ParamsIPA<C>,
    parity: KagemushaPastaParityV1,
) -> Result<ParamsIPA<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
{
    validate_length(
        parity,
        "qualification parameters",
        bytes.len(),
        KAGEMUSHA_PARAMS_BYTES_V1,
        true,
    )?;
    if canonical.k() != KAGEMUSHA_HALO2_K_V1 {
        return Err(key_decode_message(
            parity,
            "qualification parameters",
            "canonical parameter domain differs",
        ));
    }
    let role = match parity {
        KagemushaPastaParityV1::Eq => KagemushaArtifactRoleV1::ParamsEq,
        KagemushaPastaParityV1::Ep => KagemushaArtifactRoleV1::ParamsEp,
    };
    let original = binding(role, bytes);
    let mut writer = CanonicalArtifactDigestWriterV1::new(KAGEMUSHA_PARAMS_BYTES_V1);
    canonical
        .write(&mut writer)
        .map_err(|e| key_decode_error(parity, "qualification parameters", e))?;
    if !writer.matches(original) {
        return Err(key_decode_message(
            parity,
            "qualification parameters",
            "original differs from canonical fixed SRS",
        ));
    }
    let mut cursor = Cursor::new(bytes);
    let decoded = ParamsIPA::<C>::read(&mut cursor)
        .map_err(|e| key_decode_error(parity, "qualification parameters", e))?;
    ensure_cursor_consumed(parity, "qualification parameters", &cursor, bytes.len())?;
    let mut writer = CanonicalArtifactDigestWriterV1::new(KAGEMUSHA_PARAMS_BYTES_V1);
    decoded
        .write(&mut writer)
        .map_err(|e| key_decode_error(parity, "qualification parameters", e))?;
    if decoded.k() != KAGEMUSHA_HALO2_K_V1 || !writer.matches(original) {
        return Err(key_decode_message(
            parity,
            "qualification parameters",
            "decoded parameter roundtrip differs",
        ));
    }
    Ok(decoded)
}

/// Exercise the shipping checked PK/VK codecs, exact input consumption, embedded VK
/// equality and canonical protocol identity under the original finite helper limits.
/// This is a data roundtrip, not authentication of the generated fixture's manifest.
pub(super) fn roundtrip<C, ConcreteCircuit>(
    originals: Originals<'_, C>,
    canonical: &ParamsIPA<C>,
    circuit_params: <ConcreteCircuit as halo2_proofs::plonk::Circuit<C::ScalarExt>>::Params,
    width: usize,
) -> Result<(ParamsIPA<C>, PlonkProtocol<C>), KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::SerdeCurveAffine,
    C::ScalarExt: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64> + BigPrimeField,
    ConcreteCircuit: halo2_proofs::plonk::Circuit<C::ScalarExt>,
    <ConcreteCircuit as halo2_proofs::plonk::Circuit<C::ScalarExt>>::Params: Clone,
{
    use super::super::artifacts::{KagemushaArtifactDescriptorV1, KagemushaArtifactKindV1};
    let pk_role = KagemushaArtifactDescriptorV1::for_role(originals.proving_role);
    let vk_role = KagemushaArtifactDescriptorV1::for_role(originals.verifying_role);
    let parity = pk_role.parity;
    if pk_role.kind != KagemushaArtifactKindV1::ProvingKey
        || vk_role.kind != KagemushaArtifactKindV1::VerifyingKey
        || vk_role.parity != parity
        || pk_role.family != vk_role.family
        || pk_role.byte_limit != KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1
        || vk_role.byte_limit != KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
        || originals.protocol.num_instance != [width]
    {
        return Err(key_decode_message(
            parity,
            "qualification role",
            "helper roles or public shape differ",
        ));
    }
    validate_length(
        parity,
        "qualification proving key",
        originals.proving_key.len(),
        pk_role.byte_limit,
        false,
    )?;
    validate_length(
        parity,
        "qualification verifying key",
        originals.verifying_key.len(),
        vk_role.byte_limit,
        false,
    )?;
    let params = read_parameters(originals.parameters, canonical, parity)?;
    let mut cursor = Cursor::new(originals.proving_key);
    let pk = read_canonical_proving_key_v1::<C, ConcreteCircuit>(
        &mut cursor,
        binding(originals.proving_role, originals.proving_key),
        parity,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params.clone(),
    )?;
    ensure_cursor_consumed(
        parity,
        "qualification proving key",
        &cursor,
        originals.proving_key.len(),
    )?;
    ensure_embedded_vk(parity, &pk, originals.verifying_key)?;
    drop(pk);
    let vk = read_checked_verifying_key_v1::<C, ConcreteCircuit>(
        originals.verifying_key,
        circuit_params,
        parity,
        KAGEMUSHA_HALO2_K_V1,
        "qualification verifying key",
    )?;
    let protocol = compile(
        &params,
        &vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![width]),
    );
    if kagemusha_protocol_structure_digest_v1(&protocol, parity)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?
        != kagemusha_protocol_structure_digest_v1(originals.protocol, parity)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?
        || native_parent_protocol_digest_v1(&protocol, parity)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?
            != native_parent_protocol_digest_v1(originals.protocol, parity)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?
    {
        return Err(key_decode_message(
            parity,
            "qualification protocol",
            "decoded key changes original protocol",
        ));
    }
    Ok((params, protocol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_parameter_roundtrip_rejects_substitution_length_and_domain_in_both_parities() {
        macro_rules! check {
            ($curve:ty, $canonical:ident, $parity:expr) => {{
                let canonical = $canonical();
                let mut bytes = Vec::new();
                canonical.write(&mut bytes).unwrap();
                let decoded = read_parameters::<$curve>(&bytes, &canonical, $parity).unwrap();
                let mut replay = Vec::new();
                decoded.write(&mut replay).unwrap();
                assert_eq!(bytes, replay);
                assert_eq!(decoded.k(), KAGEMUSHA_HALO2_K_V1);
                assert!(
                    read_parameters::<$curve>(&bytes[..bytes.len() - 1], &canonical, $parity)
                        .is_err()
                );
                bytes.push(0);
                assert!(read_parameters::<$curve>(&bytes, &canonical, $parity).is_err());
                bytes.pop();
                bytes[4] ^= 1;
                assert!(read_parameters::<$curve>(&bytes, &canonical, $parity).is_err());
                bytes[4] ^= 1;
                bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
                assert!(read_parameters::<$curve>(&bytes, &canonical, $parity).is_err());
                let other_domain = ParamsIPA::<$curve>::new(6);
                assert!(read_parameters::<$curve>(&replay, &other_domain, $parity).is_err());
            }};
        }
        check!(
            EqAffine,
            canonical_kagemusha_eq_parameters_v1,
            KagemushaPastaParityV1::Eq
        );
        check!(
            EpAffine,
            canonical_kagemusha_ep_parameters_v1,
            KagemushaPastaParityV1::Ep
        );
    }
}
