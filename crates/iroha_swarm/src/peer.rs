//! Peer utils.
pub type PeerName = String;
pub type P2pApiPorts = [u16; 2];
pub type ExposedKeyPair = (
    iroha_crypto::PublicKey,
    Option<iroha_crypto::ExposedPrivateKey>,
);
pub type PeerPop = Vec<u8>;
#[derive(Debug)]
pub struct PeerInfo {
    pub name: PeerName,
    pub ports: P2pApiPorts,
    pub key_pair: ExposedKeyPair,
    pub soranet_transport_key_pair: ExposedKeyPair,
    /// Dedicated Ed25519 streaming identity rendered into the development environment.
    ///
    /// Prepared validators carry the streaming identity admitted with their runtime config, so
    /// Compose never renders one for them.
    pub streaming_identity_key_pair: Option<ExposedKeyPair>,
    pub pop: PeerPop,
}
pub const SERVICE_NAME: &str = "irohad";
type Result<T> = std::result::Result<T, iroha_crypto::Error>;
const SORANET_TRANSPORT_SEED_DOMAIN: &[u8] = b"iroha:swarm:soranet-transport:v1|";
const STREAMING_IDENTITY_SEED_DOMAIN: &[u8] = b"iroha:swarm:streaming-identity:v1|";
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
#[cfg(test)]
pub fn generate_key_pair(base_seed: Option<&[u8]>, extra_seed: &[u8]) -> Result<ExposedKeyPair> {
    let key_pair = match base_seed {
        Some(seed) => iroha_crypto::KeyPair::try_from_seed(
            seed.iter().chain(extra_seed).copied().collect::<Vec<_>>(),
            iroha_crypto::Algorithm::default(),
        )?,
        None => iroha_crypto::KeyPair::try_random()?,
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((
        public_key,
        Some(iroha_crypto::ExposedPrivateKey(private_key)),
    ))
}
pub fn generate_bls_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<(ExposedKeyPair, PeerPop)> {
    let kp = match base_seed {
        Some(seed) => {
            let material = seed.iter().chain(extra_seed).copied().collect::<Vec<_>>();
            iroha_crypto::KeyPair::try_from_seed(material, iroha_crypto::Algorithm::BlsNormal)?
        }
        None => {
            iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)?
        }
    };
    let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())?;
    let (public_key, private_key) = kp.into_parts();
    Ok((
        (
            public_key,
            Some(iroha_crypto::ExposedPrivateKey(private_key)),
        ),
        pop,
    ))
}
/// Derive a domain-separated Ed25519 key pair from `base_seed ‖ seed_domain ‖ extra_seed`,
/// or draw a random one without a base seed.
fn generate_ed25519_key_pair(
    base_seed: Option<&[u8]>,
    seed_domain: &[u8],
    extra_seed: &[u8],
) -> Result<ExposedKeyPair> {
    let key_pair = match base_seed {
        Some(seed) => iroha_crypto::KeyPair::try_from_seed(
            seed.iter()
                .chain(seed_domain)
                .chain(extra_seed)
                .copied()
                .collect::<Vec<_>>(),
            iroha_crypto::Algorithm::Ed25519,
        )?,
        None => iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)?,
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((
        public_key,
        Some(iroha_crypto::ExposedPrivateKey(private_key)),
    ))
}
pub fn generate_soranet_transport_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<ExposedKeyPair> {
    generate_ed25519_key_pair(base_seed, SORANET_TRANSPORT_SEED_DOMAIN, extra_seed)
}
/// Derive the dedicated Ed25519 streaming identity of one development validator.
///
/// The seed domain differs from the `SoraNet` transport domain, so the two identities of a
/// validator are independent keys.
pub fn generate_streaming_identity_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<ExposedKeyPair> {
    generate_ed25519_key_pair(base_seed, STREAMING_IDENTITY_SEED_DOMAIN, extra_seed)
}
pub fn network(
    count: u16,
    key_seed: Option<&[u8]>,
) -> Result<std::collections::BTreeMap<u16, PeerInfo>> {
    (0..count)
        .map(|nth| {
            let name = format!("{SERVICE_NAME}{nth}");
            let ports = [super::BASE_PORT_P2P + nth, super::BASE_PORT_API + nth];
            let (key_pair, pop) = generate_bls_key_pair(key_seed, &nth.to_be_bytes())?;
            let soranet_transport_key_pair =
                generate_soranet_transport_key_pair(key_seed, &nth.to_be_bytes())?;
            let streaming_identity_key_pair =
                generate_streaming_identity_key_pair(key_seed, &nth.to_be_bytes())?;
            Ok((
                nth,
                PeerInfo {
                    name,
                    ports,
                    key_pair,
                    soranet_transport_key_pair,
                    streaming_identity_key_pair: Some(streaming_identity_key_pair),
                    pop,
                },
            ))
        })
        .collect()
}
pub fn chain() -> iroha_model_base::chain::ChainId {
    iroha_model_base::chain::ChainId::from(crate::CHAIN_ID)
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
        .map(|peer_info| {
            peer(
                &peer_info.name,
                peer_info.ports[0],
                peer_info.key_pair.0.clone(),
            )
        })
        .collect()
}
