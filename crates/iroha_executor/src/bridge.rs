//! Minimal bridge helpers for emitting typed bridge receipts.
//! Feature-gated behind `bridge`.

#![cfg(feature = "bridge")]

use iroha_data_model::{
    isi::{InstructionBox, bridge::RecordBridgeReceipt},
    prelude::*,
};

/// Emit a bridge receipt as a typed data event.
pub fn emit_bridge_receipt_log<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    receipt: &iroha_data_model::bridge::BridgeReceipt,
) {
    let isi = RecordBridgeReceipt::new(receipt.clone());
    let boxed = InstructionBox::from(isi);
    executor.visit_instruction(&boxed);
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::prelude::{AccountId, BlockHeader, LaneId};

    use super::*;
    use crate::{
        Execute, Iroha,
        data_model::executor::Result as ExecResult,
        prelude::{Context, Visit},
    };

    struct StubExecutor {
        host: Iroha,
        context: Context,
        verdict: ExecResult,
        captured: Option<iroha_data_model::bridge::BridgeReceipt>,
    }

    impl StubExecutor {
        fn new() -> Self {
            let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let account_id = AccountId::new(key_pair.public_key().clone());
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height > 0"),
                None,
                None,
                None,
                0,
                0,
            );
            Self {
                host: Iroha,
                context: Context {
                    authority: account_id,
                    curr_block: header,
                },
                verdict: Ok(()),
                captured: None,
            }
        }
    }

    impl Execute for StubExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }

        fn context(&self) -> &Context {
            &self.context
        }

        fn context_mut(&mut self) -> &mut Context {
            &mut self.context
        }

        fn verdict(&self) -> &ExecResult {
            &self.verdict
        }

        fn deny(&mut self, reason: iroha_data_model::ValidationFail) {
            self.verdict = Err(reason);
        }
    }

    impl Visit for StubExecutor {
        fn visit_instruction(&mut self, isi: &InstructionBox) {
            let record = isi
                .as_any()
                .downcast_ref::<RecordBridgeReceipt>()
                .expect("expected record bridge receipt instruction");
            self.captured = Some(record.receipt.clone());
        }
    }

    #[test]
    fn emit_bridge_receipt_log_emits_record_instruction() {
        let mut executor = StubExecutor::new();
        let receipt = iroha_data_model::bridge::BridgeReceipt {
            lane: LaneId::new(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash: [0x33; 32],
            amount: 1,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };

        emit_bridge_receipt_log(&mut executor, &receipt);

        let captured = executor.captured.expect("captured receipt");
        assert_eq!(captured, receipt);
    }
}
