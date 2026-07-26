use soranet_relay::{
    capability::{
        self, ConstantRateCapability, ConstantRateMode, KemAdvertisement, KemId,
        NegotiatedCapabilities, ServerCapabilities, SignatureAdvertisement, SignatureId,
        TYPE_CONSTANT_RATE, TYPE_PADDING, TYPE_PQ_KEM, TYPE_PQ_SIG, parse_client_advertisement,
    },
    constant_rate::CONSTANT_RATE_CELL_BYTES,
    handshake::{CLIENT_HELLO_TYPE, ClientHello, NOISE_PADDING_BLOCK},
};

fn build_client_hello(capabilities: &[u8]) -> Vec<u8> {
    const NONCE: [u8; 32] = [0xAB; 32];
    const EPHEMERAL: [u8; 32] = [0x11; 32];
    let kem_public = [0x01, 0x02, 0x03, 0x04];

    let mut frame = Vec::new();
    frame.push(CLIENT_HELLO_TYPE);

    frame.extend_from_slice(&(NONCE.len() as u16).to_be_bytes());
    frame.extend_from_slice(&NONCE);

    frame.push(KemId::MlKem768.code());
    frame.push(SignatureId::Dilithium3.code());

    frame.extend_from_slice(&EPHEMERAL);

    frame.extend_from_slice(&(kem_public.len() as u16).to_be_bytes());
    frame.extend_from_slice(&kem_public);

    frame.extend_from_slice(&(capabilities.len() as u16).to_be_bytes());
    frame.extend_from_slice(capabilities);

    frame.push(0); // resume flag disabled

    let rem = frame.len() % NOISE_PADDING_BLOCK;
    if rem != 0 {
        frame.resize(frame.len() + NOISE_PADDING_BLOCK - rem, 0);
    }

    frame
}

fn encode_tlv(ty: u16, value: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + value.len());
    bytes.extend_from_slice(&ty.to_be_bytes());
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value);
    bytes
}

fn client_capabilities(constant: Option<ConstantRateCapability>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&encode_tlv(TYPE_PQ_KEM, &[KemId::MlKem768.code(), 0x01]));
    bytes.extend_from_slice(&encode_tlv(
        TYPE_PQ_SIG,
        &[SignatureId::Dilithium3.code(), 0x01],
    ));
    bytes.extend_from_slice(&encode_tlv(TYPE_PADDING, &1024u16.to_le_bytes()));
    if let Some(cap) = constant {
        let value = cap.encode_value();
        bytes.extend_from_slice(&encode_tlv(TYPE_CONSTANT_RATE, &value));
    }
    bytes
}

fn constant_rate_capability(mode: ConstantRateMode) -> ConstantRateCapability {
    ConstantRateCapability {
        version: 1,
        mode,
        cell_bytes: CONSTANT_RATE_CELL_BYTES as u16,
    }
}

fn server_caps_with_constant_rate() -> ServerCapabilities {
    ServerCapabilities::new(
        vec![KemAdvertisement {
            id: KemId::MlKem768,
            required: true,
        }],
        vec![SignatureAdvertisement {
            id: SignatureId::Dilithium3,
            required: true,
        }],
        1024,
        None,
        0x01,
        Some(constant_rate_capability(ConstantRateMode::Strict)),
    )
}

fn handshake_caps(
    client_caps: &[u8],
    server_caps: &ServerCapabilities,
) -> Result<NegotiatedCapabilities, capability::CapabilityError> {
    let advert = parse_client_advertisement(client_caps)?;
    capability::negotiate_capabilities(&advert, server_caps)
}

#[test]
fn constant_rate_handshake_roundtrip() {
    let caps = client_capabilities(Some(constant_rate_capability(ConstantRateMode::Strict)));
    let frame = build_client_hello(&caps);
    let hello = ClientHello::parse(&frame).expect("client hello parses");
    let negotiated = handshake_caps(hello.raw_capabilities(), &server_caps_with_constant_rate())
        .expect("negotiate");
    assert_eq!(
        negotiated.constant_rate,
        Some(constant_rate_capability(ConstantRateMode::Strict))
    );
}

#[test]
fn constant_rate_handshake_rejects_mismatch() {
    let caps = client_capabilities(Some(constant_rate_capability(ConstantRateMode::Strict)));
    let frame = build_client_hello(&caps);
    let hello = ClientHello::parse(&frame).expect("client hello parses");

    let server = ServerCapabilities::new(
        vec![KemAdvertisement {
            id: KemId::MlKem768,
            required: true,
        }],
        vec![SignatureAdvertisement {
            id: SignatureId::Dilithium3,
            required: true,
        }],
        1024,
        None,
        0x01,
        None,
    );
    let err =
        handshake_caps(hello.raw_capabilities(), &server).expect_err("should reject mismatch");
    assert!(
        matches!(
            err,
            capability::CapabilityError::ConstantRateUnsupported
                | capability::CapabilityError::ConstantRateStrictRequired
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn constant_rate_profile_applied_when_viewer_omits_tlv() {
    let caps = client_capabilities(None);
    let frame = build_client_hello(&caps);
    let hello = ClientHello::parse(&frame).expect("client hello parses");
    let negotiated = handshake_caps(hello.raw_capabilities(), &server_caps_with_constant_rate())
        .expect("negotiate");
    assert_eq!(
        negotiated.constant_rate,
        Some(constant_rate_capability(ConstantRateMode::Strict)),
        "server should enforce constant-rate even when viewer omits TLV"
    );
}

#[test]
fn constant_rate_multihop_chain_propagates_bits() {
    // viewer -> entry
    let viewer_caps = client_capabilities(None);
    let entry_neg =
        handshake_caps(&viewer_caps, &server_caps_with_constant_rate()).expect("entry negotiation");
    assert!(
        entry_neg.constant_rate.is_some(),
        "entry must enforce constant-rate"
    );

    // entry -> mid
    let forward_to_mid = client_capabilities(entry_neg.constant_rate);
    let mid_neg = handshake_caps(&forward_to_mid, &server_caps_with_constant_rate())
        .expect("mid negotiation");
    assert_eq!(
        mid_neg.constant_rate, entry_neg.constant_rate,
        "mid hop must see the same constant-rate profile"
    );

    // mid -> exit
    let forward_to_exit = client_capabilities(mid_neg.constant_rate);
    let exit_neg = handshake_caps(&forward_to_exit, &server_caps_with_constant_rate())
        .expect("exit negotiation");
    assert_eq!(
        exit_neg.constant_rate, entry_neg.constant_rate,
        "exit hop must inherit the constant-rate profile"
    );
}

#[test]
fn constant_rate_multihop_detects_missing_mid_support() {
    let viewer_caps = client_capabilities(None);
    let entry_neg =
        handshake_caps(&viewer_caps, &server_caps_with_constant_rate()).expect("entry negotiation");
    assert!(entry_neg.constant_rate.is_some());

    let forward_to_mid = client_capabilities(entry_neg.constant_rate);
    let mid_server = ServerCapabilities::new(
        vec![KemAdvertisement {
            id: KemId::MlKem768,
            required: true,
        }],
        vec![SignatureAdvertisement {
            id: SignatureId::Dilithium3,
            required: true,
        }],
        1024,
        None,
        0x01,
        None,
    );
    let err = handshake_caps(&forward_to_mid, &mid_server).expect_err("mid must reject");
    assert!(
        matches!(
            err,
            capability::CapabilityError::ConstantRateUnsupported
                | capability::CapabilityError::ConstantRateStrictRequired
        ),
        "unexpected error: {err:?}"
    );
}
