//! Asset movement index: fold rules, KAGEMUSHA record binding, paging, freshness and bounds.
//!
//! Include as a child of `routing::asset_movements`. Chain fixtures submit signed transactions
//! through production execution. KAGEMUSHA rows are checked against exact, validated ledger
//! rows, because a genuine Load or Unload needs native proofs these tests do not produce.

use super::*;
use crate::{collections::specs, routing::collection_sources::collection_limits};
use iroha_core::{
    kagemusha_wallet_v1::{LoadCharge, LoadCommand},
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetId},
    domain::Domain,
    isi::{
        Burn, Mint, Register, Transfer, TransferAssetBatch, TransferAssetBatchEntry,
        kagemusha_wallet::KagemushaWalletLoadReceiptV1,
    },
    kagemusha::{
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaWalletAssetScopeV1,
        KagemushaWalletChargeKindV1, KagemushaWalletChargeQuoteBodyV1,
        KagemushaWalletChargeQuoteV1, KagemushaWalletSchemeV1, kagemusha_wallet_account_digest_v1,
        kagemusha_wallet_provider_contract_v1,
    },
    nexus::AxtAssetIncarnationV1,
    transaction::{FeePaymentIntent, TransactionBuilder, signed::TransactionResult},
};
use iroha_executor_data_model::isi::multisig::{
    DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigProposalId, MultisigProposalNonce,
    MultisigPropose, MultisigRegister, MultisigSpec,
};
use iroha_model_base::domain::DomainId;
use iroha_torii_shared::list_query::field;
use std::num::{NonZeroU16, NonZeroU64};

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic asset movement fixture key")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}

fn prepared<'q>(query: &'q ListQuery, scope: &str) -> collections::Prepared<'q> {
    collections::prepare(&specs::ASSET_MOVEMENTS, scope, query, &collection_limits())
        .expect("valid asset movement query")
}

fn page(
    index: &AssetMovementIndex,
    definition: &str,
    query: &ListQuery,
    visibility: &DataspaceReadVisibility,
) -> Result<RowPage, crate::Error> {
    index.page(&prepared(query, definition), definition, visibility)
}

/// Every row of `definition` that `query` selects, following every cursor.
fn walk(
    index: &AssetMovementIndex,
    definition: &str,
    base: &ListQuery,
    visibility: &DataspaceReadVisibility,
) -> Vec<Map> {
    let mut query = base.clone();
    let mut rows = Vec::new();
    let mut cursors = BTreeSet::new();
    for _ in 0..10_000 {
        let page = page(index, definition, &query, visibility).expect("bounded movement page");
        if let Some(limit) = query.limit {
            assert!(page.items.len() <= limit as usize);
        }
        rows.extend(page.items);
        let Some(cursor) = page.next_cursor else {
            return rows;
        };
        assert!(
            cursors.insert(cursor.clone()),
            "every continuation advances"
        );
        query = base.clone().cursor(cursor);
    }
    panic!("the fixture exceeded the bounded page walk");
}

fn movement(
    definition: &str,
    kind: Kind,
    from: Option<&str>,
    to: Option<&str>,
    amount: &str,
) -> Movement {
    Movement {
        definition: definition.to_owned(),
        kind,
        from: from.map(str::to_owned),
        to: to.map(str::to_owned),
        amount: amount.to_owned(),
    }
}

fn folded_transaction(
    block_index: u64,
    route: Option<BTreeSet<DataSpaceId>>,
    movements: Vec<Movement>,
) -> FoldedTransaction {
    FoldedTransaction {
        tx_hash: format!("{block_index:064x}"),
        authority: "signer".to_owned(),
        timestamp_ms: 1_000 + block_index,
        block_index,
        result_ok: true,
        route,
        movements,
    }
}

fn block(height: usize, transactions: Vec<FoldedTransaction>) -> FoldedBlock {
    FoldedBlock {
        height,
        hash: HashOf::from_untyped_unchecked(Hash::prehashed([height as u8; 32])),
        transactions: Some(transactions),
    }
}

/// `count` transfers of `coin` in each of `heights`, one transaction per movement.
fn transfer_blocks(
    index: &mut AssetMovementIndex,
    heights: std::ops::RangeInclusive<usize>,
    count: u64,
) {
    for height in heights {
        index.append(block(
            height,
            (0..count)
                .map(|position| {
                    folded_transaction(
                        position,
                        None,
                        vec![movement("coin", Kind::Transfer, Some("a"), Some("b"), "1")],
                    )
                })
                .collect(),
        ));
    }
}

fn coordinates(rows: &[Map]) -> Vec<(u64, u64, u64)> {
    rows.iter()
        .map(|row| {
            (
                row["block_height"].as_u64().unwrap(),
                row["block_index"].as_u64().unwrap(),
                row["movement_index"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn leg(
    direction: &str,
    counterparty: Option<&str>,
    amount: Option<&str>,
) -> AccountHistoryProjection {
    AccountHistoryProjection {
        direction: direction.to_owned(),
        account_id: "alice".to_owned(),
        counterparty_account_id: counterparty.map(str::to_owned),
        asset_definition_id: Some("coin".to_owned()),
        amount: amount.map(str::to_owned),
        ..AccountHistoryProjection::default()
    }
}

#[test]
fn account_legs_become_one_row_per_movement() {
    let legs = [
        // A transfer projects its outgoing leg and the mirrored incoming leg.
        leg("outgoing", Some("bob"), Some("5")),
        leg("incoming", Some("carol"), Some("5")),
        leg("incoming", None, Some("7")),
        leg("outgoing", None, Some("2")),
        leg("self", None, Some("1")),
        leg("affected", None, None),
        // A domain or NFT transfer carries no amount.
        leg("outgoing", Some("bob"), None),
    ];
    let movements: Vec<_> = leg_movements(&legs).collect();
    assert_eq!(
        movements,
        [
            movement("coin", Kind::Transfer, Some("alice"), Some("bob"), "5"),
            movement("coin", Kind::Mint, None, Some("alice"), "7"),
            movement("coin", Kind::Burn, Some("alice"), None, "2"),
            movement("coin", Kind::Transfer, Some("alice"), Some("alice"), "1"),
        ]
    );
}

#[test]
fn spec_declares_the_row_fields_and_history_coordinates() {
    let spec = &specs::ASSET_MOVEMENTS;
    assert_eq!(spec.positioned, 3);
    assert_eq!(
        spec.identity,
        &["block_height", "block_index", "movement_index"]
    );
    for name in ["from_account_id", "to_account_id", "authority"] {
        assert!(
            spec.field(name).unwrap().account,
            "{name} canonicalizes account literals"
        );
    }
    let result_ok = spec.field("result_ok").unwrap();
    assert_eq!(result_ok.ty, specs::FieldType::Bool);
    let mut index = AssetMovementIndex::new(MAX_ROWS);
    transfer_blocks(&mut index, 2..=2, 1);
    let row = &walk(
        &index,
        "coin",
        &ListQuery::new(),
        &DataspaceReadVisibility::all_for_tests(),
    )[0];
    let mut fields: Vec<&str> = row.keys().map(String::as_str).collect();
    fields.sort_unstable();
    let mut declared: Vec<&str> = spec.fields.iter().map(|field| field.name).collect();
    declared.sort_unstable();
    assert_eq!(
        fields, declared,
        "every row field is declared and every declared field is set"
    );
}

#[test]
fn pages_run_newest_first_and_cursors_keep_each_movement_once() {
    let mut index = AssetMovementIndex::new(MAX_ROWS);
    for height in 2..=6 {
        index.append(block(
            height,
            vec![
                folded_transaction(
                    0,
                    None,
                    vec![
                        movement("coin", Kind::Mint, None, Some("a"), "3"),
                        movement("other", Kind::Mint, None, Some("a"), "9"),
                        movement("coin", Kind::Transfer, Some("a"), Some("b"), "1"),
                    ],
                ),
                folded_transaction(
                    1,
                    None,
                    vec![movement("coin", Kind::Burn, Some("b"), None, "1")],
                ),
            ],
        ));
    }
    let all = DataspaceReadVisibility::all_for_tests();
    let everything = walk(&index, "coin", &ListQuery::new(), &all);
    assert_eq!(everything.len(), 15);
    let positions = coordinates(&everything);
    let mut sorted = positions.clone();
    sorted.sort_unstable_by(|left, right| right.cmp(left));
    assert_eq!(
        positions, sorted,
        "newest first, then block index, then movement index"
    );
    assert_eq!(positions[0], (6, 1, 0));
    // movement_index counts every movement of the transaction, across definitions.
    assert!(positions.contains(&(6, 0, 2)) && !positions.contains(&(6, 0, 1)));
    assert_eq!(
        everything[0]["id"].as_str(),
        Some(format!("{:064x}:0", 1).as_str())
    );
    for limit in [1, 2, 4] {
        assert_eq!(
            walk(&index, "coin", &ListQuery::new().limit(limit), &all),
            everything,
            "limit {limit}"
        );
    }
    let newer = walk(
        &index,
        "coin",
        &ListQuery::new().filter(field("block_height").gt(4_u64)),
        &all,
    );
    assert_eq!(
        newer,
        everything[..6].to_vec(),
        "only the rows above the height"
    );
    let window = walk(
        &index,
        "coin",
        &ListQuery::new().limit(2).filter(
            field("block_height")
                .gte(3_u64)
                .and(field("block_height").lte(4_u64)),
        ),
        &all,
    );
    assert_eq!(coordinates(&window).first(), Some(&(4, 1, 0)));
    assert_eq!(coordinates(&window).last(), Some(&(3, 0, 0)));
    assert_eq!(window.len(), 6);
    let kinds = walk(
        &index,
        "coin",
        &ListQuery::new().filter(
            field("kind")
                .eq("BURN")
                .and(field("from_account_id").eq("b")),
        ),
        &all,
    );
    assert_eq!(kinds.len(), 5);
    assert!(kinds.iter().all(|row| row["to_account_id"].is_null()));
    assert!(walk(&index, "missing", &ListQuery::new(), &all).is_empty());
}

#[test]
fn hidden_rows_spend_the_budget_and_continue_at_public_block_boundaries() {
    let mut index = AssetMovementIndex::new(MAX_ROWS);
    // Each block holds more hidden rows than half a page budget, then one visible row on top.
    let per_block = (PAGE_EXAMINED_ROWS / 2 + 1) as u64;
    let visible: BTreeSet<_> = [DataSpaceId::new(7)].into();
    for height in 2..=4 {
        let mut transactions: Vec<_> = (0..per_block)
            .map(|position| {
                folded_transaction(
                    position,
                    None,
                    vec![movement("coin", Kind::Transfer, Some("a"), Some("b"), "1")],
                )
            })
            .collect();
        transactions.push(folded_transaction(
            per_block,
            Some(visible.clone()),
            vec![movement("coin", Kind::Transfer, Some("a"), Some("c"), "2")],
        ));
        index.append(block(height, transactions));
    }
    let reader = DataspaceReadVisibility::new(visible, false);
    let mut query = ListQuery::new();
    let mut rows = Vec::new();
    let mut boundaries = 0;
    loop {
        let current = prepared(&query, "coin");
        let page = index.page(&current, "coin", &reader).unwrap();
        rows.extend(page.items);
        let Some(cursor) = page.next_cursor else {
            break;
        };
        let next = ListQuery::new().cursor(cursor);
        let resumed = prepared(&next, "coin");
        if resumed.resume_before_block().is_some() {
            boundaries += 1;
            assert!(
                resumed.resume_subrow_position().is_none(),
                "no hidden coordinate"
            );
        }
        query = next;
    }
    assert!(
        boundaries > 0,
        "the budget ended at least one page at a block boundary"
    );
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|row| row["to_account_id"].as_str() == Some("c"))
    );
    assert!(
        walk(
            &index,
            "coin",
            &ListQuery::new(),
            &DataspaceReadVisibility::default()
        )
        .is_empty(),
        "an entrypoint without visible route legs stays with global readers"
    );
}

#[test]
fn the_row_bound_drops_the_oldest_blocks_and_reads_below_the_floor_fail() {
    let mut index = AssetMovementIndex::new(10);
    let all = DataspaceReadVisibility::all_for_tests();
    for height in 2..=7 {
        transfer_blocks(&mut index, height..=height, 3);
        index.enforce_capacity();
        assert!(index.rows <= 10);
    }
    let floor = index.not_retained_below;
    assert!(floor > 2, "old blocks were dropped");
    let retained = walk(
        &index,
        "coin",
        &ListQuery::new().filter(field("block_height").gte(floor)),
        &all,
    );
    assert_eq!(retained.len(), index.rows);
    assert!(
        coordinates(&retained)
            .iter()
            .all(|(height, _, _)| *height >= floor)
    );
    // A read that reaches below the floor returns what is retained, then fails explicitly.
    let first = page(&index, "coin", &ListQuery::new(), &all).unwrap();
    assert_eq!(first.items.len(), index.rows);
    let next = ListQuery::new().cursor(first.next_cursor.expect("the history continues"));
    let error = page(&index, "coin", &next, &all).unwrap_err();
    let crate::Error::CollectionQuery(error) = error else {
        panic!("not retained is a collection error: {error:?}")
    };
    assert_eq!(error.code, NOT_RETAINED_CODE);
    assert_eq!(error.expected.as_deref(), Some(floor.to_string().as_str()));
    let narrow = ListQuery::new().filter(field("block_height").lte(floor - 1));
    assert!(page(&index, "coin", &narrow, &all).is_err());
    // A block this node cannot project stops everything at or below it being retained.
    index.append(FoldedBlock {
        height: 8,
        hash: HashOf::from_untyped_unchecked(Hash::prehashed([8; 32])),
        transactions: None,
    });
    assert_eq!(index.not_retained_below, 9);
    assert_eq!(index.rows, 0);
    assert_eq!(index.height, 8);
}

/// A scheme registration and its exact ledger row.
struct LedgerFixture {
    network: NetworkId,
    scheme: [u8; 32],
    asset: [u8; 32],
    definition: AssetDefinitionId,
    reserve: AccountId,
    rows: BTreeMap<KagemushaWalletLedgerKeyV1, Vec<u8>>,
}

impl LedgerRecords for BTreeMap<KagemushaWalletLedgerKeyV1, Vec<u8>> {
    fn record(&self, key: &KagemushaWalletLedgerKeyV1) -> Option<&[u8]> {
        self.get(key).map(Vec::as_slice)
    }
}

/// Uncompressed SEC1 P-256 generator, a valid scheme root key.
const P256_GENERATOR: &str = "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";
const WALLET: [u8; 32] = [0x31; 32];
const REQUEST: [u8; 32] = [0x32; 32];

impl LedgerFixture {
    fn new() -> Self {
        let network = super::super::routing_test_network_id(0x5d);
        let scheme = KagemushaWalletSchemeV1 {
            version: 1,
            network_id: *network.as_bytes(),
            scheme_root_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
                &hex::decode(P256_GENERATOR).unwrap(),
            )
            .unwrap(),
            relation_id: [0x5a; 32],
            provider_contract: kagemusha_wallet_provider_contract_v1(),
        };
        let definition = AssetDefinitionId::from_uuid_bytes([
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x47, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ])
        .unwrap();
        let incarnation =
            AxtAssetIncarnationV1::try_from_bytes(*Hash::prehashed([0x66; 32]).as_ref()).unwrap();
        let scope = KagemushaWalletAssetScopeV1::new(definition.clone(), &incarnation, 2).unwrap();
        let reserve = account(0x95);
        let registration = Registration {
            scheme,
            asset: scope.clone(),
            reserve: reserve.clone(),
            balance_scope: AssetBalanceScope::Global,
        };
        let mut fixture = Self {
            network,
            scheme: scheme.scheme_id(),
            asset: scope.asset_digest(),
            definition,
            reserve,
            rows: BTreeMap::new(),
        };
        fixture.insert(
            KagemushaWalletLedgerKeyV1::from_parts(
                REGISTRATION_ROW,
                fixture.scheme,
                fixture.asset,
                [0; 32],
            ),
            &registration,
        );
        fixture
    }

    fn insert<T: norito::NoritoSerialize>(&mut self, key: KagemushaWalletLedgerKeyV1, value: &T) {
        let bytes = norito::to_bytes(value).unwrap();
        validate_row(&key, &bytes).expect("the fixture writes rows the ledger accepts");
        self.rows.insert(key, bytes);
    }

    /// The Load issuance the ledger records for `transaction` at `height`.
    fn issuance(
        &self,
        payer: &AccountId,
        amount: u128,
        charge: Option<(&AccountId, u128)>,
        (transaction, height): ([u8; 32], u64),
    ) -> Issuance {
        let charge = charge.map(|(beneficiary, online_charge)| {
            let mut signature = [0_u8; 64];
            signature[31] = 1;
            signature[63] = 1;
            LoadCharge {
                quote: KagemushaWalletChargeQuoteV1 {
                    body: KagemushaWalletChargeQuoteBodyV1 {
                        version: 1,
                        scheme_id: self.scheme,
                        asset_digest: self.asset,
                        wallet_id: WALLET,
                        kind: KagemushaWalletChargeKindV1::Load,
                        ordinal: 0,
                        net_amount: amount,
                        online_charge,
                        beneficiary_account_digest: kagemusha_wallet_account_digest_v1(beneficiary)
                            .unwrap(),
                        issued_at_ms: 1,
                        signer_certificate: [0x01; 32],
                    },
                    signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature).unwrap(),
                },
                beneficiary: beneficiary.clone(),
            }
        });
        let (online_charge, charge_quote) = charge.as_ref().map_or((0, [0; 32]), |charge| {
            (
                charge.quote.body.online_charge,
                charge.quote.charge_quote_digest(),
            )
        });
        Issuance {
            command: LoadCommand {
                scheme: self.scheme,
                wallet: WALLET,
                asset: self.asset,
                ordinal: 0,
                request_id: REQUEST,
                amount,
                charge,
            },
            payer: payer.clone(),
            body: KagemushaWalletLoadReceiptV1 {
                version: 1,
                scheme_id: self.scheme,
                asset_digest: self.asset,
                wallet_id: WALLET,
                request_id: REQUEST,
                ordinal: 0,
                amount,
                online_charge,
                charge_quote,
                transaction_hash: transaction,
                block_height: height,
                payer_account_digest: kagemusha_wallet_account_digest_v1(payer).unwrap(),
            },
        }
    }

    fn load(&self, payer: &AccountId) -> TransactionEntrypoint {
        let instruction = KagemushaWalletLedgerV1::new(
            self.scheme,
            KagemushaWalletLedgerActionV1::IssueLoad {
                wallet: WALLET,
                asset: self.asset,
                ordinal: 0,
                request_id: REQUEST,
                amount: 103_225,
                charge: None,
            },
        );
        let signed = TransactionBuilder::new(
            self.network,
            payer.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(instruction)])
        .sign(key(0x41).private_key());
        TransactionEntrypoint::External(signed)
    }

    fn unload(&self, nullifier: u8, payees: Vec<(AccountId, u128)>) -> PayoutClaim {
        PayoutClaim {
            kind: Kind::OfflineUnload,
            scheme: self.scheme,
            asset: self.asset,
            key: KagemushaWalletPayoutKeyV1::Unload([nullifier; 32]),
            source: [0x44; 32],
            amount: payees.iter().map(|(_, value)| value).sum(),
            payees,
        }
    }

    fn pay(&mut self, claim: &PayoutClaim, transaction: [u8; 32]) {
        let record = KagemushaWalletPayoutRecordV1 {
            key: claim.key,
            source: claim.source,
            amount: claim.amount,
            transaction,
        };
        self.insert(claim.key.ledger_key(claim.scheme), &record);
    }
}

fn offline_rows(movements: &[Movement]) -> Vec<(Kind, Option<String>, Option<String>, String)> {
    movements
        .iter()
        .map(|movement| {
            (
                movement.kind,
                movement.from.clone(),
                movement.to.clone(),
                movement.amount.clone(),
            )
        })
        .collect()
}

#[test]
fn loads_come_from_the_issuance_that_names_their_transaction() {
    let mut ledger = LedgerFixture::new();
    let payer = account(0x41);
    let entrypoint = ledger.load(&payer);
    let hash: [u8; 32] = *entrypoint.hash().as_ref();
    let result = TransactionResult::new(Ok(Vec::new()));
    let transaction = BorrowedNetworkTransaction {
        entrypoint: &entrypoint,
        entrypoint_hash: entrypoint.hash(),
        result: &result,
    };
    // The issuance names another transaction: a later reuse of the request id moved nothing.
    let earlier = ledger.issuance(&payer, 103_225, None, ([0x77; 32], 7));
    ledger.insert(
        KagemushaWalletLedgerKeyV1::from_parts(ISSUANCE_ROW, ledger.scheme, WALLET, REQUEST),
        &earlier,
    );
    let mut out = Vec::new();
    offline_movements(&transaction, 7, &ledger.rows, &mut out).unwrap();
    assert!(out.is_empty());
    let original = ledger.issuance(&payer, 103_225, None, (hash, 7));
    ledger.insert(
        KagemushaWalletLedgerKeyV1::from_parts(ISSUANCE_ROW, ledger.scheme, WALLET, REQUEST),
        &original,
    );
    offline_movements(&transaction, 8, &ledger.rows, &mut out).unwrap();
    assert!(out.is_empty(), "the issuance names its own block height");
    offline_movements(&transaction, 7, &ledger.rows, &mut out).unwrap();
    assert_eq!(
        offline_rows(&out),
        [(
            Kind::OfflineLoad,
            Some(payer.to_string()),
            Some(ledger.reserve.to_string()),
            "1032.25".to_owned()
        )]
    );
    assert_eq!(out[0].definition, ledger.definition.to_string());
    // A failed transaction projects no KAGEMUSHA movement, whatever the ledger holds.
    let failed = TransactionResult::new(Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("fixture".to_owned()),
        ),
    ));
    let rejected = BorrowedNetworkTransaction {
        entrypoint: &entrypoint,
        entrypoint_hash: entrypoint.hash(),
        result: &failed,
    };
    let app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let view = app.state.view();
    let mut budget = MultisigHistoryBudget::new(64, 1 << 20);
    assert!(
        project_transaction(
            &rejected,
            7,
            view.world(),
            &ledger.rows,
            &ledger.network,
            &mut budget
        )
        .unwrap()
        .is_none()
    );
    let projected = project_transaction(
        &transaction,
        7,
        view.world(),
        &ledger.rows,
        &ledger.network,
        &mut budget,
    )
    .unwrap()
    .expect("a successful Load moves value");
    assert_eq!(
        projected.0,
        payer.to_string(),
        "the authority is the signer"
    );
    assert_eq!(projected.1, out);
}

#[test]
fn a_charged_load_pays_its_online_charge_to_the_quoted_beneficiary() {
    let mut ledger = LedgerFixture::new();
    let payer = account(0x41);
    let beneficiary = account(0x52);
    let issuance = ledger.issuance(&payer, 500, Some((&beneficiary, 25)), ([0x78; 32], 9));
    ledger.insert(
        KagemushaWalletLedgerKeyV1::from_parts(ISSUANCE_ROW, ledger.scheme, WALLET, REQUEST),
        &issuance,
    );
    let registration = registration(&ledger.rows, ledger.scheme, ledger.asset).unwrap();
    let mut out = Vec::new();
    load_movements(&issuance, &registration, &mut out).unwrap();
    assert_eq!(
        offline_rows(&out),
        [
            (
                Kind::OfflineLoad,
                Some(payer.to_string()),
                Some(ledger.reserve.to_string()),
                "5".to_owned()
            ),
            (
                Kind::OfflineLoad,
                Some(payer.to_string()),
                Some(beneficiary.to_string()),
                "0.25".to_owned()
            ),
        ]
    );
}

#[test]
fn unloads_and_fees_pay_once_from_the_transaction_their_record_names() {
    let mut ledger = LedgerFixture::new();
    let holder = account(0x61);
    let beneficiary = account(0x62);
    let unload = ledger.unload(
        0x51,
        vec![(holder.clone(), 1_950), (beneficiary.clone(), 50)],
    );
    let paying = [0x81; 32];
    let mut out = Vec::new();
    let mut credited = BTreeSet::new();
    // No record yet: nothing was paid.
    unload
        .movements(paying, &ledger.rows, &mut credited, &mut out)
        .unwrap();
    assert!(out.is_empty());
    ledger.pay(&unload, paying);
    // A retry in a later transaction finds the original payout and moves nothing.
    unload
        .movements([0x82; 32], &ledger.rows, &mut BTreeSet::new(), &mut out)
        .unwrap();
    assert!(out.is_empty());
    let mut credited = BTreeSet::new();
    unload
        .movements(paying, &ledger.rows, &mut credited, &mut out)
        .unwrap();
    // The same claim twice in one transaction is paid once.
    unload
        .movements(paying, &ledger.rows, &mut credited, &mut out)
        .unwrap();
    assert_eq!(
        offline_rows(&out),
        [
            (
                Kind::OfflineUnload,
                Some(ledger.reserve.to_string()),
                Some(holder.to_string()),
                "19.5".to_owned()
            ),
            (
                Kind::OfflineUnload,
                Some(ledger.reserve.to_string()),
                Some(beneficiary.to_string()),
                "0.5".to_owned()
            ),
        ]
    );
    // A whole-charge Unload pays only the charge; a record for other terms is refused.
    let charge_only = ledger.unload(0x52, vec![(holder.clone(), 0), (beneficiary.clone(), 7)]);
    ledger.pay(&charge_only, paying);
    let mut rows = Vec::new();
    charge_only
        .movements(paying, &ledger.rows, &mut BTreeSet::new(), &mut rows)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].to.as_deref(),
        Some(beneficiary.to_string().as_str())
    );
    let mut altered = ledger.unload(0x52, vec![(holder.clone(), 8)]);
    altered.source = [0x45; 32];
    assert!(
        altered
            .movements(paying, &ledger.rows, &mut BTreeSet::new(), &mut Vec::new())
            .is_err()
    );
    let fee = PayoutClaim {
        kind: Kind::OfflineFee,
        scheme: ledger.scheme,
        asset: ledger.asset,
        key: KagemushaWalletPayoutKeyV1::Fee([0x53; 32]),
        source: [0x46; 32],
        amount: 3,
        payees: vec![(beneficiary.clone(), 3)],
    };
    ledger.pay(&fee, paying);
    let mut fees = Vec::new();
    fee.movements(paying, &ledger.rows, &mut BTreeSet::new(), &mut fees)
        .unwrap();
    assert_eq!(
        offline_rows(&fees),
        [(
            Kind::OfflineFee,
            Some(ledger.reserve.to_string()),
            Some(beneficiary.to_string()),
            "0.03".to_owned()
        )]
    );
    // A malformed claim frame in a successful transaction is an inconsistent block.
    assert!(PayoutClaim::unload(&[0, 1, 2], &ledger.scheme).is_err());
    assert!(PayoutClaim::fee(&[0, 1, 2], &ledger.scheme).is_err());
}

#[test]
fn offline_rows_render_the_contract_json() {
    let ledger = LedgerFixture::new();
    let payer = account(0x41);
    let holder = account(0x61);
    let beneficiary = account(0x62);
    let definition = ledger.definition.to_string();
    let (payer, holder, beneficiary, reserve) = (
        payer.to_string(),
        holder.to_string(),
        beneficiary.to_string(),
        ledger.reserve.to_string(),
    );
    let mut index = AssetMovementIndex::new(MAX_ROWS);
    let mut load = folded_transaction(
        0,
        None,
        vec![movement(
            &definition,
            Kind::OfflineLoad,
            Some(payer.as_str()),
            Some(reserve.as_str()),
            "1032.25",
        )],
    );
    load.authority.clone_from(&payer);
    let mut unload = folded_transaction(
        1,
        None,
        vec![
            movement(
                &definition,
                Kind::OfflineUnload,
                Some(reserve.as_str()),
                Some(holder.as_str()),
                "19.5",
            ),
            movement(
                &definition,
                Kind::OfflineUnload,
                Some(reserve.as_str()),
                Some(beneficiary.as_str()),
                "0.5",
            ),
        ],
    );
    unload.authority.clone_from(&holder);
    index.append(block(12, vec![load, unload]));
    let rows = walk(
        &index,
        &definition,
        &ListQuery::new(),
        &DataspaceReadVisibility::all_for_tests(),
    );
    let load = rows
        .iter()
        .find(|row| row["kind"].as_str() == Some("OFFLINE_LOAD"))
        .unwrap();
    let unloads: Vec<&Map> = rows
        .iter()
        .filter(|row| row["kind"].as_str() == Some("OFFLINE_UNLOAD"))
        .collect();
    println!(
        "OFFLINE_LOAD row: {}",
        norito::json::to_json(&Value::Object(load.clone())).unwrap()
    );
    for unload in &unloads {
        println!(
            "OFFLINE_UNLOAD row: {}",
            norito::json::to_json(&Value::Object((*unload).clone())).unwrap()
        );
    }
    assert_eq!(load["from_account_id"].as_str(), Some(payer.as_str()));
    assert_eq!(load["amount"].as_str(), Some("1032.25"));
    assert_eq!(load["result_ok"].as_bool(), Some(true));
    assert_eq!(
        load["asset_definition_id"].as_str(),
        Some(definition.as_str())
    );
    assert_eq!(
        unloads
            .iter()
            .map(|row| (
                row["from_account_id"].as_str().unwrap(),
                row["to_account_id"].as_str().unwrap(),
                row["amount"].as_str().unwrap(),
                row["movement_index"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        [
            (reserve.as_str(), beneficiary.as_str(), "0.5", 1),
            (reserve.as_str(), holder.as_str(), "19.5", 0),
        ],
        "the online charge follows the account payout; pages run newest first"
    );
    assert_eq!(rows.len(), 3);
}

/// A real certified chain with ordinary, failed, batch and multisig-executed movements.
struct ChainFixture {
    chain: CertifiedTestChain,
    genesis: KeyPair,
    sender: KeyPair,
    signer: KeyPair,
    recipient: AccountId,
    multisig: AccountId,
    definition: AssetDefinitionId,
}

struct Submission {
    tx_hash: String,
    height: u64,
    succeeded: bool,
}

impl ChainFixture {
    fn new() -> Self {
        let genesis_time = 1_000;
        let sender = key(0xB1);
        let signer = key(0xB2);
        let recipient = account(0xB3);
        let domain = DomainId::try_new("asset_movements", "universal").unwrap();
        let definition =
            AssetDefinitionId::derive_from_components(domain.clone(), "kina".parse().unwrap());
        let mut config = TestChainConfig::new(World::new(), genesis_time);
        let genesis = config.genesis_key.clone();
        config.genesis_instructions = vec![
            Register::domain(Domain::new(domain.clone())).into(),
            Register::account(Account::new(AccountId::new(sender.public_key().clone()))).into(),
            Register::account(Account::new(AccountId::new(signer.public_key().clone()))).into(),
            Register::account(Account::new(recipient.clone())).into(),
            Register::account(Account::new(account(0xB5))).into(),
            Register::asset_definition(AssetDefinition::numeric(
                definition.clone(),
                "Asset movement coin",
                AssetBalancePolicy::Global,
                Some(domain.clone()),
            ))
            .into(),
        ];
        let mut chain = CertifiedTestChain::start(config).expect("certified movement genesis");
        let spec = MultisigSpec::new(
            BTreeMap::from([
                (AccountId::new(signer.public_key().clone()), 1),
                (account(0xB5), 1),
            ]),
            NonZeroU16::new(1).unwrap(),
            NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        );
        let register = chain.sign(
            &genesis,
            [MultisigRegister::with_account(account(0xB4), domain, spec).into()],
            2_000,
        );
        assert_eq!(chain.commit_at(2_000, vec![register]), [true]);
        let multisig = chain
            .state()
            .view()
            .world()
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .expect("native registered multisig account")
            .id()
            .clone();
        Self {
            chain,
            genesis,
            sender,
            signer,
            recipient,
            multisig,
            definition,
        }
    }

    fn holding(&self, account: &AccountId) -> AssetId {
        AssetId::new(self.definition.clone(), account.clone())
    }

    fn sender_id(&self) -> AccountId {
        AccountId::new(self.sender.public_key().clone())
    }

    fn submit(&mut self, signer: &KeyPair, instructions: Vec<InstructionBox>) -> Submission {
        let current = self.chain.committed(self.chain.height());
        let next_time =
            u64::try_from(current.block().header().creation_time().as_millis()).unwrap() + 1_000;
        let signed = self.chain.sign(signer, instructions, next_time);
        let tx_hash = signed.hash_as_entrypoint().to_string();
        let outcomes = self.chain.commit_at(next_time, vec![signed]);
        assert_eq!(outcomes.len(), 1);
        Submission {
            tx_hash,
            height: self.chain.height(),
            succeeded: outcomes[0],
        }
    }

    fn app(&self) -> crate::SharedAppState {
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let unique = Arc::get_mut(&mut app).expect("unique fixture app");
        unique.state = Arc::clone(self.chain.state());
        unique.kura = Arc::clone(self.chain.kura());
        app
    }
}

fn leaked_feed(capacity: usize) -> &'static MovementFeed {
    Box::leak(Box::new(MovementFeed::new(capacity)))
}

fn row_of<'a>(rows: &'a [Map], submission: &Submission) -> Vec<&'a Map> {
    rows.iter()
        .filter(|row| row["tx_hash"].as_str() == Some(submission.tx_hash.as_str()))
        .collect()
}

#[test]
fn committed_blocks_fold_every_movement_once_with_its_kind_and_outcome() {
    let mut fixture = ChainFixture::new();
    let sender = fixture.sender_id();
    let genesis = fixture.genesis.clone();
    let mint = fixture.submit(
        &genesis,
        vec![
            Mint::asset_quantity(100_u32, fixture.holding(&sender)).into(),
            Mint::asset_quantity(10_u32, fixture.holding(&fixture.multisig.clone())).into(),
        ],
    );
    let sender_key = fixture.sender.clone();
    let transfer = fixture.submit(
        &sender_key,
        vec![
            Transfer::asset_quantity(fixture.holding(&sender), 10_u32, fixture.recipient.clone())
                .into(),
        ],
    );
    let failed = fixture.submit(
        &sender_key,
        vec![
            Transfer::asset_quantity(
                fixture.holding(&sender),
                1_000_u32,
                fixture.recipient.clone(),
            )
            .into(),
        ],
    );
    let burn = fixture.submit(
        &sender_key,
        vec![Burn::asset_quantity(5_u32, fixture.holding(&sender)).into()],
    );
    let batch = fixture.submit(
        &sender_key,
        vec![
            TransferAssetBatch::new(vec![TransferAssetBatchEntry::with_leg_id(
                "leg",
                sender.clone(),
                fixture.recipient.clone(),
                fixture.definition.clone(),
                2_u32,
            )])
            .into(),
        ],
    );
    let network = fixture.chain.network_id();
    let nonce = MultisigProposalNonce::new([0x6d; 32]).unwrap();
    let proposed: Vec<InstructionBox> = vec![
        Transfer::asset_quantity(
            fixture.holding(&fixture.multisig.clone()),
            3_u32,
            fixture.recipient.clone(),
        )
        .into(),
    ];
    let proposal = MultisigProposalId::derive(&network, &fixture.multisig, &nonce);
    let signer_key = fixture.signer.clone();
    let multisig = fixture.submit(
        &signer_key,
        vec![
            MultisigPropose::new(fixture.multisig.clone(), nonce, proposed.clone(), None).into(),
            MultisigApprove::new(fixture.multisig.clone(), proposal, HashOf::new(&proposed)).into(),
        ],
    );
    assert!(mint.succeeded && transfer.succeeded && burn.succeeded && batch.succeeded);
    assert!(multisig.succeeded);
    assert!(
        !failed.succeeded,
        "insufficient funds reject the transaction"
    );

    let app = fixture.app();
    let feed = leaked_feed(MAX_ROWS);
    feed.fold_to_tip(&app, true);
    assert_eq!(feed.read().height as u64, fixture.chain.height());
    let definition = fixture.definition.to_string();
    let all = DataspaceReadVisibility::all_for_tests();
    let rows = walk(&feed.read(), &definition, &ListQuery::new(), &all);
    let text = |account: &AccountId| Some(account.to_string());
    let shape = |row: &Map| {
        (
            row["kind"].as_str().unwrap().to_owned(),
            row["from_account_id"].as_str().map(str::to_owned),
            row["to_account_id"].as_str().map(str::to_owned),
            row["amount"].as_str().unwrap().to_owned(),
            row["result_ok"].as_bool().unwrap(),
        )
    };
    let minted: Vec<_> = row_of(&rows, &mint).into_iter().map(shape).collect();
    assert_eq!(
        minted,
        [
            (
                "MINT".to_owned(),
                None,
                text(&fixture.multisig),
                "10".to_owned(),
                true
            ),
            (
                "MINT".to_owned(),
                None,
                text(&sender),
                "100".to_owned(),
                true
            ),
        ],
        "newest movement first within the transaction"
    );
    let transfer_rows = row_of(&rows, &transfer);
    assert_eq!(
        transfer_rows
            .iter()
            .map(|row| shape(row))
            .collect::<Vec<_>>(),
        [(
            "TRANSFER".to_owned(),
            text(&sender),
            text(&fixture.recipient),
            "10".to_owned(),
            true
        )]
    );
    assert_eq!(
        transfer_rows[0]["block_height"].as_u64(),
        Some(transfer.height)
    );
    assert_eq!(
        transfer_rows[0]["authority"].as_str(),
        text(&sender).as_deref()
    );
    assert_eq!(
        transfer_rows[0]["id"].as_str(),
        Some(format!("{}:0", transfer.tx_hash).as_str())
    );
    assert_eq!(
        row_of(&rows, &failed)
            .into_iter()
            .map(shape)
            .collect::<Vec<_>>(),
        [(
            "TRANSFER".to_owned(),
            text(&sender),
            text(&fixture.recipient),
            "1000".to_owned(),
            false
        )],
        "a failed transaction keeps its attempted leg"
    );
    assert_eq!(
        row_of(&rows, &burn)
            .into_iter()
            .map(shape)
            .collect::<Vec<_>>(),
        [("BURN".to_owned(), text(&sender), None, "5".to_owned(), true)]
    );
    assert_eq!(
        row_of(&rows, &batch)
            .into_iter()
            .map(shape)
            .collect::<Vec<_>>(),
        [(
            "TRANSFER".to_owned(),
            text(&sender),
            text(&fixture.recipient),
            "2".to_owned(),
            true
        )]
    );
    let executed = row_of(&rows, &multisig);
    assert_eq!(
        executed.iter().map(|row| shape(row)).collect::<Vec<_>>(),
        [(
            "TRANSFER".to_owned(),
            text(&fixture.multisig),
            text(&fixture.recipient),
            "3".to_owned(),
            true
        )],
        "the executed proposal moved value once; the proposal itself is not a movement"
    );
    assert_eq!(
        executed[0]["authority"].as_str(),
        Some(
            AccountId::new(fixture.signer.public_key().clone())
                .to_string()
                .as_str()
        )
    );
    println!(
        "TRANSFER row: {}",
        norito::json::to_json(&Value::Object(transfer_rows[0].clone())).unwrap()
    );
    println!(
        "multisig TRANSFER row: {}",
        norito::json::to_json(&Value::Object(executed[0].clone())).unwrap()
    );
    assert_eq!(rows.len(), 7);

    // Successful money only; an incremental read sees only the newer blocks.
    let settled = walk(
        &feed.read(),
        &definition,
        &ListQuery::new().filter(field("result_ok").eq(true)),
        &all,
    );
    assert_eq!(settled.len(), 6);
    let newer = walk(
        &feed.read(),
        &definition,
        &ListQuery::new().filter(field("block_height").gt(burn.height)),
        &all,
    );
    assert_eq!(newer.len(), 2);
    assert!(
        walk(
            &feed.read(),
            &definition,
            &ListQuery::new(),
            &DataspaceReadVisibility::default()
        )
        .is_empty(),
        "fixture entrypoints carry no visible route legs"
    );
}

#[tokio::test]
async fn reads_fold_a_short_tail_and_answer_503_until_a_long_gap_is_indexed() {
    let mut fixture = ChainFixture::new();
    let genesis = fixture.genesis.clone();
    let sender = fixture.sender_id();
    let holding = fixture.holding(&sender);
    for _ in 0..REQUEST_TAIL_BLOCKS + 3 {
        let mint = fixture.submit(
            &genesis,
            vec![Mint::asset_quantity(1_u32, holding.clone()).into()],
        );
        assert!(mint.succeeded);
    }
    let app = fixture.app();
    let feed = leaked_feed(MAX_ROWS);
    // The page refuses rather than answer from an incomplete index.
    let query = ListQuery::new();
    let definition_id = fixture.definition.clone();
    assert!(matches!(
        feed.page(
            app.state.as_ref(),
            &prepared(&query, &definition_id.to_string()),
            &definition_id,
            &DataspaceReadVisibility::all_for_tests(),
        ),
        Err(crate::Error::AppServiceUnavailable {
            code: INDEXING_CODE,
            ..
        })
    ));
    let response = feed
        .ensure_current(&app)
        .await
        .expect_err("a long gap answers 503");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["x-iroha-reject-code"], INDEXING_CODE);
    assert!(response.headers().contains_key(header::RETRY_AFTER));
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let envelope: ErrorEnvelope = norito::json::from_slice(&body).unwrap();
    assert_eq!(envelope.code, INDEXING_CODE);
    let details = envelope.details.unwrap();
    assert_eq!(
        details.expected.as_deref(),
        Some(fixture.chain.height().to_string().as_str())
    );
    assert!(details.actual.is_some() && details.retry_after_seconds.is_some());
    for _ in 0..600 {
        if !feed.backfilling.load(Ordering::Acquire) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        feed.read().height as u64,
        fixture.chain.height(),
        "one background pass"
    );
    assert!(feed.ensure_current(&app).await.is_ok());
    // A short tail is folded by the read itself.
    for _ in 0..2 {
        assert!(
            fixture
                .submit(
                    &genesis,
                    vec![Mint::asset_quantity(1_u32, holding.clone()).into()]
                )
                .succeeded
        );
    }
    assert!(feed.ensure_current(&app).await.is_ok());
    assert_eq!(feed.read().height as u64, fixture.chain.height());
    let rows = feed
        .page(
            app.state.as_ref(),
            &prepared(&ListQuery::new().limit(500), &definition_id.to_string()),
            &definition_id,
            &DataspaceReadVisibility::all_for_tests(),
        )
        .unwrap();
    assert_eq!(rows.items.len(), REQUEST_TAIL_BLOCKS + 5);
    assert!(
        rows.items
            .iter()
            .all(|row| row["kind"].as_str() == Some("MINT"))
    );
}

/// One test drives the node's shared index, through the collection engine.
#[tokio::test]
async fn the_collection_route_reads_the_shared_index_with_canonical_account_filters() {
    let mut fixture = ChainFixture::new();
    let sender = fixture.sender_id();
    let genesis = fixture.genesis.clone();
    assert!(
        fixture
            .submit(
                &genesis,
                vec![Mint::asset_quantity(40_u32, fixture.holding(&sender)).into()]
            )
            .succeeded
    );
    let sender_key = fixture.sender.clone();
    for amount in [3_u32, 4] {
        assert!(
            fixture
                .submit(
                    &sender_key,
                    vec![
                        Transfer::asset_quantity(
                            fixture.holding(&sender),
                            amount,
                            fixture.recipient.clone()
                        )
                        .into()
                    ],
                )
                .succeeded
        );
    }
    let app = fixture.app();
    assert!(
        FEED.ensure_current(&app).await.is_ok(),
        "a read folds the short fixture chain into the shared index"
    );
    let target = super::super::collection_sources::CollectionTarget::AssetMovements(
        fixture.definition.to_string(),
    );
    let page = super::super::collection_sources::execute_collection_local(
        None,
        fixture.chain.state(),
        &target,
        ListQuery::new().filter(field("from_account_id").eq(sender.to_string())),
        &crate::routing::MaybeTelemetry::for_tests(),
        &DataspaceReadVisibility::all_for_tests(),
    )
    .await
    .expect("asset movement collection");
    let amounts: Vec<_> = page
        .items
        .iter()
        .map(|row| row["amount"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(amounts, ["4", "3"]);
    assert!(
        page.items
            .iter()
            .all(|row| row["kind"].as_str() == Some("TRANSFER"))
    );
}
