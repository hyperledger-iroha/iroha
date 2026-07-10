---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: Acuñar, transferir e querer um NFT
descrição: Repita o ciclo de vida de um NFT de extremo a extremo: conhecimento do proprietário, transferência, etiqueta de metadados e pergunta.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

Repita o ciclo de vida de um NFT de extremo a extremo: conhecimento do proprietário, transferência, etiqueta de metadados e pergunta.

## Recorrido do livro prefeito

- Certifique-se de que existe a definição do NFT (por exemplo, `n0#wonderland`) junto com as informações do proprietário/receptor usadas no fragmento (`<i105-account-id>`, `<i105-account-id>`).
- Invoque o ponto de entrada `nft_issue_and_transfer` para obter o NFT, transfira-o de Alice para Bob e adicione uma faixa de metadados que descreve a emissão.
- Inspecione o estado do livro maior de NFT com `iroha_cli ledger nfts list --account <id>` ou os equivalentes do SDK para verificar a transferência, depois confirme que o ativo foi eliminado uma vez que a instrução de pergunta foi executada.

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/nft-flow.ko)

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
