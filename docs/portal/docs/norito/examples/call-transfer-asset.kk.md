<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
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
title: Kotodama жүйесінен хост тасымалдауын шақыру
description: Kotodama кіру нүктесі кірістірілген метадеректерді тексеру арқылы `transfer_asset` нұсқаулығын хостқа қалай шақыра алатынын көрсетеді.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama кіру нүктесі кірістірілген метадеректерді тексеру арқылы `transfer_asset` хост нұсқаулығын қалай шақыра алатынын көрсетеді.

## Бухгалтерлік кітапшаға шолу

- Келісім-шарт бойынша уәкілетті органды (мысалы, келісім-шарт шоты үшін `<i105-account-id>`) ол беретін активпен қаржыландырыңыз және уәкілетті органға `CanTransfer` рөлін немесе баламалы рұқсатты береді.
- Тізбектегі автоматтандырудың хост қоңырауларын орау мүмкіндігін көрсететіндей етіп, келісім-шарт тіркелгісінен Bob-қа (`<i105-account-id>`) 5 бірлік аудару үшін `call_transfer_asset` кіру нүктесіне қоңырау шалыңыз.
- `FindAccountAssets` немесе `iroha_cli ledger asset list --account <i105-account-id>` арқылы теңгерімдерді тексеріңіз және тасымалдау контекстін тіркеген метадеректер қорғаушысы растау үшін оқиғаларды тексеріңіз.

## Қатысты SDK нұсқаулықтары

- [Rust SDK жылдам іске қосу](/sdks/rust)
- [Python SDK жылдам іске қосу](/sdks/python)
- [JavaScript SDK жылдам іске қосу](/sdks/javascript)

[Kotodama көзін жүктеп алыңыз](/norito-snippets/call-transfer-asset.ko)

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