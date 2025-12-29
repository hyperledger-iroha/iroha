use super::*;

isi! {
    /// Claim a promotional reward for an active Twitter follow binding.
    pub struct ClaimTwitterFollowReward {
        /// Binding hash (keyed) proven by the soracles feed.
        pub binding_hash: crate::oracle::KeyedHash,
    }
}

isi! {
    /// Send a reward to a Twitter handle; funds are escrowed until the binding appears.
    pub struct SendToTwitter {
        /// Binding hash (keyed) for the target handle.
        pub binding_hash: crate::oracle::KeyedHash,
        /// Amount to escrow or deliver immediately.
        pub amount: iroha_primitives::numeric::Numeric,
    }
}

isi! {
    /// Cancel an existing escrow created by [`SendToTwitter`].
    pub struct CancelTwitterEscrow {
        /// Binding hash (keyed) for the escrow.
        pub binding_hash: crate::oracle::KeyedHash,
    }
}

impl crate::seal::Instruction for ClaimTwitterFollowReward {}
impl crate::seal::Instruction for SendToTwitter {}
impl crate::seal::Instruction for CancelTwitterEscrow {}
