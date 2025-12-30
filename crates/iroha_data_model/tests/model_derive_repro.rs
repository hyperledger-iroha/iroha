//! Repro tests for `#[model]`-derived Norito encode/decode on selected types.
//!
//! These tests compare bare-codec (`codec::Encode/DecodeAll`) and header-framed
//! (`core::to_bytes/from_bytes`) paths to surface any discrepancies in layouts
//! for string wrappers, simple structs, and mixed types.
//!
//! They are ignored by default to keep the suite green; run them explicitly to
//! inspect differences while iterating on derives and/or Norito flags.

use iroha_data_model::{events::time::TimeInterval, prelude::*};

#[test]
fn repro_name_bare_vs_header_valid() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: manual derive repro test. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let name: Name = "valid".parse().expect("valid name");

    // Bare path
    let bare = norito::codec::Encode::encode(&name);
    let name_bare: Name =
        norito::codec::DecodeAll::decode_all(&mut &bare[..]).expect("bare decode");

    // Header-framed path
    let header = norito::to_bytes(&name).expect("to_bytes");
    let archived = norito::from_bytes::<Name>(&header).expect("from_bytes");
    let name_header = norito::core::NoritoDeserialize::deserialize(archived);

    assert_eq!(name_bare.as_ref(), name.as_ref(), "bare mismatch");
    assert_eq!(name_header.as_ref(), name.as_ref(), "header mismatch");
}

#[test]
fn repro_ipfs_bare_vs_header_valid() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: manual derive repro test. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let path: iroha_data_model::ipfs::IpfsPath = "/ipld".parse().expect("valid");

    let bare = norito::codec::Encode::encode(&path);
    let path_bare: iroha_data_model::ipfs::IpfsPath =
        norito::codec::DecodeAll::decode_all(&mut &bare[..]).expect("bare decode");

    let header = norito::to_bytes(&path).expect("to_bytes");
    let archived =
        norito::from_bytes::<iroha_data_model::ipfs::IpfsPath>(&header).expect("from_bytes");
    let path_header = norito::core::NoritoDeserialize::deserialize(archived);

    assert_eq!(path_bare.as_ref(), path.as_ref(), "bare mismatch");
    assert_eq!(path_header.as_ref(), path.as_ref(), "header mismatch");
}

#[test]
fn repro_timeinterval_bare_vs_header() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: manual derive repro test. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let ti = TimeInterval::new(
        core::time::Duration::from_millis(10),
        core::time::Duration::from_millis(20),
    );

    let bare = norito::codec::Encode::encode(&ti);
    let ti_bare: TimeInterval =
        norito::codec::DecodeAll::decode_all(&mut &bare[..]).expect("bare decode");

    let header = norito::to_bytes(&ti).expect("to_bytes");
    let archived = norito::from_bytes::<TimeInterval>(&header).expect("from_bytes");
    let ti_header = norito::core::NoritoDeserialize::deserialize(archived);

    assert_eq!(ti_bare, ti, "bare mismatch");
    assert_eq!(ti_header, ti, "header mismatch");
}

#[test]
fn repro_blocksignature_bare_vs_header() {
    use iroha_crypto::{KeyPair, SignatureOf};
    use iroha_data_model::block::BlockHeader;
    use nonzero_ext::nonzero;
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: manual derive repro test. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }

    let kp = KeyPair::random();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let sig = SignatureOf::from_hash(kp.private_key(), header.hash());
    let bs = iroha_data_model::prelude::BlockSignature::new(42, sig);

    let bare = norito::codec::Encode::encode(&bs);
    let bs_bare: iroha_data_model::prelude::BlockSignature =
        norito::codec::DecodeAll::decode_all(&mut &bare[..]).expect("bare decode");

    let header_bytes = norito::to_bytes(&bs).expect("to_bytes");
    let archived = norito::from_bytes::<iroha_data_model::prelude::BlockSignature>(&header_bytes)
        .expect("from_bytes");
    let bs_header: iroha_data_model::prelude::BlockSignature =
        norito::core::NoritoDeserialize::deserialize(archived);

    assert_eq!(bs_bare, bs, "bare mismatch");
    assert_eq!(bs_header, bs, "header mismatch");
}
