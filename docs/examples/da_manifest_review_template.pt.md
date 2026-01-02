---
lang: pt
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

# Pacote de governanca do manifesto de disponibilidade de dados (Modelo)

Use este modelo quando paineis do Parlamento revisarem manifestos de DA para subsidios,
takedowns ou mudancas de retencao (roadmap DA-10). Copie o Markdown para o ticket de
governanca, preencha os placeholders e anexe o arquivo completo junto com os payloads Norito
assinados e os artefatos de CI referenciados abaixo.

```markdown
## Metadados do manifesto
- Nome / versao do manifesto: <string>
- Classe do blob e tag de governanca: <taikai_segment / da.taikai.live>
- Digest BLAKE3 (hex): `<digest>`
- Hash do payload Norito (opcional): `<digest>`
- Envelope de origem / URL: <https://.../manifest_signatures.json>
- ID do snapshot de politica Torii: `<unix timestamp or git sha>`

## Verificacao de assinaturas
- Fonte de coleta do manifesto / ticket de storage: `<hex>`
- Comando/saida de verificacao: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (trecho de log anexado?)
- `manifest_blake3` reportado pela ferramenta: `<digest>`
- `chunk_digest_sha3_256` reportado pela ferramenta: `<digest>`
- Multihashes dos signatarios do conselho:
  - `<did:...>` / `<ed25519 multihash>`
- Timestamp de verificacao (UTC): `<2026-02-20T11:04:33Z>`

## Verificacao de retencao
| Campo | Esperado (politica) | Observado (manifesto) | Evidencia |
|-------|---------------------|-----------------------|----------|
| Retencao hot (segundos) | <p. ex., 86400> | <valor> | `<torii.da_ingest.replication_policy dump | CI link>` |
| Retencao cold (segundos) | <p. ex., 1209600> | <valor> |  |
| Replicas exigidas | <valor> | <valor> |  |
| Classe de armazenamento | <hot / warm / cold> | <valor> |  |
| Tag de governanca | <da.taikai.live> | <valor> |  |

## Contexto
- Tipo de solicitacao: <Subsidio | Takedown | Rotacao de manifesto | Congelamento de emergencia>
- Ticket de origem / referencia de compliance: <link ou ID>
- Impacto em subsidio / aluguel: <mudanca XOR esperada ou "n/a">
- Link de apelacao de moderacao (se houver): <case_id ou link>

## Resumo da decisao
- Painel: <Infraestrutura | Moderacao | Tesouraria>
- Resultado da votacao: `<for>/<against>/<abstain>` (quorum `<threshold>` atingido?)
- Altura ou janela de ativacao / rollback: `<block/slot range>`
- Acoes de acompanhamento:
  - [ ] Notificar a Tesouraria / ops de aluguel
  - [ ] Atualizar relatorio de transparencia (`TransparencyReportV1`)
  - [ ] Agendar auditoria de buffer

## Escalacao e relatorio
- Trilha de escalacao: <Subsidio | Compliance | Congelamento de emergencia>
- Link / ID do relatorio de transparencia (se atualizado): <`TransparencyReportV1` CID>
- Bundle de proof-token ou referencia ComplianceUpdate: <caminho ou ticket ID>
- Delta do ledger de aluguel / reserva (se aplicavel): <`ReserveSummaryV1` snapshot link>
- URL(s) de snapshot de telemetria: <Grafana permalink ou artefact ID>
- Notas para a ata do Parlamento: <resumo de prazos / obrigacoes>

## Anexos
- [ ] Manifesto Norito assinado (`.to`)
- [ ] Resumo JSON / artefato de CI comprovando valores de retencao
- [ ] Proof token ou pacote de compliance (para takedowns)
- [ ] Snapshot de telemetria do buffer (`iroha_settlement_buffer_xor`)
```

Arquive cada pacote completo na entrada do DAG de governanca da votacao para que revisoes
subsequentes possam referenciar o digest do manifesto sem repetir toda a cerimonia.
