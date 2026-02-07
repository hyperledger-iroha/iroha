---
lang: pt
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Aprovação da implementação do Payload v1 (Conselho SDK, 28/04/2026).
//!
//! Captura o memorando de decisão do SDK Council exigido por `roadmap.md:M1` para que o
//! a implementação de carga útil criptografada v1 tem um registro auditável (entregável M1.4).

# Decisão de implementação da carga útil v1 (28/04/2026)

- **Presidente:** Líder do Conselho SDK (M. Takemiya)
- **Membros votantes:** Swift Lead, CLI Mantenedor, Confidential Assets TL, DevRel WG
- **Observadores:** Gerenciamento de Programa, Operações de Telemetria

## Entradas revisadas

1. **Ligações rápidas e remetentes** — `ShieldRequest`/`UnshieldRequest`, remetentes assíncronos e ajudantes do construtor Tx chegaram com testes de paridade e docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **Ergonomia CLI** — O auxiliar `iroha app zk envelope` cobre fluxos de trabalho de codificação/inspeção, além de diagnóstico de falhas, alinhado com os requisitos de ergonomia do roteiro.【crates/iroha_cli/src/zk.rs:1256】
3. **Aparelhos determinísticos e conjuntos de paridade** - aparelho compartilhado + validação Rust/Swift para manter bytes/superfícies de erro Norito alinhado.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Decisão

- **Aprovar a implementação do payload v1** para SDKs e CLI, permitindo que as carteiras Swift originem envelopes confidenciais sem nenhum encanamento personalizado.
- **Condições:** 
  - Mantenha os equipamentos de paridade sob alertas de desvio de CI (vinculados a `scripts/check_norito_bindings_sync.py`).
  - Documente o playbook operacional em `docs/source/confidential_assets.md` (já atualizado via Swift SDK PR).
  - Registre a calibração + evidências de telemetria antes de lançar qualquer sinalizador de produção (rastreado em M2).

## Itens de ação

| Proprietário | Artigo | Vencimento |
|-------|------|-----|
| Liderança rápida | Anuncie a disponibilidade do GA + trechos README | 01/05/2026 |
| Mantenedor CLI | Adicionar auxiliar `iroha app zk envelope --from-fixture` (opcional) | Backlog (sem bloqueio) |
| GT DevRel | Atualizar os guias de início rápido da carteira com instruções de carga útil v1 | 05/05/2026 |

> **Nota:** Este memorando substitui a chamada temporária “pendente de aprovação do conselho” em `roadmap.md:2426` e satisfaz o item M1.4 do rastreador. Atualize `status.md` sempre que os itens de ação de acompanhamento forem encerrados.