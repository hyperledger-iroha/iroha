//! Hash-only inventory of retained suffix obligations after complete native replay.

use super::*;
use iroha_core::state::{StorageReadOnly as _, WorldReadOnly as _};
use iroha_crypto::Hash;
use iroha_data_model::kagemusha::KagemushaWalletLedgerKeyV1;

const INVENTORY_DOMAIN: &[u8] = b"iroha.replay.kagemusha.suffix-rows.v1";

/// Retained suffix row counts, never raw wallet records or an execution authorization.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct KagemushaSuffixRowInventoryV1 {
    /// Exact diagnostic projection grammar.
    pub schema: String,
    /// Authenticated State height after replay reached the durable tip.
    pub height: u64,
    /// Number of all retained KAGEMUSHA ledger rows, including ordinary records.
    pub total_ledger_rows: u64,
    /// Retained Begin records (row kind 16).
    pub begin_rows: u64,
    /// Retained verified chunk records (row kind 17).
    pub stage_rows: u64,
    /// Retained final payout records (row kind 18).
    pub final_rows: u64,
    /// Domain/height-bound native hash of ordered suffix keys and value hashes.
    pub rows_digest: String,
}

pub(super) fn capture(
    state: &State,
    tip_height: u64,
) -> ReportResult<KagemushaSuffixRowInventoryV1, MainError> {
    let view = state.view();
    let height = u64::try_from(view.height())
        .map_err(|_| failure("suffix inventory State height exceeds u64"))?;
    if height != tip_height {
        return Err(failure(
            "suffix inventory State height differs from matched durable tip",
        ));
    }
    project_rows(
        tip_height,
        view.world
            .kagemusha_wallet_ledger()
            .iter()
            .map(|(key, value)| (*key, value.as_slice())),
    )
}

fn project_rows<'a>(
    height: u64,
    rows: impl IntoIterator<Item = (KagemushaWalletLedgerKeyV1, &'a [u8])>,
) -> ReportResult<KagemushaSuffixRowInventoryV1, MainError> {
    let mut result = KagemushaSuffixRowInventoryV1 {
        schema: "kagemusha.suffix-row-inventory.v1".into(),
        height,
        total_ledger_rows: 0,
        begin_rows: 0,
        stage_rows: 0,
        final_rows: 0,
        rows_digest: String::new(),
    };
    let mut previous = None;
    let mut digest = Hash::new_from_chunks(&[INVENTORY_DOMAIN, &height.to_le_bytes()]);
    for (key, value) in rows {
        if previous.is_some_and(|prior| prior >= key) {
            return Err(failure(
                "suffix inventory ledger keys are not strictly ordered",
            ));
        }
        previous = Some(key);
        result.total_ledger_rows = result
            .total_ledger_rows
            .checked_add(1)
            .ok_or_else(|| failure("suffix inventory row count overflow"))?;
        let count = match key.components().0 {
            16 => &mut result.begin_rows,
            17 => &mut result.stage_rows,
            18 => &mut result.final_rows,
            _ => continue,
        };
        *count = count
            .checked_add(1)
            .ok_or_else(|| failure("suffix inventory obligation count overflow"))?;
        let key_bytes = norito::encode_canonical(&key)
            .map_err(|error| failure(format!("suffix inventory key encoding: {error}")))?;
        let key_hash = Hash::new(key_bytes);
        let value_hash = Hash::new(value);
        digest = Hash::new_from_chunks(&[
            INVENTORY_DOMAIN,
            digest.as_ref(),
            key_hash.as_ref(),
            value_hash.as_ref(),
        ]);
    }
    result.rows_digest = digest.to_string();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: u8, owner: u8) -> KagemushaWalletLedgerKeyV1 {
        KagemushaWalletLedgerKeyV1::from_parts(kind, [1; 32], [owner; 32], [0; 32])
    }

    #[test]
    fn inventory_distinguishes_retained_obligations_and_binds_their_originals() {
        let rows = [
            (key(2, 2), b"ordinary wallet".as_slice()),
            (key(16, 3), b"begin".as_slice()),
            (key(17, 3), b"first chunk".as_slice()),
            (key(17, 4), b"second chunk".as_slice()),
            (key(18, 3), b"final".as_slice()),
        ];
        let before = project_rows(42, rows).unwrap();
        assert_eq!(before.total_ledger_rows, 5);
        assert_eq!(
            (before.begin_rows, before.stage_rows, before.final_rows),
            (1, 2, 1)
        );
        let mut changed = rows;
        changed[2].1 = b"different chunk";
        assert_ne!(
            before.rows_digest,
            project_rows(42, changed).unwrap().rows_digest
        );
        assert_ne!(
            before.rows_digest,
            project_rows(43, rows).unwrap().rows_digest
        );
        changed = rows;
        changed[0].1 = b"different ordinary wallet";
        assert_eq!(
            before.rows_digest,
            project_rows(42, changed).unwrap().rows_digest
        );
    }

    #[test]
    fn inventory_requires_exact_order_and_explicit_empty_counts() {
        let ordinary = [(key(2, 2), b"ordinary wallet".as_slice())];
        let report = project_rows(42, ordinary).unwrap();
        assert_eq!(report.total_ledger_rows, 1);
        assert_eq!(
            (report.begin_rows, report.stage_rows, report.final_rows),
            (0, 0, 0)
        );
        let one = (key(16, 3), b"begin".as_slice());
        assert!(project_rows(42, [one, one]).is_err());
        assert!(project_rows(42, [one, ordinary[0]]).is_err());
    }
}
