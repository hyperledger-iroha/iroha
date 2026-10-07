//! Exact authenticated-original transport into production immutable custody.
//!
//! This copies DATA only. The source constructor must subsequently reconstruct
//! the complete graph; a successful copy creates no installed source capability.

use super::*;

fn identities(
    wallet: &[OriginalV1],
    finality: &[ArtifactRecord],
) -> Result<BTreeMap<[u8; 32], BlobV1>, Error> {
    let mut blobs = BTreeMap::new();
    let mut add = |blob: BlobV1, cap| {
        blob.length(cap)?;
        if blobs
            .insert(blob.sha256, blob)
            .is_some_and(|previous| previous != blob)
        {
            return Err(Error::Inventory);
        }
        Ok(())
    };
    for original in wallet {
        add(original.descriptor, DESCRIPTOR_MAX_BYTES_V1)?;
        add(original.verifying_key, VERIFYING_KEY_MAX_BYTES_V1)?;
        add(original.proving_key, PROVING_KEY_MAX_BYTES_V1)?;
    }
    for record in finality {
        // The signed server PK hash stays in the inventory, but its tables are
        // deliberately absent from a wallet. No graph dependency is omitted:
        // the qualifier reconstructs every exact descriptor and verifying key.
        for (index, cap) in [DESCRIPTOR_MAX_BYTES_V1, VERIFYING_KEY_MAX_BYTES_V1]
            .into_iter()
            .enumerate()
        {
            add(
                BlobV1 {
                    bytes: record.lengths[index],
                    sha256: record.sha256[index],
                },
                cap,
            )?;
        }
    }
    Ok(blobs)
}

fn copy_blobs(
    blobs: &BTreeMap<[u8; 32], BlobV1>,
    source: &mut dyn OriginalSourceV1,
    destination: &mut DirectoryOriginalsV1,
) -> Result<(usize, u64), Error> {
    let total = blobs.values().try_fold(0_u64, |sum, blob| {
        sum.checked_add(blob.bytes)
            .filter(|sum| *sum <= OUTPUT_BYTES)
            .ok_or(Error::Inventory)
    })?;
    for blob in blobs.values() {
        // Each exact bounded original is released before opening the next.
        let bytes = read(source, *blob, PROVING_KEY_MAX_BYTES_V1)?;
        destination
            .store_original(*blob, &bytes)
            .map_err(|_| Error::Inventory)?;
    }
    Ok((blobs.len(), total))
}

pub(super) fn copy(
    authenticated: &AuthenticatedProducerInventoryV1,
    source: &mut dyn OriginalSourceV1,
    destination: &mut DirectoryOriginalsV1,
) -> Result<(usize, u64), Error> {
    let inventory = &authenticated.inventory;
    copy_blobs(
        &identities(&inventory.originals, &inventory.finality.originals)?,
        source,
        destination,
    )
}

fn membership(wallet: &[OriginalV1], finality: &[ArtifactRecord]) -> Result<Vec<u8>, Error> {
    let wallet = identities(wallet, &[])?;
    let verifier = identities(&[], finality)?;
    let mut all = wallet.clone();
    for (&hash, &blob) in &verifier {
        if all.insert(hash, blob).is_some_and(|old| old != blob) {
            return Err(Error::Inventory);
        }
    }
    let mut server = BTreeMap::new();
    for record in finality {
        let blob = BlobV1 {
            bytes: record.lengths[2],
            sha256: record.sha256[2],
        };
        if blob.bytes == 0 || blob.sha256 == [0; 32] {
            return Err(Error::Inventory);
        }
        if all.get(&blob.sha256).is_some_and(|old| *old != blob)
            || server
                .insert(blob.sha256, blob)
                .is_some_and(|old| old != blob)
        {
            return Err(Error::Inventory);
        }
    }
    // A byte-identical shared original is required by the wallet, regardless of
    // the additional server role. Only genuinely excluded identities go here.
    server.retain(|hash, _| !all.contains_key(hash));
    let rows = |blobs: &BTreeMap<[u8; 32], BlobV1>| {
        blobs
            .values()
            .map(|blob| {
                norito::json!({
                    "sha256": (hex::encode(blob.sha256)),
                    "bytes": (blob.bytes),
                })
            })
            .collect::<Vec<_>>()
    };
    let total = |blobs: &BTreeMap<[u8; 32], BlobV1>| {
        blobs.values().try_fold(0_u64, |sum, blob| {
            sum.checked_add(blob.bytes).ok_or(Error::Inventory)
        })
    };
    norito::json::to_vec(&norito::json!({
        "schema": "iroha.kagemusha.engineering-qualified-source-membership.v1",
        "scope": "Exact signed inventory membership after complete source qualification; no phone storage, memory, performance or deployment authority",
        "wallet_originals": (rows(&wallet)),
        "wallet_unique_bytes": (total(&wallet)?),
        "finality_verifier_originals": (rows(&verifier)),
        "finality_verifier_unique_bytes": (total(&verifier)?),
        "combined_unique_bytes": (total(&all)?),
        "server_only_proving_key_references": (rows(&server)),
        "server_only_committed_bytes": (total(&server)?),
        "server_proving_key_bytes_read": 0,
        "server_proving_key_scope": "Signed source-inventory lengths and hashes only; excluded server proving bytes were not loaded or rehashed",
    }))
    .map_err(|_| Error::Inventory)
}

pub(super) fn qualified_membership(qualified: &QualifiedWalletSourcesV1) -> Result<Vec<u8>, Error> {
    let inventory = qualified.inventory();
    membership(&inventory.originals, &inventory.finality.originals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_selects_all_wallet_tables_but_only_finality_verifier_originals() {
        let wallet = OriginalV1 {
            descriptor: BlobV1::of(b"wallet descriptor"),
            verifying_key: BlobV1::of(b"wallet verifying key"),
            proving_key: BlobV1::of(b"wallet original proving key"),
        };
        let server = [
            BlobV1::of(b"server descriptor"),
            BlobV1::of(b"server verifying key"),
            BlobV1::of(b"server proving key intentionally not transported"),
        ];
        let record = ArtifactRecord {
            name: Vec::new(),
            lengths: server.map(|blob| blob.bytes),
            sha256: server.map(|blob| blob.sha256),
        };
        let selected = identities(&[wallet], &[record]).unwrap();
        assert_eq!(selected.len(), 5);
        for blob in [
            wallet.descriptor,
            wallet.verifying_key,
            wallet.proving_key,
            server[0],
            server[1],
        ] {
            assert_eq!(selected.get(&blob.sha256), Some(&blob));
        }
        assert!(!selected.contains_key(&server[2].sha256));
        let mut conflicting = wallet;
        conflicting.proving_key.bytes += 1;
        assert!(identities(&[wallet, conflicting], &[]).is_err());
    }

    #[test]
    fn transport_seals_exact_bytes_and_refuses_changed_or_missing_originals() {
        struct Source(Vec<u8>);
        impl OriginalSourceV1 for Source {
            fn open(&mut self, _: [u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
                Ok(Box::new(std::io::Cursor::new(&self.0)))
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let private =
            iroha_fs::PrivateDirectory::open_or_create(temp.path().join("originals")).unwrap();
        let mut destination = DirectoryOriginalsV1::open_existing(
            private.path().canonicalize().unwrap(),
            PROVING_KEY_MAX_BYTES_V1,
        )
        .unwrap();
        let bytes = b"exact copied original";
        let blob = BlobV1::of(bytes);
        let selected = BTreeMap::from([(blob.sha256, blob)]);
        assert!(
            copy_blobs(
                &selected,
                &mut Source(b"changed bytes".to_vec()),
                &mut destination
            )
            .is_err()
        );
        assert!(
            destination
                .open(blob.sha256)
                .is_err_and(|error| error == Error::Unavailable)
        );
        assert_eq!(
            copy_blobs(&selected, &mut Source(bytes.to_vec()), &mut destination).unwrap(),
            (1, bytes.len() as u64)
        );
        destination.verify_original(blob).unwrap();
        assert_eq!(read(&mut destination, blob, bytes.len()).unwrap(), bytes);
        // Retry cannot replace the sealed original and must reauthenticate it.
        copy_blobs(&selected, &mut Source(bytes.to_vec()), &mut destination).unwrap();
        struct Missing;
        impl OriginalSourceV1 for Missing {
            fn open(&mut self, _: [u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
                Err(Error::Unavailable)
            }
        }
        assert!(
            copy_blobs(&selected, &mut Missing, &mut destination)
                .is_err_and(|error| error == Error::Unavailable)
        );
    }

    #[test]
    fn membership_counts_unique_required_bytes_and_only_excluded_server_references() {
        // Metadata DATA only: this does not construct a qualified source owner.
        let shared = BlobV1::of(b"shared verifier");
        let wallet = OriginalV1 {
            descriptor: BlobV1::of(b"wallet descriptor"),
            verifying_key: shared,
            proving_key: BlobV1::of(b"wallet proving key"),
        };
        let descriptor = BlobV1::of(b"server descriptor");
        let excluded = BlobV1::of(b"server-only proving key");
        let mut record = ArtifactRecord {
            name: Vec::new(),
            lengths: [descriptor.bytes, shared.bytes, excluded.bytes],
            sha256: [descriptor.sha256, shared.sha256, excluded.sha256],
        };
        let read = |record| -> norito::json::Value {
            norito::json::from_slice(&membership(&[wallet, wallet], &[record]).unwrap()).unwrap()
        };
        let value = read(record.clone());
        let wallet_bytes = wallet.descriptor.bytes + shared.bytes + wallet.proving_key.bytes;
        assert_eq!(value["wallet_originals"].as_array().unwrap().len(), 3);
        assert_eq!(value["wallet_unique_bytes"].as_u64(), Some(wallet_bytes));
        assert_eq!(
            value["combined_unique_bytes"].as_u64(),
            Some(wallet_bytes + descriptor.bytes)
        );
        assert_eq!(
            value["server_only_committed_bytes"].as_u64(),
            Some(excluded.bytes)
        );
        assert_eq!(value["server_proving_key_bytes_read"].as_u64(), Some(0));
        record.lengths[2] = wallet.proving_key.bytes;
        record.sha256[2] = wallet.proving_key.sha256;
        let value = read(record.clone());
        assert!(
            value["server_only_proving_key_references"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(value["server_only_committed_bytes"].as_u64(), Some(0));
        record.lengths[2] += 1;
        assert!(membership(&[wallet], &[record]).is_err());
    }
}
