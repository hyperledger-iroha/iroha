---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: Acuñar, transferir y quemar un NFT
descripción: Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencia, etiquetado de metadatos y quema.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencia, etiquetado de metadatos y quema.

## Recorrido del libro mayor

- Asegúrese de que exista la definición del NFT (por ejemplo `n0#wonderland`) junto con las cuentas de propietario/receptor usadas en el fragmento (`<i105-account-id>`, `<i105-account-id>`).
- Invoca el punto de entrada `nft_issue_and_transfer` para acuñar el NFT, transferirlo de Alice a Bob y adjuntar una bandera de metadatos que describe la emisión.
- Inspecciona el estado del libro mayor de NFT con `iroha_cli ledger nfts list --account <id>` o los equivalentes del SDK para verificar la transferencia, luego confirma que el activo se elimina una vez que se ejecuta la instrucción de quema.

## Guías de SDK relacionadas

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
        ledger::nft::transfer(owner, nft, to);
        ledger::nft::set_metadata(nft, Name::parse("issued"), Json::parse("{\"issued\":\"demo\"}"));
        ledger::nft::burn(nft);
    }
}
```
