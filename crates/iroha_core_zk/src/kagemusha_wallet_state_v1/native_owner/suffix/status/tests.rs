//! Exact request absence is authenticated scheduling DATA, using mock custody only.
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{bootstrap, wallet};

fn namespace(scheme_id: [u8; 32], wallet_id: [u8; 32]) -> SuffixArchiveManifestV1 {
    SuffixArchiveManifestV1 {
        version: 1,
        discriminator: 2,
        scheme_id,
        wallet_id,
        requests: IndexRoot::default(),
        authorities: IndexRoot::default(),
        operations: IndexRoot::default(),
        source_pins: IndexRoot::default(),
        ledger_attempts: IndexRoot::default(),
        ledger_outcomes: IndexRoot::default(),
        ledger_stages: IndexRoot::default(),
        active_request: None,
    }
}

#[test]
fn named_resume_absence_requires_an_actual_selected_index_miss() {
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let (root, manifest) = w.sync_manifest().unwrap();
    let requested = [71; 32];
    let selected = w.selected_suffix_checkpoint().unwrap().unwrap();
    let ControlFlow::Break(absent) = w.suffix_resume_observation(&selected, &requested).unwrap()
    else {
        panic!("unknown request must be precisely absent");
    };
    assert_eq!(absent.state, 7);
    assert_eq!(absent.request_id, Some(requested));
    assert_eq!(absent.current_sequence, manifest.indexed);
    assert_eq!(absent.folded_sequence, manifest.folded);
    assert_eq!((absent.completed_steps, absent.total_steps), (0, 0));
    assert_eq!(
        SuffixUnloadStatusV1::decode_canonical(&absent.to_canonical_bytes().unwrap()).unwrap(),
        absent
    );
    assert_ne!(w.suffix_unload_status().unwrap().state, 7);
    assert!(w.suffix_resume_observation(&selected, &[0; 32]).is_err());

    let mut suffix = namespace(w.scheme_id, w.wallet_id);
    // A missing referenced record is not an absent request, even with a valid selected index.
    suffix.requests = suffix
        .requests
        .set(&mut w.archive, requested, &[72; 32])
        .unwrap();
    let root = w
        .publish_suffix_checkpoint(root, &manifest, &suffix)
        .unwrap();
    let selected = w.selected_suffix_checkpoint().unwrap().unwrap();
    assert!(w.suffix_resume_observation(&selected, &requested).is_err());
    assert!(matches!(
        w.suffix_resume_observation(&selected, &[73; 32]),
        Ok(ControlFlow::Break(_))
    ));

    // Malformed index payload is also an error, never converted into scheduling absence.
    suffix.requests = suffix
        .requests
        .set(&mut w.archive, requested, &[1, 2, 3])
        .unwrap();
    w.publish_suffix_checkpoint(root, &manifest, &suffix)
        .unwrap();
    let selected = w.selected_suffix_checkpoint().unwrap().unwrap();
    assert!(w.suffix_resume_observation(&selected, &requested).is_err());
}

#[test]
fn malformed_referenced_request_original_does_not_become_not_started() {
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let (root, manifest) = w.sync_manifest().unwrap();
    let requested = [81; 32];
    let corrupt = w.archive.write_object(&[1, 2, 3], 64).unwrap();
    let mut suffix = namespace(w.scheme_id, w.wallet_id);
    suffix.requests = suffix
        .requests
        .set(&mut w.archive, requested, &corrupt)
        .unwrap();
    w.publish_suffix_checkpoint(root, &manifest, &suffix)
        .unwrap();
    let selected = w.selected_suffix_checkpoint().unwrap().unwrap();
    assert!(w.suffix_resume_observation(&selected, &requested).is_err());
}
