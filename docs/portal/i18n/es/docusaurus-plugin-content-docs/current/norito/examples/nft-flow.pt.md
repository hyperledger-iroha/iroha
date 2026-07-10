---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: Cunhar, transferir y quemar un NFT
descripción: Percorre o ciclo de vida de un NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre el ciclo de vida de un NFT de inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.

## Roteiro do livro razao

- Garantía que a definicao do NFT (por ejemplo `n0#wonderland`) existe junto com as contas de dono/destinatario usadas no trecho (`<i105-account-id>`, `<i105-account-id>`).
- Invoque el punto de entrada `nft_issue_and_transfer` para Cunhar o NFT, transferi-lo de Alice para Bob y anexar una señal de metadados que descreva a emissao.
- Inspeccione el estado del libro razao de NFT con `iroha_cli ledger nfts list --account <id>` o sus equivalentes del SDK para verificar una transferencia, después de confirmar que o activo y eliminado cuando a instrucciones de queima roda.

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

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
