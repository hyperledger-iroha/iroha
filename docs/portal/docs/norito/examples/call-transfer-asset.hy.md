<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: Հրավիրեք հոսթի փոխանցում Kotodama-ից
description: Ցույց է տալիս, թե ինչպես կարող է Kotodama մուտքի կետը զանգահարել հյուրընկալող `transfer_asset` հրահանգը՝ ներկառուցված մետատվյալների վավերացմամբ:
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Ցույց է տալիս, թե ինչպես կարող է Kotodama մուտքի կետը զանգահարել հյուրընկալող `transfer_asset` հրահանգը՝ ներկառուցված մետատվյալների վավերացմամբ:

## Լեջերի քայլարշավ

- Ֆինանսավորեք պայմանագրային մարմնին (օրինակ՝ `<i105-account-id>` պայմանագրային հաշվի համար) այն ակտիվով, որը նա կփոխանցի և մարմնին կտրամադրի `CanTransfer` դերը կամ համարժեք թույլտվությունը:
- Զանգահարեք `call_transfer_asset` մուտքի կետ՝ պայմանագրային հաշվից 5 միավոր Բոբին փոխանցելու համար (`<i105-account-id>`)՝ արտացոլելով այն ճանապարհը, որով շղթայական ավտոմատացումը կարող է փաթեթավորել հյուրընկալող զանգերը:
- Ստուգեք մնացորդները `FindAccountAssets`-ի կամ `iroha_cli ledger asset list --account <i105-account-id>`-ի միջոցով և ստուգեք իրադարձությունները՝ հաստատելու համար, որ մետատվյալների պահակը գրանցել է փոխանցման համատեքստը:

## Առնչվող SDK ուղեցույցներ

- [Rust SDK արագ մեկնարկ] (/sdks/rust)
- [Python SDK արագ մեկնարկ] (/sdks/python)
- [JavaScript SDK արագ մեկնարկ] (/sdks/javascript)

[Ներբեռնեք Kotodama աղբյուրը](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```