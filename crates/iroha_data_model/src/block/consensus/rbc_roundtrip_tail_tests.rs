#[test]
fn rbc_init_roundtrip_codec() {
    let init = sample_rbc_init();
    let bytes = init.encode();
    let dec = RbcInit::decode(&mut &bytes[..]).expect("decode rbc init");
    assert_eq!(init, dec);
}
#[test]
fn rbc_chunk_roundtrip_codec() {
    let chunk = sample_rbc_chunk();
    let bytes = chunk.encode();
    let dec = RbcChunk::decode(&mut &bytes[..]).expect("decode rbc chunk");
    assert_eq!(chunk, dec);
}
#[test]
fn rbc_ready_roundtrip_codec() {
    let ready = sample_rbc_ready();
    let bytes = ready.encode();
    let dec = RbcReady::decode(&mut &bytes[..]).expect("decode rbc ready");
    assert_eq!(ready, dec);
}
#[test]
fn rbc_deliver_roundtrip_codec() {
    let deliver = sample_rbc_deliver();
    let bytes = deliver.encode();
    let dec = RbcDeliver::decode(&mut &bytes[..]).expect("decode rbc deliver");
    assert_eq!(deliver, dec);
}
