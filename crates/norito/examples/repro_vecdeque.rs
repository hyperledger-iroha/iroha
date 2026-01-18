use norito::NoritoSerialize;
fn main() {
    use std::collections::VecDeque;
    let mut dq = VecDeque::new();
    dq.push_back(String::from("a"));
    dq.push_back(String::from("b"));
    let bytes = norito::core::to_bytes(&dq).expect("to_bytes");
    // Header flags are at offset 39 (NRT0 + major + minor + schema16 + compression + len8 + crc8 + flags)
    println!("hdr flags={:02x}", bytes[39]);
    match norito::decode_from_bytes::<VecDeque<String>>(&bytes) {
        Ok(v) => println!("ok {}", v.len()),
        Err(e) => println!("err: {e}"),
    }

    // BinaryHeap
    use std::collections::BinaryHeap;
    let mut heap = BinaryHeap::new();
    heap.push(3i64);
    heap.push(1);
    heap.push(2);
    let hb = norito::core::to_bytes(&heap).unwrap();
    println!("heap flags={:02x}", hb[39]);
    match norito::decode_from_bytes::<BinaryHeap<i64>>(&hb) {
        Ok(h) => println!("heap ok {}", h.len()),
        Err(e) => println!("heap err: {e}"),
    }

    let mut heap2 = BinaryHeap::new();
    for i in 0..200i64 {
        heap2.push(i * 13 - 7);
    }
    let hb2 = norito::core::to_bytes(&heap2).unwrap();
    println!("heap2 flags={:02x}", hb2[39]);
    println!(
        "heap2 first bytes: {}",
        hb2.iter()
            .skip(40)
            .take(12)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    match norito::decode_from_bytes::<BinaryHeap<i64>>(&hb2) {
        Ok(h) => println!("heap2 ok {} first={}", h.len(), *h.peek().unwrap()),
        Err(e) => println!("heap2 err: {e}"),
    }

    // Bare codec path
    let bytes_bare = norito::codec::Encode::encode(&dq);
    println!(
        "bare bytes: {}",
        bytes_bare
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    match <VecDeque<String> as norito::codec::Decode>::decode(&mut &bytes_bare[..]) {
        Ok(v) => println!("bare ok {v:?}"),
        Err(e) => println!("bare err: {e}"),
    }

    // Inspect flags usage inside serialize
    let __flags: u8 = 0;
    let _g = norito::core::DecodeFlagsGuard::enter(__flags);
    println!(
        "ucl={} flags={:02x}",
        norito::core::use_compact_len(),
        __flags
    );
    let mut w = Vec::new();
    dq.serialize(&mut w).unwrap();
    println!(
        "manual bytes: {}",
        w.iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}
