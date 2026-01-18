
use iroha_core::sumeragi::network_topology::{rotated_for_prev_block_hash, Topology};
use iroha_data_model::peer::PeerId;
use iroha_crypto::{KeyPair, HashOf};
use iroha_data_model::block::BlockHeader;

#[test]
fn test_rotated_for_prev_block_hash_determinism() {
    let keys: Vec<KeyPair> = (0..5).map(|_| KeyPair::random()).collect();
    let peers: Vec<PeerId> = keys.iter().map(|k| PeerId::new(k.public_key().clone())).collect();
    
    // Sort peers for reference
    let mut sorted_peers = peers.clone();
    sorted_peers.sort();
    
    // Reverse peers for comparison
    let mut reversed_peers = sorted_peers.clone();
    reversed_peers.reverse();
    
    let prev_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0x11; iroha_crypto::Hash::LENGTH]),
    );
    
    let topo1 = rotated_for_prev_block_hash(sorted_peers.clone(), prev_hash);
    let topo2 = rotated_for_prev_block_hash(reversed_peers.clone(), prev_hash);
    
    // This assertion is expected to fail if rotated_for_prev_block_hash doesn't sort
    assert_eq!(topo1.as_ref(), topo2.as_ref(), "Topologies should be identical regardless of input order");
}
