//! Explicit native test fixture for the real canonical post/grant/partial-I/O chain.
//! This starts no node, listener or network; Tokio duplex forces partial records.
use super::*;
use crate::network::{RelayMessage, RelayTarget};
use iroha_crypto::{Algorithm, KeyPair, encryption::ChaCha20Poly1305};
use iroha_data_model::peer::Peer;
use stream::CreditStream;
type Cipher = ChaCha20Poly1305;
fn pool() -> Arc<Pool> {
    let frames =
        InboundFrameByteBudgets::new(128 * 1024 * 1024, 64 * 1024 * 1024, 17 * 1024 * 1024, 2)
            .unwrap();
    assert!(frames.install_protected_sources(HashSet::new()));
    Pool::new(
        frames,
        InboundDispatchByteBudgets::new(128 * 1024 * 1024, 64 * 1024 * 1024, 2 * 1024 * 1024)
            .unwrap(),
        6,
        [512 * 1024; CLASS_COUNT],
    )
    .unwrap()
}
fn enqueue<T: Pload + ClassifyTopic>(
    stream: &mut CreditStream<Cipher, T>,
    source: &Arc<post_admission::Source>,
    value: T,
) -> oneshot::Receiver<()> {
    let size = checked_data_message_wire_len(&value).unwrap();
    let lease = source
        .reserve(value.admission_class(), size)
        .expect("actual semantic post reservation");
    let mut ownership = OutboundPostOwnership::granted(lease);
    let (tx, rx) = oneshot::channel();
    ownership.flush_ack = Some(tx);
    assert!(stream.enqueue(RetainedPost::new(value, ownership)).is_ok());
    rx
}
/// Drive real signed canonical application envelopes through admitted post bytes
/// and the mandatory grant stream while retaining a prior Payload occurrence.
pub async fn exercise<T: Pload + ClassifyTopic>(key: &KeyPair, payload: T, availability: T) {
    assert_eq!(payload.admission_class(), Class::Payload);
    assert_eq!(availability.admission_class(), Class::Availability);
    let other = KeyPair::try_from_seed(vec![87; 32], Algorithm::BlsNormal).unwrap();
    assert_ne!(key.public_key(), other.public_key());
    let a = Peer::new("127.0.0.1:1337".parse().unwrap(), key.public_key().clone());
    let b = Peer::new(
        "127.0.0.1:1338".parse().unwrap(),
        other.public_key().clone(),
    );
    let payload = RelayMessage::new_signed(key, RelayTarget::Direct(b.id().clone()), 1, payload);
    let availability =
        RelayMessage::new_signed(key, RelayTarget::Direct(b.id().clone()), 1, availability);
    payload.verify_origin_signature().unwrap();
    availability.verify_origin_signature().unwrap();
    let pa = pool();
    let pb = pool();
    let outbound =
        OutboundPostByteBudgets::new(128 * 1024 * 1024, 64 * 1024 * 1024, 17 * 1024 * 1024, 2)
            .unwrap();
    assert!(
        outbound
            .source_geometry
            .install_protected_sources(HashSet::new())
    );
    let post_pool = outbound
        .install_semantic([512 * 1024; CLASS_COUNT])
        .unwrap();
    let source = post_pool.bind(b.id()).unwrap();
    let cipher = cryptographer::Cryptographer::<Cipher>::new_with_raw_key_bytes(&[21; 32]).unwrap();
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"canonical availability grant fixture",
        )),
    );
    let ba = record::Binding::verified(
        &network,
        a.id(),
        b.id(),
        cipher.session_binding,
        [9; 32],
        pa.geometry,
        pb.geometry,
    )
    .unwrap();
    let bb = record::Binding::verified(
        &network,
        b.id(),
        a.id(),
        cipher.session_binding,
        [9; 32],
        pb.geometry,
        pa.geometry,
    )
    .unwrap();
    let (ia, ib) = tokio::io::duplex(7);
    let (ra, wa) = tokio::io::split(ia);
    let (rb, wb) = tokio::io::split(ib);
    let limits = OutboundFrameQueueLimits::new_with_progress_reserve(
        128 * 1024 * 1024,
        64 * 1024 * 1024,
        17 * 1024 * 1024,
        8192,
        4096,
    );
    let caps = crate::network::TopicFrameCaps::uniform(512 * 1024);
    let mut sa = CreditStream::<Cipher, RelayMessage<T>>::new(
        Box::new(ra),
        Box::new(wa),
        pa.bind(b.id()).unwrap(),
        ba,
        cipher.clone(),
        b,
        700,
        caps,
        [512 * 1024; CLASS_COUNT],
        limits,
        Duration::from_secs(20),
    )
    .unwrap();
    let mut sb = CreditStream::<Cipher, RelayMessage<T>>::new(
        Box::new(rb),
        Box::new(wb),
        pb.bind(a.id()).unwrap(),
        bb,
        cipher,
        a,
        701,
        caps,
        [512 * 1024; CLASS_COUNT],
        limits,
        Duration::from_secs(20),
    )
    .unwrap();
    let mut first_ack = enqueue(&mut sa, &source, payload.clone());
    let first=tokio::time::timeout(Duration::from_secs(10),async {
        loop {tokio::select! {a=sa.step()=>{assert!(a.unwrap().is_none());},b=sb.step()=>{if let Some(value)=b.unwrap(){break value;}}}}
    }).await.unwrap();
    assert_eq!(first.granted_admission_class(), Some(Class::Payload));
    first.payload.verify_origin_signature().unwrap();
    tokio::time::timeout(Duration::from_secs(10),async {
        loop { match first_ack.try_recv(){Ok(())=>break,Err(oneshot::error::TryRecvError::Closed)=>panic!("flush owner lost"),Err(oneshot::error::TryRecvError::Empty)=>{}}
            tokio::select! {a=sa.step()=>{assert!(a.unwrap().is_none());},b=sb.step()=>{assert!(b.unwrap().is_none());}}
        }
    }).await.unwrap();
    let _second_ack = enqueue(&mut sa, &source, payload);
    let _availability_ack = enqueue(&mut sa, &source, availability);
    let available=tokio::time::timeout(Duration::from_secs(10),async {
        loop {tokio::select! {a=sa.step()=>{assert!(a.unwrap().is_none());},b=sb.step()=>{if let Some(value)=b.unwrap(){break value;}}}}
    }).await.unwrap();
    assert_eq!(
        available.granted_admission_class(),
        Some(Class::Availability)
    );
    assert_eq!(
        available.payload.topic(),
        crate::network::message::Topic::ConsensusChunk
    );
    available.payload.verify_origin_signature().unwrap();
    tokio::time::timeout(Duration::from_secs(10),async {
        while !sb.request_waits_without_grant(Class::Payload) {
            tokio::select! {a=sa.step()=>{assert!(a.unwrap().is_none());},b=sb.step()=>{assert!(b.unwrap().is_none());}}
        }
    }).await.unwrap();
    drop(first);
    let resumed=tokio::time::timeout(Duration::from_secs(10),async {
        loop {tokio::select! {a=sa.step()=>{assert!(a.unwrap().is_none());},b=sb.step()=>{if let Some(value)=b.unwrap(){break value;}}}}
    }).await.unwrap();
    assert_eq!(resumed.granted_admission_class(), Some(Class::Payload));
    resumed.payload.verify_origin_signature().unwrap();
    drop((available, resumed, sa, sb, source));
    assert_eq!(pb.frames.high.retained_total(), 0);
    assert_eq!(pb.frames.high_decode_scratch.retained_total(), 0);
    assert_eq!(pb.dispatch.high.retained_total(), 0);
    assert_eq!(outbound.shared_high().retained_total(), 0);
}
