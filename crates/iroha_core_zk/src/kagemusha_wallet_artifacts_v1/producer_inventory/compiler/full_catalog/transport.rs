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
}
