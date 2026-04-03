<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: NFT-ті таратыңыз, тасымалдаңыз және жағыңыз
description: NFT өмірлік циклінің соңына дейін жүреді: иесіне беру, метадеректерді тасымалдау, тегтеу және жазу.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT өмірлік циклінің соңына дейін жүреді: иесіне беру, метадеректерді тасымалдау, тегтеу және жазу.

## Бухгалтерлік кітапшаға шолу

- NFT анықтамасының (мысалы, `n0#wonderland`) үзіндіде пайдаланылған иесі/алушы тіркелгілерімен қатар бар екеніне көз жеткізіңіз (Алис үшін `<i105-account-id>`, Боб үшін `<i105-account-id>`).
- NFT жасау үшін `nft_issue_and_transfer` кіру нүктесін шақырыңыз, оны Алисадан Бобқа тасымалдаңыз және шығарылымды сипаттайтын метадеректер жалаушасын тіркеңіз.
- Тасымалдауды тексеру үшін `iroha_cli ledger nft list --account <id>` немесе SDK эквиваленттерімен NFT кітапшасының күйін тексеріңіз, содан кейін жазу нұсқауы орындалғаннан кейін актив жойылғанын растаңыз.

## Қатысты SDK нұсқаулықтары

- [Rust SDK жылдам іске қосу](/sdks/rust)
- [Python SDK жылдам іске қосу](/sdks/python)
- [JavaScript SDK жылдам іске қосу](/sdks/javascript)

[Kotodama көзін жүктеп алыңыз](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  #[access(read="*", write="*")]
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```