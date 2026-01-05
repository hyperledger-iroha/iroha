//! Peer utils.

pub type PeerInfo = (PeerName, P2pApiPorts, ExposedKeyPair, PeerPop);
pub type PeerName = String;
pub type P2pApiPorts = [u16; 2];
pub type ExposedKeyPair = (iroha_crypto::PublicKey, iroha_crypto::ExposedPrivateKey);
pub type ExposedKeyRefPair<'a> = (
    &'a iroha_crypto::PublicKey,
    &'a iroha_crypto::ExposedPrivateKey,
);
pub type PeerPop = Vec<u8>;

pub const SERVICE_NAME: &str = "irohad";

/// Peer overrides supplied by higher-level tooling.
#[derive(Clone, Debug)]
pub struct PeerOverride {
    /// Human-readable service name for the peer (used in Compose metadata).
    pub name: String,
    /// External P2P port exposed by Docker for this peer service.
    pub p2p_port: u16,
    /// External API port exposed by Docker for this peer service.
    pub api_port: u16,
}

pub fn generate_key_pair(base_seed: Option<&[u8]>, extra_seed: &[u8]) -> ExposedKeyPair {
    let (public_key, private_key) = base_seed
        .map_or_else(iroha_crypto::KeyPair::random, |seed| {
            iroha_crypto::KeyPair::from_seed(
                seed.iter().chain(extra_seed).copied().collect::<Vec<_>>(),
                iroha_crypto::Algorithm::default(),
            )
        })
        .into_parts();
    (public_key, iroha_crypto::ExposedPrivateKey(private_key))
}

pub fn generate_bls_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> (ExposedKeyPair, PeerPop) {
    let kp = base_seed.map_or_else(
        || iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal),
        |seed| {
            let material = seed.iter().chain(extra_seed).copied().collect::<Vec<_>>();
            iroha_crypto::KeyPair::from_seed(material, iroha_crypto::Algorithm::BlsNormal)
        },
    );
    let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("generate BLS PoP");
    let (public_key, private_key) = kp.into_parts();
    (
        (public_key, iroha_crypto::ExposedPrivateKey(private_key)),
        pop,
    )
}

pub fn network(count: u16, key_seed: Option<&[u8]>) -> std::collections::BTreeMap<u16, PeerInfo> {
    (0..count)
        .map(|nth| {
            let name = format!("{SERVICE_NAME}{nth}");
            let ports = [super::BASE_PORT_P2P + nth, super::BASE_PORT_API + nth];
            let (key_pair, pop) = generate_bls_key_pair(key_seed, &nth.to_be_bytes());
            (nth, (name, ports, key_pair, pop))
        })
        .collect()
}

pub fn chain() -> iroha_data_model::ChainId {
    iroha_data_model::ChainId::from(crate::CHAIN_ID)
}

pub fn peer(
    name: &str,
    port: u16,
    public_key: iroha_crypto::PublicKey,
) -> iroha_data_model::peer::Peer {
    iroha_data_model::peer::Peer::new(
        iroha_primitives::addr::SocketAddrHost {
            host: name.to_owned().into(),
            port,
        }
        .into(),
        public_key,
    )
}

#[allow(single_use_lifetimes)]
pub fn topology<'a>(
    peers: impl Iterator<Item = &'a PeerInfo>,
) -> std::collections::BTreeSet<iroha_data_model::peer::Peer> {
    peers
        .map(|(service_name, [port_p2p, _], (public_key, _), _)| {
            peer(service_name, *port_p2p, public_key.clone())
        })
        .collect()
}
