//! Lifecycle and original-custody controls; token values confer no proof verdict.

use super::*;
use crate::finality::{
    continuity::tree::OriginalBytes,
    native::{ArtifactId, Program},
};
use std::{cell::Cell, rc::Rc};

fn pair() -> OriginalPair {
    OriginalPair {
        source: OriginalBytes {
            descriptor: vec![1, 2],
            verifying_key: vec![3, 4],
            proving_key: vec![5, 6],
        },
        wrapper: OriginalBytes {
            descriptor: vec![7, 8],
            verifying_key: vec![9, 10],
            proving_key: vec![11, 12],
        },
    }
}

fn id(class: u32) -> NodeId {
    NodeId::Leaf(Program::Result, class)
}

struct Originals {
    pair: OriginalPair,
    loads: usize,
    fail: Option<(usize, Error)>,
    cancellation: Option<(usize, CancellationToken)>,
    resident: Rc<Cell<usize>>,
    expected_resident: usize,
}

impl Originals {
    fn new() -> Self {
        Self {
            pair: pair(),
            loads: 0,
            fail: None,
            cancellation: None,
            resident: Rc::new(Cell::new(0)),
            expected_resident: 0,
        }
    }
}

impl ArtifactSource for Originals {
    fn load(&mut self, artifact: &ArtifactId) -> Result<OriginalBytes, Error> {
        assert_eq!(self.resident.get(), self.expected_resident);
        self.loads += 1;
        if let Some((at, error)) = self.fail {
            if at == self.loads {
                return Err(error);
            }
        }
        if let Some((at, token)) = &self.cancellation {
            if *at == self.loads {
                token.cancel();
            }
        }
        match artifact {
            ArtifactId::Source(_) => Ok(self.pair.source.clone()),
            ArtifactId::Wrapper(_) => Ok(self.pair.wrapper.clone()),
            _ => panic!("only the selected source and wrapper are loaded"),
        }
    }
}

struct Token(Rc<Cell<usize>>);

impl Token {
    fn new(resident: &Rc<Cell<usize>>) -> Self {
        assert_eq!(
            resident.get(),
            0,
            "previous pair must be dropped before import"
        );
        resident.set(1);
        Self(Rc::clone(resident))
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        assert_eq!(self.0.get(), 1);
        self.0.set(0);
    }
}

#[test]
fn identical_originals_reload_both_roles_but_import_once() {
    let mut originals = Originals::new();
    let resident = Rc::clone(&originals.resident);
    let mut cache = LastImported::new(8);
    let mut imports = 0;
    for iteration in 0..3 {
        originals.expected_resident = usize::from(iteration != 0);
        let value = cache
            .get_or_import(&id(1), &mut originals, None, |_| {
                imports += 1;
                Ok(Token::new(&resident))
            })
            .unwrap();
        assert!(Rc::ptr_eq(&value.0, &resident));
        assert_eq!(imports, 1);
        assert_eq!(originals.loads, (iteration + 1) * 2);
    }
    drop(cache);
    assert_eq!(resident.get(), 0);
}

#[test]
fn each_changed_original_role_refuses_and_discards_cached_pair() {
    for role in 0..6 {
        let mut originals = Originals::new();
        let resident = Rc::clone(&originals.resident);
        let mut cache = LastImported::new(8);
        cache
            .get_or_import(&id(1), &mut originals, None, |_| Ok(Token::new(&resident)))
            .unwrap();
        let fields = [
            &mut originals.pair.source.descriptor,
            &mut originals.pair.source.verifying_key,
            &mut originals.pair.source.proving_key,
            &mut originals.pair.wrapper.descriptor,
            &mut originals.pair.wrapper.verifying_key,
            &mut originals.pair.wrapper.proving_key,
        ];
        fields[role][0] ^= 0x80; // Same length: resource accounting alone misses this.
        originals.expected_resident = 1;
        let result = cache.get_or_import(&id(1), &mut originals, None, |_| {
            panic!("changed same-class originals must not be silently reimported")
        });
        assert_eq!(result.err(), Some(Error::Artifact), "role {role}");
        assert!(cache.entry.is_none());
        assert_eq!(resident.get(), 0);
    }
}

#[test]
fn unavailable_original_and_loader_cancellation_cannot_use_cached_pair() {
    for role in 3..=4 {
        for error in [Error::Artifact, Error::Cancelled] {
            let mut originals = Originals::new();
            let resident = Rc::clone(&originals.resident);
            let mut cache = LastImported::new(8);
            cache
                .get_or_import(&id(1), &mut originals, None, |_| Ok(Token::new(&resident)))
                .unwrap();
            originals.expected_resident = 1;
            originals.fail = Some((role, error));
            assert_eq!(
                cache
                    .get_or_import(&id(1), &mut originals, None, |_| {
                        panic!("unavailable originals cannot authorize cached tables")
                    })
                    .err(),
                Some(error)
            );
            assert_eq!(originals.loads, role);
            assert!(cache.entry.is_none());
            assert_eq!(resident.get(), 0);
        }
    }
}

#[test]
fn class_change_evicts_before_loader_and_returning_class_reimports() {
    let mut originals = Originals::new();
    let resident = Rc::clone(&originals.resident);
    let mut cache = LastImported::new(8);
    let mut imports = 0;
    for class in [1, 2, 1] {
        // Loader requires zero resident even though the prior call left one.
        cache
            .get_or_import(&id(class), &mut originals, None, |_| {
                imports += 1;
                Ok(Token::new(&resident))
            })
            .unwrap();
        assert_eq!(resident.get(), 1);
    }
    assert_eq!(imports, 3);
    assert_eq!(originals.loads, 6);
    cache.select(&id(9)); // Same selection occurs before checkpoint restoration.
    assert_eq!(resident.get(), 0);
    assert!(cache.entry.is_none());
}

#[test]
fn strict_import_failure_never_installs_a_partial_pair() {
    let mut originals = Originals::new();
    let resident = Rc::clone(&originals.resident);
    let mut cache = LastImported::<Token>::new(8);
    for error in [Error::Artifact, Error::Cancelled] {
        assert_eq!(
            cache
                .get_or_import(&id(1), &mut originals, None, |_| Err(error))
                .err(),
            Some(error)
        );
        assert!(cache.entry.is_none());
    }
    cache
        .get_or_import(&id(1), &mut originals, None, |_| Ok(Token::new(&resident)))
        .unwrap();
    // New-class failure happens after eviction, not while both values are alive.
    assert_eq!(
        cache
            .get_or_import(&id(2), &mut originals, None, |_| Err(Error::Artifact))
            .err(),
        Some(Error::Artifact)
    );
    assert_eq!(resident.get(), 0);
    assert!(cache.entry.is_none());
}

#[test]
fn cancellation_before_reuse_after_reload_and_after_import_discards_pair() {
    for after_load in [None, Some(4)] {
        let mut originals = Originals::new();
        let resident = Rc::clone(&originals.resident);
        let mut cache = LastImported::new(8);
        let token = CancellationToken::new();
        cache
            .get_or_import(&id(1), &mut originals, None, |_| Ok(Token::new(&resident)))
            .unwrap();
        originals.expected_resident = 1;
        if let Some(at) = after_load {
            originals.cancellation = Some((at, token.clone()));
        } else {
            token.cancel();
        }
        assert_eq!(
            cache
                .get_or_import(&id(1), &mut originals, Some(&token), |_| {
                    panic!("cancelled cache use never imports")
                })
                .err(),
            Some(Error::Cancelled)
        );
        assert_eq!(originals.loads, after_load.unwrap_or(2));
        assert_eq!(resident.get(), 0);
        assert!(cache.entry.is_none());
    }
    let mut originals = Originals::new();
    let resident = Rc::clone(&originals.resident);
    let mut cache = LastImported::new(8);
    let token = CancellationToken::new();
    assert_eq!(
        cache
            .get_or_import(&id(1), &mut originals, Some(&token), |_| {
                token.cancel();
                Ok(Token::new(&resident))
            })
            .err(),
        Some(Error::Cancelled)
    );
    assert_eq!(resident.get(), 0);
    assert!(cache.entry.is_none());
}

#[test]
fn identity_hashes_all_chunks_lengths_and_role_order() {
    let mut original = pair();
    original.source.proving_key = vec![19; HASH_CHUNK_BYTES + 1];
    let expected = PairIdentity::read(&original, HASH_CHUNK_BYTES + 2, None).unwrap();
    assert_eq!(expected.0[2].bytes, HASH_CHUNK_BYTES + 1);
    assert_eq!(
        expected.0[2].sha256,
        <[u8; 32]>::from(Sha256::digest(&original.source.proving_key))
    );
    for index in [0, HASH_CHUNK_BYTES - 1, HASH_CHUNK_BYTES] {
        let mut changed = original.clone();
        changed.source.proving_key[index] ^= 1;
        assert_ne!(
            PairIdentity::read(&changed, HASH_CHUNK_BYTES + 2, None).unwrap(),
            expected
        );
    }
    let mut changed = original.clone();
    changed.source.proving_key.push(19);
    assert_ne!(
        PairIdentity::read(&changed, HASH_CHUNK_BYTES + 2, None).unwrap(),
        expected
    );
    std::mem::swap(&mut original.source, &mut original.wrapper);
    assert_ne!(
        PairIdentity::read(&original, HASH_CHUNK_BYTES + 2, None).unwrap(),
        expected
    );
}

#[test]
fn empty_oversize_and_cancelled_identity_reads_refuse() {
    for role in 0..6 {
        let mut original = pair();
        let fields = [
            &mut original.source.descriptor,
            &mut original.source.verifying_key,
            &mut original.source.proving_key,
            &mut original.wrapper.descriptor,
            &mut original.wrapper.verifying_key,
            &mut original.wrapper.proving_key,
        ];
        fields[role].clear();
        assert_eq!(
            PairIdentity::read(&original, 8, None).err(),
            Some(Error::Artifact)
        );
    }
    assert_eq!(
        PairIdentity::read(&pair(), 1, None).err(),
        Some(Error::Artifact)
    );
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        PairIdentity::read(&pair(), 8, Some(&token)).err(),
        Some(Error::Cancelled)
    );
}
