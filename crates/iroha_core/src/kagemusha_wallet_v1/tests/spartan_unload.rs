//! Future Unload diagnostic over the actual WSV reserve and the exact signed Spartan programs.
//! The fixture supplies only the profile registration and local artifact cache. Every quota,
//! asset, replay and transaction operation uses the same ledger owner as ordinary Unload.

use super::*;
use iroha_core_zk::kagemusha_spartan_native::native::{
    self, AuthenticatedParameters, NativeEngine,
};
use iroha_data_model::sumeragi_finality::{
    SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture,
};

struct FixtureInstallation<'a> {
    scheme: Digest,
    asset: Digest,
    manifest_digest: Option<Digest>,
    cached: Option<&'a AuthenticatedParameters>,
}

impl FixtureInstallation<'_> {
    fn mounted(&self, registration: &Registration) -> Result<&AuthenticatedParameters> {
        registration.require(&self.scheme, &self.asset)?;
        let digest = self
            .manifest_digest
            .as_ref()
            .ok_or(Error::ArtifactsUnavailable)?;
        let parameters = self.cached.ok_or(Error::ArtifactsUnavailable)?;
        if parameters.manifest_digest() != digest {
            return Err(Error::Binding);
        }
        Ok(parameters)
    }
}

fn pay_spartan_unload(
    tx: &mut StateTransaction<'_, '_>,
    relayer: &AccountId,
    installation: &FixtureInstallation<'_>,
    scheme: &Digest,
    asset: &Digest,
    claim: &[u8],
) -> Result<KagemushaWalletPayoutRecordV1> {
    if claim.len() > native::MAX_UNLOAD_CLAIM_BYTES {
        return Err(Error::Proof);
    }
    let mut ledger = wsv::WsvLedger::new(tx, relayer)?;
    let registration = ledger.registration(scheme, asset)?;
    // This existing reservation charges the shared transaction and block proof budgets.
    ledger.reserve_suffix_proofs(claim.len(), 2)?;
    let parameters = installation.mounted(&registration)?;
    let verified = NativeEngine::default()
        .verify_unload_claim(claim, parameters, scheme, asset)
        .map_err(|error| match error {
            native::Error::Unavailable(_) | native::Error::NotEnoughMemory { .. } => {
                Error::ArtifactsUnavailable
            }
            _ => Error::Proof,
        })?;
    let destinations = verified.destinations();
    super::super::super::ledger::settle_unload(
        &mut ledger,
        &registration,
        *verified.nullifier(),
        *verified.digest(),
        verified.amount(),
        &destinations.account,
        &destinations.charge,
    )
}

#[test]
fn spartan_unload_requires_registered_reserve_and_installed_parameters() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    let scheme = memory.registration.scheme.scheme_id();
    let asset = memory.registration.asset.asset_digest();
    let empty = FixtureInstallation {
        scheme,
        asset,
        manifest_digest: None,
        cached: None,
    };
    let attempt = |state: &mut State, asset, claim: &[u8]| {
        transact(state, 2, |tx| {
            pay_spartan_unload(tx, &memory.authority, &empty, &scheme, &asset, claim)
        })
    };
    assert!(matches!(
        attempt(&mut state, asset, &[1]),
        Err(Error::Unavailable)
    ));
    register(&mut state, &memory, 1).unwrap();
    assert!(matches!(
        attempt(&mut state, asset, &[1]),
        Err(Error::ArtifactsUnavailable)
    ));
    assert!(matches!(
        attempt(&mut state, [0; 32], &[1]),
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        attempt(
            &mut state,
            asset,
            &vec![0; native::MAX_UNLOAD_CLAIM_BYTES + 1]
        ),
        Err(Error::Proof)
    ));
}

#[test]
#[ignore = "requires the signed three-mode Spartan fixture bundle and its completed charged claim"]
fn spartan_unload_replays_exact_claim_and_applies_charged_payout_atomically() {
    let mut memory = Memory::new();
    let scheme = memory.registration.scheme.scheme_id();
    let asset = memory.registration.asset.asset_digest();
    let root =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let read = |name| std::fs::read(root.join(name)).unwrap();
    let finality = NativeFinalityFixture::new_with_explicit_parameters();
    let manifest = read("parameter-manifest.norito");
    let parameters = AuthenticatedParameters::install(
        &read("program-bundle.norito"),
        &manifest,
        read("parameter-signature.bin").try_into().unwrap(),
        &memory.registration.scheme,
        SumeragiCommitVerifierV1::new(&finality.verifier()).unwrap(),
    )
    .unwrap();
    let claim = std::fs::read(std::env::var_os("KAGEMUSHA_SPARTAN_UNLOAD_CLAIM").unwrap()).unwrap();
    let verified = NativeEngine::default()
        .verify_unload_claim(&claim, &parameters, &scheme, &asset)
        .unwrap();
    assert_eq!(
        (
            verified.amount(),
            verified.account_payout(),
            verified.online_charge()
        ),
        (4, 3, 1)
    );
    let account = &verified.destinations().account;
    let beneficiary = verified.destinations().charge.beneficiary().unwrap();
    let reserve = memory.registration.reserve.clone();
    let key = KagemushaWalletPayoutKeyV1::Unload(*verified.nullifier());
    memory.balances.insert(account.clone(), 0);
    memory.balances.insert(beneficiary.clone(), 0);
    memory.balances.insert(reserve.clone(), 3);
    // The ledger and experiment have independent P_bytes implementations. A fixture
    // issuer registers both expected public pins independently of the mounted cache.
    // The defaults refer to the retained 4,818-byte v3 manifest and its 15,142-byte claim.
    let (manifest_hex, claim_hex) = match (
        std::env::var("KAGEMUSHA_SPARTAN_EXPECTED_MANIFEST_DIGEST").ok(),
        std::env::var("KAGEMUSHA_SPARTAN_EXPECTED_UNLOAD_DIGEST").ok(),
    ) {
        (Some(manifest), Some(claim)) => (manifest, claim),
        (None, None) => (
            "101142c39d85b53c8158e0312d2fad00679f9f9d141af029cfd387b2c32aca35".into(),
            "851edd15b16ab010acacfce8566222394d9c3c4bac71bb9e3c4befc798a0621b".into(),
        ),
        _ => panic!("supply both independent fixture digest pins together"),
    };
    let manifest_digest: Digest = hex::decode(manifest_hex).unwrap().try_into().unwrap();
    let claim_digest: Digest = hex::decode(claim_hex).unwrap().try_into().unwrap();
    assert_eq!(
        kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*b"kgwparm1"), &manifest),
        manifest_digest
    );
    assert_eq!(
        kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*b"kgwuclm1"), &claim),
        claim_digest
    );
    assert_eq!(parameters.manifest_digest(), &manifest_digest);
    assert_eq!(verified.digest(), &claim_digest);
    let pin = FixtureInstallation {
        scheme,
        asset,
        manifest_digest: Some(manifest_digest),
        cached: Some(&parameters),
    };
    let attempt = |state: &mut State, height, profile: &FixtureInstallation<'_>, wire: &[u8]| {
        transact(state, height, |tx| {
            pay_spartan_unload(tx, &memory.authority, profile, &scheme, &asset, wire)
        })
    };
    let amount = |n: u128| {
        Quantity::from_canonical_numeric(Numeric::new(n, memory.registration.asset.scale)).unwrap()
    };
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    assert!(attempt(&mut state, 2, &pin, &claim).is_err());
    assert_eq!(balance(&state, &memory, &reserve), amount(3));
    assert_eq!(balance(&state, &memory, account), amount(0));
    assert_eq!(balance(&state, &memory, beneficiary), amount(0));
    assert!(
        state
            .view()
            .world
            .kagemusha_wallet_ledger()
            .get(&key.ledger_key(scheme))
            .is_none()
    );

    let mut funded = memory.clone();
    funded.balances.insert(reserve.clone(), 4);
    let mut state = world_state(&funded, true);
    register(&mut state, &funded, 1).unwrap();
    let wrong = FixtureInstallation {
        manifest_digest: Some(*parameters.id()),
        ..pin
    };
    assert!(matches!(
        attempt(&mut state, 2, &wrong, &claim),
        Err(Error::Binding)
    ));
    let payout = attempt(&mut state, 2, &pin, &claim).unwrap();
    assert_eq!(
        payout,
        KagemushaWalletPayoutRecordV1 {
            key,
            source: claim_digest,
            amount: 4,
            transaction: *Hash::new(2u64.to_le_bytes()).as_ref()
        }
    );
    assert_eq!(attempt(&mut state, 3, &pin, &claim).unwrap(), payout);
    let mut malformed = claim.clone();
    malformed.push(0);
    assert!(matches!(
        attempt(&mut state, 4, &pin, &malformed),
        Err(Error::Proof)
    ));
    assert_eq!(balance(&state, &funded, &reserve), amount(0));
    assert_eq!(balance(&state, &funded, account), amount(3));
    assert_eq!(balance(&state, &funded, beneficiary), amount(1));

    // An already recorded distinct canonical claim under this nullifier conflicts even if
    // the amount matches. Set the isolated fixture's history, then verify the actual claim.
    transact(&mut state, 4, |tx| {
        let mut other = payout;
        other.source[0] ^= 1;
        tx.world
            .kagemusha_wallet_ledger
            .insert(key.ledger_key(scheme), storage::encode(&other)?);
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        attempt(&mut state, 5, &pin, &claim),
        Err(Error::Conflict)
    ));
}
