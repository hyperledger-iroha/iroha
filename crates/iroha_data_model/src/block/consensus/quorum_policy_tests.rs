#[test]
fn qc_vote_roundtrip_codec_and_decode_from_slice() {
    let vote = QcVote {
        phase: CertPhase::Commit,
        block_hash: dummy_hash(),
        parent_state_root: Hash::new(b"parent_root"),
        post_state_root: Hash::new(b"post_root"),
        height: 7,
        view: 2,
        epoch: 0,
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 3,
        bls_sig: vec![0x01, 0x02],
    };
    let bytes = vote.encode();
    let dec = QcVote::decode(&mut &bytes[..]).expect("decode qc vote");
    assert_eq!(vote, dec);
    let (slice_dec, used) = QcVote::decode_from_slice(&bytes).expect("decode_from_slice qc vote");
    assert_eq!(vote, slice_dec);
    assert_eq!(used, bytes.len());
}
#[test]
fn vrf_commit_roundtrip_codec() {
    let commit = sample_vrf_commit();
    let bytes = commit.encode();
    let dec = VrfCommit::decode(&mut &bytes[..]).expect("decode vrf commit");
    assert_eq!(commit, dec);
}
#[test]
fn vrf_reveal_roundtrip_codec() {
    let reveal = sample_vrf_reveal();
    let bytes = reveal.encode();
    let dec = VrfReveal::decode(&mut &bytes[..]).expect("decode vrf reveal");
    assert_eq!(reveal, dec);
}
#[test]
fn reconfig_roundtrip_codec() {
    let reconfig = sample_reconfig();
    let bytes = reconfig.encode();
    let dec = Reconfig::decode(&mut &bytes[..]).expect("decode reconfig");
    assert_eq!(reconfig, dec);
}
