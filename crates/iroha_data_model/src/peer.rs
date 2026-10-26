//! Ledger registration traits for the canonical foundational peer endpoint.

use crate::{Identifiable, Registered};
use iroha_model_base::peer::{Peer, PeerId};

impl Identifiable for Peer {
    type Id = PeerId;

    fn id(&self) -> &Self::Id {
        Peer::id(self)
    }
}

impl Registered for Peer {
    type With = PeerId;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_peer_keeps_ledger_registration_identity() {
        fn registration_id<P: Registered<With = PeerId>>() {}
        registration_id::<Peer>();
        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let peer: Peer = format!("{key}@127.0.0.1:1337").parse().unwrap();
        assert_eq!(Identifiable::id(&peer), peer.id());
    }
}
