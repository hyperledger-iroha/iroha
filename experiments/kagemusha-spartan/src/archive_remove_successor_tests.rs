//! Successor-only indexed removal, using the signed Send insertion relation backwards.
use super::*;
use crate::{
    bootstrap_context::{LINEAGE_IO, PENDING_ROOT},
    indexed_map::{self, InsertWitness, Leaf, Path},
    operation_auth::tests::{expose, finish, holds, proof},
    send_arithmetic::Source,
    uniform::Array,
};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedOpeningV1, KagemushaWalletIndexedRemoveV1, KagemushaWalletIndexedTreeV1,
    kagemusha_wallet_indexed_verify_membership_v1,
    kagemusha_wallet_indexed_verify_non_membership_v1,
};
use iroha_pasta::poseidon;

// A change at another slot changes one sibling on this path, at the highest
// differing index bit. The subtree below that fork keeps all of its old siblings.
fn subtree(mut hash: Fp, path: &Path<32>, levels: usize) -> Fp {
    for (h, sibling) in path.siblings.iter().take(levels).enumerate() {
        let pair = if (path.index >> h) & 1 == 0 {
            [hash, *sibling]
        } else {
            [*sibling, hash]
        };
        hash = poseidon::hash_with_domain(indexed_map::NODE_DOMAIN, &pair);
    }
    hash
}

fn path(original: &KagemushaWalletIndexedOpeningV1) -> Result<Path<32>> {
    let mut siblings = [Fp::ZERO; 32];
    for (dst, raw) in siblings.iter_mut().zip(&original.siblings) {
        *dst = field(raw)?;
    }
    Ok(Path {
        index: original.slot,
        siblings,
    })
}

fn reverse_insertion(
    before: &[u8; 32],
    key: &[u8; 32],
    value: &[u8; 32],
    removal: &KagemushaWalletIndexedRemoveV1,
) -> Result<InsertWitness<32>> {
    let after = removal
        .verify(before, key)
        .map_err(|_| Error::Malformed("indexed removal openings"))?;
    if removal.leaf.value != *value {
        return Err(Error::AuthorityMismatch("pending descriptor"));
    }
    let mut low_path = path(&removal.predecessor_opening)?;
    let mut slot = path(&removal.leaf_opening)?;
    let fork = (low_path.index ^ slot.index).ilog2() as usize; // verify checked distinct slots
    let low = Leaf {
        key: field(&removal.predecessor.key)?,
        value: field(&removal.predecessor.value)?,
        next: field(&removal.leaf.next_key)?,
    };
    let key = field(key)?;
    // Insertion opens the predecessor *after* deletion, and the vacant slot
    // after relinking the predecessor back to the removed key.
    let original_low = indexed_map::leaf_hash(Leaf { next: key, ..low });
    low_path.siblings[fork] = subtree(Fp::ZERO, &slot, fork);
    slot.siblings[fork] = subtree(original_low, &low_path, fork);
    Ok(InsertWitness {
        old_root: field(&after)?,
        key,
        value: field(value)?,
        low,
        low_path,
        slot,
    })
}

fn exercise(
    tree: &mut KagemushaWalletIndexedTreeV1,
    key: Fp,
    value: Fp,
    expected_before: Option<Fp>,
    prove: bool,
) -> [u8; 32] {
    let before = tree.root();
    if let Some(original) = expected_before {
        assert_eq!(before, original.to_repr());
    }
    let (original, old_path) = tree.membership(&key.to_repr()).unwrap();
    assert_eq!(original.value, value.to_repr());
    kagemusha_wallet_indexed_verify_membership_v1(&before, &original, &old_path).unwrap();
    let removal = tree.remove(&key.to_repr()).unwrap();
    let after = tree.root();
    assert_eq!(removal.verify(&before, &key.to_repr()).unwrap(), after);
    let inverse = reverse_insertion(&before, &key.to_repr(), &value.to_repr(), &removal).unwrap();
    let mut wiring = full::Wiring::default();
    let family = wiring.family(0, 1);
    let cells = indexed_map::append_insert(&mut wiring, family, &inverse);
    let ports = [cells.new_root, cells.key, cells.value, cells.old_root].map(|cell| Source {
        array: cells.local,
        cell,
        instance: 0,
    });
    expose(&mut wiring, family, 4, &ports);
    let range = wiring.push(Array {
        name: "indexed removal range",
        family,
        rho: 11,
        r_count: 2048,
        fixed: true,
        lookups: vec![],
        widths: vec![11],
        rows: vec![],
        values: vec![(0..2048u64).map(Fp::from).collect()],
    });
    wiring.tables.push((range, crate::nonnative::RANGE_TABLE));
    let built = finish(wiring, 12);
    assert_eq!(
        &built.1.io[4..8],
        &[field(&before).unwrap(), key, value, field(&after).unwrap()]
    );
    assert!(holds(&built));
    if prove {
        assert!(proof(&built));
    }
    let (low, open) = tree.non_membership(&key.to_repr()).unwrap();
    assert_eq!(low.key, removal.predecessor.key);
    assert_eq!(low.value, removal.predecessor.value);
    assert_eq!(low.next_key, original.next_key);
    assert_eq!(open.slot, removal.predecessor_opening.slot);
    kagemusha_wallet_indexed_verify_non_membership_v1(&after, &key.to_repr(), &low, &open).unwrap();
    assert_eq!(tree.opening(old_path.slot).empty_root().unwrap(), after);
    assert!(tree.leaf_at(old_path.slot).is_none());
    if original.next_key != [0; 32] {
        let (next, open) = tree.membership(&original.next_key).unwrap();
        kagemusha_wallet_indexed_verify_membership_v1(&after, &next, &open).unwrap();
    }
    built.0.program_digest()
}

#[test]
fn successor_indexed_removal_reopens_sentinel_and_interior_links() {
    let key = Fp::from(47);
    let value = Fp::from(81);
    let mut first = KagemushaWalletIndexedTreeV1::new();
    first.insert(key.to_repr(), value.to_repr()).unwrap();
    let sentinel_shape = exercise(&mut first, key, value, None, false);
    let mut middle = KagemushaWalletIndexedTreeV1::new();
    for k in [30, 80, 20, 47] {
        middle
            .insert(
                Fp::from(k).to_repr(),
                if k == 47 { value } else { Fp::from(k + 100) }.to_repr(),
            )
            .unwrap();
    }
    assert_eq!(
        exercise(&mut middle, key, value, None, true),
        sentinel_shape
    );
}

#[test]
#[ignore = "requires the signed device bundle and retained manifest-bound Send and CreditStatus"]
fn successor_removes_exact_pending_descriptor_of_verified_delivered_send() {
    use std::path::PathBuf;
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let wallet = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let read = |dir: &PathBuf, file: &str| std::fs::read(dir.join(file)).unwrap();
    let params = crate::native::device_fixture::install(
        &read(&bundle, "program-bundle.norito"),
        &read(&bundle, "parameter-manifest.norito"),
        read(&bundle, "parameter-signature.bin").try_into().unwrap(),
    )
    .unwrap();
    let engine = NativeEngine::default();
    let payment = engine
        .verify_payment(&read(&wallet, "complete-payment.bin"), &params)
        .unwrap();
    // The retained companion is byte-identical between these two signed open quotes.
    // Recover the open original of this Payment under its own recipient signature.
    let other = read(&wallet, "competing-send-request.bin");
    let (len, raw) = other.strip_prefix(b"KSO1\x03").unwrap().split_at(4);
    assert_eq!(
        u32::from_le_bytes(len.try_into().unwrap()) as usize,
        raw.len()
    );
    let companion = crate::native::decode_open_request_peer(raw).unwrap();
    let request = payment.request();
    let mut body = request.body;
    body[66..130].fill(0);
    body[194..210].fill(0);
    let exact = [body.as_slice(), &request.signature].concat();
    let quote = crate::native::encode_open_request_peer(
        &exact,
        companion.receiver_credential,
        companion.certificates,
        companion.fee_schedule,
        companion.destination_account,
    )
    .unwrap();
    let status = engine
        .verify_credit_status_peer(
            &read(&wallet, "outgoing-credit-status.bin"),
            &quote,
            &payment,
            &params,
        )
        .unwrap();
    assert_eq!(status, crate::native::VerifiedCreditStatus::Credited);
    let s = payment.statement();
    let key = s[17];
    let value = poseidon::hash_with_domain(crate::send_maps::PENDING_DOMAIN, &s[17..24]);
    let mut pending = KagemushaWalletIndexedTreeV1::new();
    assert_eq!(pending.root(), s[13].to_repr());
    pending.insert(key.to_repr(), value.to_repr()).unwrap();
    exercise(
        &mut pending,
        key,
        value,
        Some(payment.profile()[LINEAGE_IO + PENDING_ROOT]),
        true,
    );
}

#[path = "archive_pending_successor.rs"]
mod pending_state;
