//! Complete logical-route closure into the sole canonical native Omega source.
//!
//! Signed candidate predecessor metadata becomes source-qualified only after
//! every route has imported its actual compiled A/W sources, every terminal has
//! been compared in signed program order, and the exact final original imports.

use iroha_kagemusha_proof::{a_relation::native::artifact::KeyArtifact, omega::native};
use iroha_plonk::{keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};

use super::*;

/// The complete route/catalog or original Omega source failed qualification.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum OmegaQualificationErrorV1 {
    /// Installation, canonical inventory or bounded original mismatch.
    #[error(transparent)]
    Original(#[from] Error),
    /// Missing, repeated, foreign, reordered or inconsistent source-qualified route.
    #[error("incomplete source-qualified operation routes")]
    Routes,
    /// Signed terminal/candidate identity differs from its source-qualified original.
    #[error("source-qualified terminal or Omega catalog mismatch")]
    Catalog,
    /// The fixed complete catalog does not fit or match its canonical source.
    #[error(transparent)]
    Native(#[from] native::Error),
}

/// Complete source-qualified native Omega metadata for one authenticated installation.
/// No proving key is retained, and no server finality proving inventory is required.
/// This is a source owner, not an implementation of wallet orchestration or custody.
pub struct QualifiedOmegaProgramV1 {
    installation: ([u8; 32], [u8; 32]),
    program: native::Program,
    key: KeyArtifact<Ep>,
    original: BlobV1,
    layout: native::CheckpointLayout,
    terminals: usize,
}
impl QualifiedOmegaProgramV1 {
    /// Exact signed installation whose complete routes were qualified.
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        self.installation
    }
    /// Metadata-only complete canonical program, with no caller-selected layout.
    pub const fn program(&self) -> &native::Program {
        &self.program
    }
    /// Exact imported Omega descriptor and complete VK.
    pub const fn key(&self) -> &KeyArtifact<Ep> {
        &self.key
    }
    /// Complete original-derived final checkpoint and transport bounds.
    pub const fn checkpoint_layout(&self) -> native::CheckpointLayout {
        self.layout
    }
    /// Actual number of distinct complete terminal verifier identities.
    pub const fn terminal_count(&self) -> usize {
        self.terminals
    }
    /// Read and strictly import one exact signed original for active proving.
    /// The returned owner holds the sole original PK; drop it after proof work.
    /// # Errors
    /// Local cap, unavailable/changed original or any source/table/VK mismatch.
    pub fn import_prover(
        &self,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<native::Prover, OmegaQualificationErrorV1> {
        self.import_prover_cancellable(originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_prover_cancellable(
        &self,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<native::Prover, OmegaQualificationErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let bytes = read(
            originals,
            self.original,
            config.maximum_bytes.min(PROVING_KEY_MAX_BYTES_V1),
        )?;
        let owner = native::Prover::from_original_artifact_cancellable(
            self.program.clone(),
            self.key.binding().encoded(),
            self.key.key().to_bytes(),
            &bytes,
            config,
            cancellation,
        )?;
        drop(bytes);
        Ok(owner)
    }
}

type Installation = ([u8; 32], [u8; 32]);
type RouteIdentity = (Installation, u32, u32);

fn require_route_identities(
    installation: Installation,
    programs: &[u32],
    actual: &[RouteIdentity],
) -> Result<(), OmegaQualificationErrorV1> {
    if programs.len() != compiled_routes().len() || actual.len() != programs.len() {
        return Err(OmegaQualificationErrorV1::Routes);
    }
    for (index, (identity, program)) in actual.iter().zip(programs).enumerate() {
        if *identity
            != (
                installation,
                u32::try_from(index).map_err(|_| Error::Inventory)?,
                *program,
            )
        {
            return Err(OmegaQualificationErrorV1::Routes);
        }
    }
    Ok(())
}

fn equal<C: PastaCurve>(a: &KeyArtifact<C>, b: &KeyArtifact<C>) -> bool {
    a.binding() == b.binding() && a.key().to_bytes() == b.key().to_bytes()
}
fn matches<C: PastaCurve>(original: &OriginalV1, key: &KeyArtifact<C>) -> bool {
    original.descriptor == BlobV1::of(key.binding().encoded())
        && original.verifying_key == BlobV1::of(key.key().to_bytes())
}
fn metadata(original: ArtifactOriginalV1) -> Result<KeyArtifact<Ep>, OmegaQualificationErrorV1> {
    let binding = DescriptorBinding::decode_v2(&original.descriptor)
        .map_err(|_| OmegaQualificationErrorV1::Catalog)?;
    let key = VerifyingKey::read(&original.verifying_key, &binding)
        .map_err(|_| OmegaQualificationErrorV1::Catalog)?;
    KeyArtifact::new(binding, key).map_err(|_| OmegaQualificationErrorV1::Catalog)
}

impl AuthenticatedProducerInventoryV1 {
    /// Close all compiled logical routes under the exact canonical final Omega source.
    /// Every route must already have imported its full Q/A/W source graph. Terminal
    /// deduplication uses full descriptor/VK equality in signed program order.
    /// The sole original PK is strictly imported, its actual bounds recorded, then dropped.
    /// # Errors
    /// Different installation, omitted/reordered/duplicate route, changed terminal or
    /// predecessor key, nonuniform terminal descriptor, over-capacity complete source,
    /// bounded original failure or any exact original source/VK mismatch.
    pub fn qualify_omega(
        &self,
        installed: &InstalledVerifierPackV1,
        routes: &[QualifiedOperationRouteV1],
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<QualifiedOmegaProgramV1, OmegaQualificationErrorV1> {
        let installation = (
            installed.verifier().scheme().scheme_id(),
            installed.verifier().manifest_digest(),
        );
        if installation != self.installation() {
            return Err(Error::Authority.into());
        }
        let identities: Vec<_> = routes
            .iter()
            .map(QualifiedOperationRouteV1::identity)
            .collect();
        require_route_identities(installation, &self.inventory.routes, &identities)?;
        let candidate = metadata(self.read_verifier_original(self.inventory.omega, originals)?)?;
        let mut programs: Vec<Option<&KeyArtifact<Eq>>> =
            vec![None; self.inventory.operations.len()];
        for (route, required) in routes.iter().zip(compiled_routes()) {
            if required.variant == Variant::Bootstrap {
                if route.candidate_omega().is_some() {
                    return Err(OmegaQualificationErrorV1::Catalog);
                }
            } else if route
                .candidate_omega()
                .is_none_or(|key| !equal(key, &candidate))
            {
                return Err(OmegaQualificationErrorV1::Catalog);
            }
            let (_, _, program) = route.identity();
            let slot = programs
                .get_mut(usize::try_from(program).map_err(|_| Error::Inventory)?)
                .ok_or(OmegaQualificationErrorV1::Routes)?;
            if slot.is_some_and(|key| !equal(key, route.terminal())) {
                return Err(OmegaQualificationErrorV1::Catalog);
            }
            *slot = Some(route.terminal());
        }
        let mut terminals: Vec<&KeyArtifact<Eq>> = Vec::new();
        for (record, key) in self.inventory.operations.iter().zip(programs) {
            let key = key.ok_or(OmegaQualificationErrorV1::Routes)?;
            let original = self
                .inventory
                .member(*record.a.last().ok_or(Error::Inventory)?)?;
            if !matches(original, key) {
                return Err(OmegaQualificationErrorV1::Catalog);
            }
            if !terminals.iter().any(|previous| equal(previous, key)) {
                terminals.push(key);
            }
        }
        if terminals.len() != self.inventory.terminals.len()
            || terminals.is_empty()
            || terminals.len() > 32
        {
            return Err(OmegaQualificationErrorV1::Catalog);
        }
        let binding = terminals[0].binding();
        for (key, original) in terminals.iter().zip(&self.inventory.terminals) {
            if key.binding() != binding || !matches(self.inventory.member(*original)?, key) {
                return Err(OmegaQualificationErrorV1::Catalog);
            }
        }
        let pallas = PinnedParams::derive(16).map_err(|_| OmegaQualificationErrorV1::Catalog)?;
        let vesta = PinnedParams::derive(16).map_err(|_| OmegaQualificationErrorV1::Catalog)?;
        let keys: Vec<_> = terminals
            .iter()
            .map(|key| key.key().to_bytes().to_vec())
            .collect();
        let program =
            native::Program::for_compiled_catalog(binding.encoded(), &keys, vesta, pallas)?;
        let original = self.read_original(self.inventory.omega, originals, config.maximum_bytes)?;
        let owner = native::Prover::from_original_artifact(
            program.clone(),
            &original.descriptor,
            &original.verifying_key,
            &original.proving_key,
            config,
        )?;
        if owner.binding() != candidate.binding()
            || owner.verifying_key().to_bytes() != candidate.key().to_bytes()
        {
            return Err(OmegaQualificationErrorV1::Catalog);
        }
        let layout = owner.checkpoint_layout()?;
        drop(owner);
        drop(original);
        Ok(QualifiedOmegaProgramV1 {
            installation,
            program,
            key: candidate,
            original: self.inventory.member(self.inventory.omega)?.proving_key,
            layout,
            terminals: terminals.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_route_identity_requires_every_ordered_installation_and_program() {
        let installation = ([1; 32], [2; 32]);
        let programs: Vec<_> = (0..compiled_routes().len())
            .map(|index| u32::try_from(index / 2).unwrap())
            .collect();
        let identities: Vec<_> = programs
            .iter()
            .enumerate()
            .map(|(index, program)| (installation, u32::try_from(index).unwrap(), *program))
            .collect();
        require_route_identities(installation, &programs, &identities).unwrap();
        assert!(
            require_route_identities(installation, &programs[..51], &identities[..51]).is_err()
        );
        for index in 0..identities.len() {
            let mut changed = identities.clone();
            changed.remove(index);
            assert!(require_route_identities(installation, &programs, &changed).is_err());
            for field in 0..4 {
                let mut changed = identities.clone();
                match field {
                    0 => changed[index].0.0[0] ^= 1,
                    1 => changed[index].0.1[0] ^= 1,
                    2 => changed[index].1 = changed[index].1.wrapping_add(1),
                    _ => changed[index].2 = changed[index].2.wrapping_add(1),
                }
                assert!(require_route_identities(installation, &programs, &changed).is_err());
            }
            if index > 0 {
                let mut changed = identities.clone();
                changed.swap(index - 1, index);
                assert!(require_route_identities(installation, &programs, &changed).is_err());
            }
        }
    }
}
