//! Incremental reads of granted ciphertext and fixed control records.
use super::super::tests::{crypto, peer, pool};
use super::*;
use futures::FutureExt as _;
use iroha_crypto::encryption::ChaCha20Poly1305;

#[test]
fn reader_accepts_only_alignment_funded_by_the_envelope() {
    for alignment in [1, 8, 64, 512] {
        assert!(Reading::new(alignment, MAX_CRYPTO_OVERHEAD).is_ok());
    }
    for alignment in [0, 3, 1024] {
        assert!(matches!(
            Reading::new(alignment, MAX_CRYPTO_OVERHEAD),
            Err(Error::Format)
        ));
    }
    assert!(matches!(
        Reading::new(1, MAX_CRYPTO_OVERHEAD + 1),
        Err(Error::Format)
    ));
}

#[tokio::test]
async fn partial_granted_body_resumes_without_consuming_the_next_record() {
    let mut reading =
        Reading::new(64, record::encrypted_len::<ChaCha20Poly1305>(0).unwrap()).unwrap();
    let mut ledger = Ledger::new(pool(3).bind(peer(37).id()).unwrap());
    let binding = Binding {
        incoming: [1; 32],
        outgoing: [2; 32],
    };
    let cipher = crypto();
    let plaintext = [0xa5; 900];
    let request = Header::request(binding.incoming, Class::Payload, 1, plaintext.len()).unwrap();
    ledger.request(request).unwrap();
    let data = ledger.next_grant().unwrap().unwrap().with_kind(Kind::Data);
    let (header, sealed) = record::seal(&cipher, &data, &plaintext).unwrap();
    let ping = Header::ping(binding.incoming, 1).unwrap();
    let next_record = record::seal_record(&cipher, &ping, &[]).unwrap();
    let (mut writer, mut input) = tokio::io::duplex(4096);
    let prefix = u32::try_from(header.len()).unwrap().to_be_bytes();
    writer.write_all(&prefix[..2]).await.unwrap();
    assert!(
        !reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await
            .unwrap()
    );
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .now_or_never()
            .is_none()
    );
    writer.write_all(&prefix[2..]).await.unwrap();
    writer.write_all(&header[..5]).await.unwrap();
    assert!(
        !reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await
            .unwrap()
    );
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .now_or_never()
            .is_none()
    );
    assert_eq!(reading.body.capacity(), 0);
    assert!(ledger.grants[Class::Payload.index()].is_some());
    writer.write_all(&header[5..]).await.unwrap();

    // Cancel when the buffered header has spent its grant but no body has arrived.
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .now_or_never()
            .is_none()
    );
    assert!(reading.reservation.is_some());
    assert_eq!(reading.body.len(), reading.body_start);
    let allocation = reading.body.as_ptr();
    let split = 7;
    writer.write_all(&sealed[..split]).await.unwrap();
    assert!(
        !reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await
            .unwrap()
    );
    assert_eq!(&reading.body[reading.body_start..], &sealed[..split]);
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .now_or_never()
            .is_none()
    );

    writer.write_all(&sealed[split..]).await.unwrap();
    writer.write_all(&next_record).await.unwrap();
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await
            .unwrap()
    );
    assert_eq!(reading.body.as_ptr(), allocation);
    let opened = record::open(&cipher, &header, &mut reading.body[reading.body_start..]).unwrap();
    assert_eq!(opened.as_ptr() as usize % 64, 0);
    assert_eq!(opened, plaintext);
    reading.reset();

    // All three parts of the buffered Ping finish in one reader turn.
    assert!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await
            .unwrap()
    );
    assert_eq!(reading.parsed, Some(ping));
    assert_eq!(reading.body.as_ptr(), allocation);
    assert!(
        record::open(
            &cipher,
            &reading.header[..reading.header_len],
            &mut reading.body[reading.body_start..]
        )
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn data_without_an_exact_grant_is_refused_before_body_allocation() {
    let mut reading =
        Reading::new(8, record::encrypted_len::<ChaCha20Poly1305>(0).unwrap()).unwrap();
    let mut ledger = Ledger::new(pool(3).bind(peer(38).id()).unwrap());
    let binding = Binding {
        incoming: [1; 32],
        outgoing: [2; 32],
    };
    let cipher = crypto();
    let payload = [1; 512];
    let granted = Header::request(binding.incoming, Class::Lane, 1, payload.len() - 1).unwrap();
    ledger.request(granted).unwrap();
    let grant = ledger.next_grant().unwrap().unwrap();
    let data = Header::request(binding.incoming, Class::Lane, 1, payload.len())
        .unwrap()
        .with_kind(Kind::Data);
    let wire = record::seal_record(&cipher, &data, &payload).unwrap();
    let mut input = wire.as_slice();
    assert!(matches!(
        reading
            .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
            .await,
        Err(Error::Format)
    ));
    assert_eq!(
        input.len(),
        record::encrypted_len::<ChaCha20Poly1305>(payload.len()).unwrap()
    );
    assert_eq!(reading.body.capacity(), 0);
    assert_eq!(
        ledger.grants[Class::Lane.index()].as_ref().unwrap().header,
        grant
    );
}

#[tokio::test]
async fn reset_keeps_storage_only_through_the_retention_limit() {
    let overhead = record::encrypted_len::<ChaCha20Poly1305>(0).unwrap();
    let mut reading = Reading::new(1, overhead).unwrap();
    let frames = InboundFrameByteBudgets::new(256 * 1024, 128 * 1024, 256 * 1024, 2).unwrap();
    assert!(frames.install_protected_sources(HashSet::new()));
    let mut maximum = [1024; CLASS_COUNT];
    maximum[Class::Payload.index()] = RETAINED_RECEIVE_BYTES;
    let pool = Pool::new(
        frames,
        InboundDispatchByteBudgets::new(256 * 1024, 128 * 1024, 4096).unwrap(),
        3,
        maximum,
    )
    .unwrap();
    let mut ledger = Ledger::new(pool.bind(peer(39).id()).unwrap());
    let binding = Binding {
        incoming: [1; 32],
        outgoing: [2; 32],
    };
    let cipher = crypto();
    for (sequence, size) in [(1, RETAINED_RECEIVE_BYTES), (2, RETAINED_RECEIVE_BYTES + 1)] {
        let plaintext = vec![0x31; size - overhead];
        let request =
            Header::request(binding.incoming, Class::Payload, sequence, plaintext.len()).unwrap();
        ledger.request(request).unwrap();
        let data = ledger.next_grant().unwrap().unwrap().with_kind(Kind::Data);
        let wire = record::seal_record(&cipher, &data, &plaintext).unwrap();
        let mut input = wire.as_slice();
        assert!(
            reading
                .advance::<ChaCha20Poly1305>(&mut input, &mut ledger, &binding)
                .await
                .unwrap()
        );
        assert!(input.is_empty());
        assert_eq!(reading.body.len(), size);
        assert_eq!(reading.body.capacity(), size);
        reading.reset();
        assert!(reading.body.is_empty());
        if size == RETAINED_RECEIVE_BYTES {
            assert_eq!(reading.body.capacity(), RETAINED_RECEIVE_BYTES);
        } else {
            assert_eq!(reading.body.capacity(), 0);
        }
    }
}

#[tokio::test]
async fn eof_during_granted_body_fences_stream_before_authentication() {
    use crate::network::admission_class_tests::AdmissionFixture;

    let pool = pool(3);
    let peer = peer(40);
    let (mut remote, transport) = tokio::io::duplex(4096);
    let (read, write) = tokio::io::split(transport);
    let mut stream = CreditStream::<ChaCha20Poly1305, AdmissionFixture>::new(
        negotiation::OwnedTransport {
            read: Box::new(read),
            write: Box::new(write),
            source: pool.bind(peer.id()).unwrap(),
        },
        Binding {
            incoming: [1; 32],
            outgoing: [2; 32],
        },
        crypto(),
        peer,
        101,
        CreditStreamConfig {
            caps: crate::network::TopicFrameCaps::uniform(1024),
            maximum: [1024; CLASS_COUNT],
            limits: OutboundFrameQueueLimits::new_with_progress_reserve(
                256 * 1024,
                128 * 1024,
                256 * 1024,
                32,
                32,
            ),
            idle_timeout: Duration::from_secs(20),
        },
    )
    .unwrap();
    let mut plaintext = Vec::new();
    run::receive_credit_encode(&AdmissionFixture::Payload(11), &mut plaintext).unwrap();
    let request =
        Header::request(stream.binding.incoming, Class::Payload, 1, plaintext.len()).unwrap();
    stream.ledger.request(request).unwrap();
    let data = stream
        .ledger
        .next_grant()
        .unwrap()
        .unwrap()
        .with_kind(Kind::Data);
    let record = record::seal_record(&stream.crypto, &data, &plaintext).unwrap();
    remote.write_all(&record[..record.len() - 1]).await.unwrap();
    drop(remote);

    let error = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match stream.step().await {
                Ok(None) => {}
                Ok(Some(_)) => panic!("delivered a truncated record"),
                Err(error) => break error,
            }
        }
    })
    .await
    .expect("closed transport must report EOF");
    assert!(matches!(error, Error::ConnectionResetByPeer));
    assert!(stream.reading.reservation.is_some());
    assert_eq!(stream.reading.body.len() + 1, stream.reading.body_end);
    assert_eq!(stream.authenticated_records(), 0);
    assert!(matches!(stream.step().await, Err(Error::Format)));
}
