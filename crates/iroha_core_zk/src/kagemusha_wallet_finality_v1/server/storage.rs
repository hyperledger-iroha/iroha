//! Bounded immutable proof DATA. Restoring bytes never establishes proof authority.

use std::{
    fs::File,
    io::{self, Read as _, Write as _},
    path::Path,
};

use ff::PrimeField as _;
use iroha_fs::{FileIdentity, PrivateDirectory};
use iroha_kagemusha_proof::finality::continuity::{
    SourceNodeEvidence,
    checkpoint::{ProofCheckpointStore, ProofIdentity},
    producer::Error,
};
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk_recursion::{ACCUMULATOR_BYTES, AccumulatorT};
use norito::{Decode, Encode, NoritoSchema};
use rand::rand_core::TryRngCore as _;
use sha2::{Digest as _, Sha256};

const MAX_RECORD: usize = 1 << 20;
const LOCK: &str = "owner.lock";
const SELECTION: &str = "selection";

#[derive(Encode, Decode, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.server.proof_checkpoint.v1")]
struct Record {
    version: u8,
    descriptor: [u8; 32],
    key: [u8; 32],
    endpoints: [[u8; 32]; 6],
    proof: Vec<u8>,
    pallas: [u8; ACCUMULATOR_BYTES],
    vesta: [u8; ACCUMULATOR_BYTES],
}

pub(super) struct Journal {
    directory: PrivateDirectory,
    owner: File,
    identity: FileIdentity,
    maximum_entries: usize,
    maximum_bytes: u64,
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "finality journal custody differs",
    )
}
impl Journal {
    pub(super) fn open(
        path: &Path,
        selection: &[u8],
        maximum_entries: usize,
        maximum_bytes: u64,
    ) -> io::Result<Self> {
        if maximum_entries < 3
            || maximum_entries > 1_000_000
            || maximum_bytes == 0
            || maximum_bytes == u64::MAX
        {
            return Err(invalid());
        }
        let directory = PrivateDirectory::open_exact(path)?;
        let owner = directory.open_ownership_lock(LOCK)?;
        owner.try_lock().map_err(io::Error::other)?;
        let identity = FileIdentity::of(&owner)?;
        let mut journal = Self {
            directory,
            owner,
            identity,
            maximum_entries,
            maximum_bytes,
        };
        journal.inventory()?;
        // A previously initialized namespace cannot adopt a different release or root.
        if journal.read(SELECTION, MAX_RECORD)?.is_none() {
            let (entries, _) = journal.inventory()?;
            if entries != 1 {
                return Err(invalid());
            }
        }
        journal.put(SELECTION, selection)?;
        Ok(journal)
    }

    fn guard(&self) -> io::Result<()> {
        self.directory.revalidate()?;
        if FileIdentity::of(&self.owner)? != self.identity
            || FileIdentity::of(&self.directory.open_existing_lock(LOCK)?)? != self.identity
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn inventory(&self) -> io::Result<(usize, u64)> {
        self.guard()?;
        let mut entries = 0_usize;
        let mut bytes = 0_u64;
        self.directory
            .visit_private_files(self.maximum_entries, |name, metadata| {
                let name = name.to_str().ok_or_else(invalid)?;
                let digest_name = |prefix: &str| {
                    name.strip_prefix(prefix).is_some_and(|v| {
                        v.len() == 64
                            && v.bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    })
                };
                let pending = name.strip_prefix("pending-").is_some_and(|v| {
                    v.len() == 32
                        && v.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                });
                if name != LOCK
                    && name != SELECTION
                    && !digest_name("node-")
                    && !digest_name("receipt-")
                    && !pending
                {
                    return Err(invalid());
                }
                if name != LOCK && !pending && (!metadata.is_read_only() || metadata.is_empty()) {
                    return Err(invalid());
                }
                if metadata.len() > MAX_RECORD as u64 {
                    return Err(invalid());
                }
                entries = entries.checked_add(1).ok_or_else(invalid)?;
                bytes = bytes.checked_add(metadata.len()).ok_or_else(invalid)?;
                if bytes > self.maximum_bytes {
                    return Err(invalid());
                }
                Ok(())
            })?;
        self.guard()?;
        Ok((entries, bytes))
    }
    pub(super) fn read(&self, name: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        self.guard()?;
        let Some(mut original) = self
            .directory
            .open_retained_read_only_optional(name, maximum)?
        else {
            return Ok(None);
        };
        let before = original.snapshot()?;
        let length = usize::try_from(original.len()?).map_err(|_| invalid())?;
        if length == 0 {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(io::Error::other)?;
        original
            .by_ref()
            .take(length as u64 + 1)
            .read_to_end(&mut bytes)?;
        original.revalidate()?;
        if original.snapshot()? != before || bytes.len() != length {
            return Err(invalid());
        }
        self.guard()?;
        Ok(Some(bytes))
    }
    pub(super) fn put(&mut self, name: &str, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_RECORD {
            return Err(invalid());
        }
        if let Some(existing) = self.read(name, MAX_RECORD)? {
            return if existing == bytes {
                self.directory.sync()
            } else {
                Err(invalid())
            };
        }
        let (entries, total) = self.inventory()?;
        if entries >= self.maximum_entries
            || total
                .checked_add(bytes.len() as u64)
                .is_none_or(|v| v > self.maximum_bytes)
        {
            return Err(invalid());
        }
        let mut nonce = [0; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(io::Error::other)?;
        let mut pending = self
            .directory
            .create_retained_private(format!("pending-{}", hex::encode(nonce)), bytes.len())?;
        pending.write_all(bytes)?;
        pending
            .seal_read_only()?
            .publish_new_name(name)?
            .revalidate()?;
        if self.read(name, MAX_RECORD)?.as_deref() != Some(bytes) {
            return Err(invalid());
        }
        self.directory.sync()?;
        self.guard()
    }
}

fn name(id: &ProofIdentity) -> String {
    let mut hash = Sha256::new();
    hash.update(b"kagemusha-server-finality-checkpoint-v1");
    hash.update(id.source.descriptor);
    hash.update(id.source.key);
    for endpoint in id.endpoints {
        hash.update(endpoint);
    }
    format!("node-{}", hex::encode(hash.finalize()))
}
impl ProofCheckpointStore for Journal {
    fn load(&mut self, id: &ProofIdentity) -> Result<Option<SourceNodeEvidence>, Error> {
        let Some(bytes) = self
            .read(&name(id), MAX_RECORD)
            .map_err(|_| Error::Artifact)?
        else {
            return Ok(None);
        };
        let record: Record = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(MAX_RECORD),
        )
        .map_err(|_| Error::Artifact)?;
        if record.version != 1
            || record.descriptor != id.source.descriptor
            || record.key != id.source.key
            || record.endpoints != id.endpoints
        {
            return Err(Error::Artifact);
        }
        let mut endpoints = [Fp::from(0); 6];
        for (out, word) in endpoints.iter_mut().zip(record.endpoints) {
            *out = Option::<Fp>::from(Fp::from_repr(word)).ok_or(Error::Input)?;
        }
        Ok(Some(SourceNodeEvidence {
            endpoints,
            proof: record.proof,
            pallas: AccumulatorT::<Ep>::from_bytes(&record.pallas).map_err(|_| Error::Input)?,
            vesta: AccumulatorT::<Eq>::from_bytes(&record.vesta).map_err(|_| Error::Input)?,
        }))
    }
    fn store(&mut self, id: &ProofIdentity, proof: &SourceNodeEvidence) -> Result<(), Error> {
        if proof.endpoints.map(|v| v.to_repr()) != id.endpoints
            || proof.proof.len() > MAX_RECORD - 4096
        {
            return Err(Error::Input);
        }
        let bytes = norito::to_bytes(&Record {
            version: 1,
            descriptor: id.source.descriptor,
            key: id.source.key,
            endpoints: id.endpoints,
            proof: proof.proof.clone(),
            pallas: proof.pallas.to_bytes(),
            vesta: proof.vesta.to_bytes(),
        })
        .map_err(|_| Error::Artifact)?;
        self.put(&name(id), &bytes).map_err(|_| Error::Artifact)
    }
}
