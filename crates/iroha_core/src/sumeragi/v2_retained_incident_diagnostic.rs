/// Decode public intent coordinates without opening a writable WAL or minting replay authority.
pub(in crate::sumeragi) fn retained_wal_incident_metadata_for_test(
    path: &std::path::Path,
    context_id: [u8; 32],
    height: u64,
) -> Vec<String> {
    use std::io::Read as _;

    let maximum = super::safety_wal::SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES
        + super::safety_wal::SAFETY_WAL_MAX_RECORDS
            * (reducer::SAFETY_WAL_FRAME_HEADER_LEN + reducer::SAFETY_WAL_HASH_LEN)
        + reducer::SAFETY_WAL_FILE_HEADER_LEN;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .expect("open retained WAL read-only")
        .take(u64::try_from(maximum + 1).expect("bounded WAL size fits u64"))
        .read_to_end(&mut bytes)
        .expect("read bounded retained WAL");
    assert!(
        bytes.len() <= maximum,
        "retained WAL exceeds recovery bound"
    );
    assert!(bytes.len() >= reducer::SAFETY_WAL_FILE_HEADER_LEN);

    // This diagnostic selects the public network/key identity from the declared
    // fixed V1 header. The native decoder checks its revision, checksum and every
    // frame; the independently decoded ledger supplies the context and height.
    // This is structural inspection, not roster authentication or signing authority.
    let network_offset = reducer::SAFETY_WAL_FILE_MAGIC.len() + 2 + 2;
    let context_offset = network_offset + reducer::SAFETY_WAL_HASH_LEN;
    let height_offset = context_offset + reducer::SAFETY_WAL_HASH_LEN;
    let key_offset = height_offset + 8;
    let identity = reducer::WalFileIdentity::new(
        wire::PROTOCOL_VERSION,
        bytes[network_offset..context_offset]
            .try_into()
            .expect("network hash width"),
        reducer::ContextId::new(context_id),
        height,
        bytes[key_offset..key_offset + reducer::SAFETY_WAL_HASH_LEN]
            .try_into()
            .expect("public consensus key hash width"),
    );
    let recovered = reducer::recover_wal_file(&bytes, identity, &|frame: &[u8]| {
        *blake3::hash(frame).as_bytes()
    })
    .expect("retained WAL has a canonical header and complete hash-chained prefix");
    assert!(recovered.records().len() <= super::safety_wal::SAFETY_WAL_MAX_RECORDS);
    assert!(
        recovered
            .records()
            .iter()
            .map(|frame| frame.payload().len())
            .sum::<usize>()
            <= super::safety_wal::SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES
    );
    let mut summaries = vec![format!(
        "wal context={} height={height} records={} incomplete_tail={}",
        hex::encode(context_id),
        recovered.records().len(),
        recovered.has_incomplete_tail(),
    )];
    for frame in recovered.records() {
        let mut payload = frame.payload();
        let envelope = WalEnvelopeV2::decode(&mut payload)
            .expect("retained WAL intent has valid native Norito encoding");
        assert!(payload.is_empty(), "retained WAL intent has trailing bytes");
        assert_eq!(
            envelope.encode(),
            frame.payload(),
            "retained WAL intent is canonical"
        );
        assert_eq!(envelope.protocol_version, wire::PROTOCOL_VERSION);
        assert_eq!(
            envelope.persistence_id,
            frame.sequence().checked_add(1).unwrap()
        );
        let (kind, round, proposal_round, subject, manifest) = match &envelope.record {
            WalRecordV2::ProposalIntent(proposal) => (
                "ProposalIntent",
                proposal.round,
                Some(proposal.round),
                Some(proposal.subject),
                Some(HashOf::new(&proposal.manifest)),
            ),
            WalRecordV2::PrepareIntent(vote) => (
                "PrepareIntent",
                vote.round,
                Some(vote.proposal_round),
                Some(vote.subject),
                None,
            ),
            WalRecordV2::ObservePrepare(certificate) => (
                "ObservePrepare",
                certificate.round,
                Some(certificate.proposal_round),
                Some(certificate.subject),
                None,
            ),
            WalRecordV2::LockAndCommit { vote, .. } => (
                "LockAndCommit",
                vote.round,
                Some(vote.proposal_round),
                Some(vote.subject),
                None,
            ),
            WalRecordV2::TimeoutIntent(vote) => ("TimeoutIntent", vote.round, None, None, None),
            WalRecordV2::InstallTimeout(certificate) => {
                ("InstallTimeout", certificate.round, None, None, None)
            }
            WalRecordV2::Decision(certificate) => (
                "Decision",
                certificate.round,
                Some(certificate.proposal_round),
                Some(certificate.subject),
                None,
            ),
        };
        assert_eq!(round.height, height);
        assert_eq!(round.context_id.0.as_ref(), context_id.as_slice());
        summaries.push(format!(
            "wal sequence={} persistence={} kind={kind} round={round:?} proposal_round={proposal_round:?} subject={subject:?} manifest={manifest:?}",
            frame.sequence(), envelope.persistence_id,
        ));
    }
    summaries
}
