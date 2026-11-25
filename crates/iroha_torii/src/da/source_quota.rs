//! Durable admission and expiry for retained application DA source files.

use std::{
    fs::{File, TryLockError},
    io,
    path::Path,
};

use iroha_data_model::da::manifest::DaManifestV1;
use iroha_fs::{OwnerDirectory, PrivateDirectory, PublishMode};

const MAX_TICKETS: usize = 4096;
const INDEX_BYTES: usize = 2 << 20;
const BLOCK: u64 = 4096;
const LOCK: &str = "lock";
const INDEX: &str = "reservations.norito";
const ROOT_LOCK: &str = "source-quota.lock";
const QUOTA: &str = "source-quota";

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::da::source::Reservation")]
struct Reservation {
    ticket: [u8; 32],
    manifest: [u8; 32],
    expires: u64,
    charge: u64,
    chunks: u32,
}

#[derive(
    Debug,
    Default,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::da::source::Inventory")]
struct Inventory {
    /// The greatest deadline whose retirement was authorized by finalized chain time.
    retired_through: u64,
    rows: Vec<Reservation>,
}

impl Reservation {
    fn new(manifest: &DaManifestV1, digest: [u8; 32], assigned: u64) -> io::Result<Self> {
        if manifest.chunks.is_empty()
            || manifest.chunks.len() > super::super::rs16::MAX_MANIFEST_CHUNK_COMMITMENTS
        {
            return Err(invalid("invalid retained DA inventory"));
        }
        // Cover both the advertised manifest lifetime and the full lifetime from server
        // acceptance for an older caller-issued Taikai manifest. The assigned time is frozen.
        let expires = manifest
            .issued_at_unix
            .max(assigned)
            .checked_add(
                manifest
                    .retention_policy
                    .hot_retention_secs
                    .max(manifest.retention_policy.cold_retention_secs)
                    .max(1),
            )
            .ok_or_else(|| invalid("DA retention deadline overflow"))?;
        let mut charge = 2 * BLOCK; // directory and completion marker
        let mut staging = BLOCK;
        for (i, chunk) in manifest.chunks.iter().enumerate() {
            if i != chunk.index as usize
                || chunk.length == 0
                || chunk.length > super::super::rs16::MAX_CHUNK_SIZE_BYTES
            {
                return Err(invalid("invalid retained DA shard extent"));
            }
            let bytes = u64::from(chunk.length).div_ceil(BLOCK) * BLOCK;
            charge += bytes + BLOCK; // rounded extent and directory/inode allowance
            staging = staging.max(bytes + BLOCK);
        }
        Ok(Self {
            ticket: *manifest.storage_ticket.as_bytes(),
            manifest: digest,
            expires,
            charge: charge + staging,
            chunks: manifest.chunks.len() as u32,
        })
    }
}

fn invalid(text: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text)
}

fn lock_error(error: TryLockError) -> io::Error {
    match error {
        TryLockError::WouldBlock => {
            io::Error::new(io::ErrorKind::WouldBlock, "DA source custody is busy")
        }
        TryLockError::Error(error) => error,
    }
}

fn read_index(control: &PrivateDirectory) -> io::Result<Inventory> {
    let bytes = control.read(INDEX, INDEX_BYTES)?;
    let index: Inventory = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(
            MAX_TICKETS,
            INDEX_BYTES,
            INDEX_BYTES * 4,
            INDEX_BYTES * 8,
            32,
        ),
    )
    .map_err(io::Error::other)?;
    let rows = &index.rows;
    if rows.len() > MAX_TICKETS
        || rows.windows(2).any(|w| w[0].ticket >= w[1].ticket)
        || rows.iter().any(|row| {
            row.charge < 2 * BLOCK
                || row.expires == 0
                || row.chunks == 0
                || row.chunks as usize > super::super::rs16::MAX_MANIFEST_CHUNK_COMMITMENTS
        })
    {
        return Err(invalid("invalid DA source reservation journal"));
    }
    Ok(index)
}

fn initialize(root: &OwnerDirectory) -> io::Result<PrivateDirectory> {
    let control = match root.open_private_child_optional(QUOTA)? {
        Some(directory) => directory,
        None => root.create_private_child(QUOTA)?,
    };
    match control.open_retained_private(INDEX) {
        Ok(_) => return Ok(control),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // A missing journal cannot silently reset the budget of a populated spool.
    if let Some(artifacts) = root.open_child_optional("artifacts")? {
        let mut inspected = 0;
        for prefix in artifacts.entries(256)? {
            let bucket = artifacts.open_child(prefix)?;
            for name in bucket.entries(MAX_TICKETS - inspected)? {
                inspected += 1;
                if bucket
                    .open_child(name)?
                    .open_private_child_optional("source")?
                    .is_some()
                {
                    return Err(invalid(
                        "unaccounted DA source exists without its reservation journal",
                    ));
                }
            }
        }
    }
    write_index(&control, &Inventory::default())?;
    root.sync()?;
    Ok(control)
}

fn exclusive_stage(
    control: &PrivateDirectory,
    name: &str,
    maximum: usize,
) -> io::Result<(PrivateDirectory, File)> {
    let dir = control.ensure_child(name)?;
    let file = dir.open_lock(LOCK)?;
    file.try_lock().map_err(lock_error)?;
    dir.require_same_lock(LOCK, &file)?;
    dir.reconcile_atomic_staging(&[LOCK], 1, 1, maximum)?;
    Ok((dir, file))
}

fn write_index(control: &PrivateDirectory, inventory: &Inventory) -> io::Result<()> {
    let (stage, _guard) = exclusive_stage(control, "index-stage", INDEX_BYTES)?;
    let bytes = norito::encode_canonical(inventory).map_err(io::Error::other)?;
    if bytes.len() > INDEX_BYTES {
        return Err(invalid("DA custody journal exceeds its bound"));
    }
    control.write_atomic_from_staging(&stage, INDEX, &bytes, PublishMode::Replace)
}

fn stripe(ticket: &[u8; 32]) -> String {
    format!("stripe-{:02x}", ticket[0])
}

/// A stable stripe file guards publication, verification and expiry across Torii instances.
#[derive(Debug)]
pub(in crate::da) struct Lease {
    pub(super) staging: PrivateDirectory,
    _guard: File,
    expires: u64,
}

impl Lease {
    pub(crate) fn require_live(&self, finalized: u64) -> io::Result<()> {
        if finalized >= self.expires {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DA source retention has expired in finalized chain time",
            ))
        } else {
            Ok(())
        }
    }
}

fn existing_source(
    root: &OwnerDirectory,
    ticket: &[u8; 32],
) -> io::Result<Option<PrivateDirectory>> {
    let name = hex::encode(ticket);
    let Some(artifacts) = root.open_child_optional("artifacts")? else {
        return Ok(None);
    };
    let Some(bucket) = artifacts.open_child_optional(&name[..2])? else {
        return Ok(None);
    };
    let Some(ticket) = bucket.open_child_optional(name)? else {
        return Ok(None);
    };
    ticket.open_private_child_optional("source")
}

fn release(root: &OwnerDirectory, row: &Reservation) -> io::Result<()> {
    if let Some(dir) = existing_source(root, &row.ticket)? {
        let entries = dir.entries(row.chunks as usize + 1)?;
        if let Some(marker) = dir.read_optional("manifest.blake3", 32)?
            && marker.as_slice() != row.manifest
        {
            return Err(invalid("DA source marker disagrees with reservation"));
        }
        for entry in &entries {
            if entry == "manifest.blake3" {
                continue;
            }
            let valid = entry
                .to_str()
                .and_then(|s| s.strip_prefix("chunk_"))
                .and_then(|s| s.strip_suffix(".bin"))
                .and_then(|s| s.parse::<u32>().ok())
                .is_some_and(|i| {
                    i < row.chunks && entry.to_str() == Some(format!("chunk_{i:05}.bin").as_str())
                });
            if !valid {
                return Err(invalid("unexpected retained DA source entry"));
            }
        }
        for entry in entries {
            dir.remove_private(entry)?;
        }
        dir.remove_empty()?;
    }
    Ok(())
}

/// Authorization is journaled and synced before the first source shard. Failed writes keep
/// their original charge until an identical retry finishes or committed time permits reclamation.
pub(super) fn reserve(
    spool: &Path,
    manifest: &DaManifestV1,
    digest: [u8; 32],
    assigned: u64,
    finalized: u64,
    budget: u64,
) -> io::Result<Lease> {
    let wanted = Reservation::new(manifest, digest, assigned)?;
    if wanted.expires <= finalized {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DA source retention has expired",
        ));
    }
    let root = OwnerDirectory::open_or_create(spool)?;
    let journal_lock = root.open_lock(ROOT_LOCK)?;
    journal_lock.lock()?;
    root.require_same_lock(ROOT_LOCK, &journal_lock)?;
    let control = initialize(&root)?;
    let mut attempted = std::collections::BTreeSet::new();
    loop {
        let mut index = read_index(&control)?;
        let finalized = finalized.max(index.retired_through);
        if wanted.expires <= finalized {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DA source retention has expired",
            ));
        }
        let rows = &mut index.rows;
        let existing = rows.binary_search_by_key(&wanted.ticket, |r| r.ticket);
        let mut fit = true;
        if let Ok(i) = existing {
            if rows[i] != wanted {
                return Err(invalid("DA retry changes its original source reservation"));
            }
        } else {
            if existing_source(&root, &wanted.ticket)?.is_some() {
                return Err(invalid("DA source exists without its ticket reservation"));
            }
            let total = rows
                .iter()
                .try_fold(0u64, |s, r| s.checked_add(r.charge))
                .ok_or_else(|| invalid("DA quota charge overflow"))?;
            fit = rows.len() < MAX_TICKETS && wanted.charge <= budget.saturating_sub(total);
            // A single submission reclaims at most four expired tickets. No shared
            // ledger lock is held while potentially thousands of shard files are removed.
            if attempted.len() < 4 && (!fit || attempted.is_empty()) {
                let mut tried = [false; 256];
                let mut selected = None;
                let start = rows.partition_point(|row| row.ticket < wanted.ticket);
                for row in rows[start..]
                    .iter()
                    .chain(&rows[..start])
                    .filter(|r| r.expires <= finalized && !attempted.contains(&r.ticket))
                {
                    let bucket = usize::from(row.ticket[0]);
                    if tried[bucket] {
                        continue;
                    }
                    tried[bucket] = true;
                    match exclusive_stage(
                        &control,
                        &stripe(&row.ticket),
                        super::super::rs16::MAX_CHUNK_SIZE_BYTES as usize,
                    ) {
                        Ok(lease) => {
                            selected = Some((row.clone(), lease));
                            break;
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                        Err(error) => return Err(error),
                    }
                }
                if let Some((row, _lease)) = selected {
                    attempted.insert(row.ticket);
                    // Persist authorization before the first unlink. A delayed request with an
                    // older chain snapshot cannot reinstate this obligation, even after restart.
                    if row.expires > index.retired_through {
                        index.retired_through = row.expires;
                        write_index(&control, &index)?;
                    }
                    journal_lock.unlock()?;
                    let released = release(&root, &row);
                    journal_lock.lock()?;
                    root.require_same_lock(ROOT_LOCK, &journal_lock)?;
                    if let Err(error) = released {
                        // Unsafe or damaged ticket custody stays charged. An unrelated
                        // ticket may still be admitted or reclaimed within the limit.
                        iroha_logger::warn!(ticket = %hex::encode(row.ticket), %error, "expired DA source remains reserved");
                        continue;
                    }
                    let mut current = read_index(&control)?;
                    let at = current
                        .rows
                        .binary_search_by_key(&row.ticket, |r| r.ticket)
                        .map_err(|_| invalid("DA reservation changed during expiry"))?;
                    if current.rows[at] != row {
                        return Err(invalid("DA reservation changed during expiry"));
                    }
                    current.rows.remove(at);
                    write_index(&control, &current)?;
                    continue;
                }
            }
        }
        if !fit {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "DA retained source budget is full",
            ));
        }
        let (staging, guard) = exclusive_stage(
            &control,
            &stripe(&wanted.ticket),
            super::super::rs16::MAX_CHUNK_SIZE_BYTES as usize,
        )?;
        if let Err(at) = existing {
            rows.insert(at, wanted.clone());
            write_index(&control, &index)?;
        }
        return Ok(Lease {
            staging,
            _guard: guard,
            expires: wanted.expires,
        });
    }
}

pub(super) fn verify_lease(
    spool: &Path,
    manifest: &DaManifestV1,
    digest: &[u8; 32],
    assigned: u64,
) -> io::Result<Lease> {
    let root = OwnerDirectory::open(spool)?;
    let lock = root.open_existing_lock(ROOT_LOCK)?;
    lock.lock_shared()?;
    root.require_same_lock(ROOT_LOCK, &lock)?;
    let control = root.open_private_child(QUOTA)?;
    let index = read_index(&control)?;
    let rows = &index.rows;
    let nominal = Reservation::new(manifest, *digest, assigned)?;
    let ticket = nominal.ticket;
    if nominal.expires <= index.retired_through {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DA source retention has expired",
        ));
    }
    let i = rows
        .binary_search_by_key(&ticket, |r| r.ticket)
        .map_err(|_| invalid("missing DA source reservation"))?;
    let row = &rows[i];
    if row != &nominal {
        return Err(invalid(
            "retained DA source reservation differs from the manifest",
        ));
    }
    let staging = control.open_child(stripe(&ticket))?;
    let guard = staging.open_existing_lock(LOCK)?;
    guard.try_lock_shared().map_err(lock_error)?;
    staging.require_same_lock(LOCK, &guard)?;
    Ok(Lease {
        staging,
        _guard: guard,
        expires: row.expires,
    })
}
