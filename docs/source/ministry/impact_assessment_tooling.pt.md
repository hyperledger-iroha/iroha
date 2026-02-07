---
lang: pt
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Ferramentas de avaliação de impacto (MINFO‑4b)

Referência do roteiro: **MINFO‑4b — Ferramentas de avaliação de impacto.**  
Proprietário: Conselho de Governança / Analytics

Esta nota documenta o comando `cargo xtask ministry-agenda impact` que agora
produz a comparação automatizada da família de hash necessária para pacotes de referendo. O
ferramenta consome propostas validadas do Conselho da Agenda, o registro duplicado e
um instantâneo opcional de lista de bloqueio/política para que os revisores possam ver exatamente quais
as impressões digitais são novas, o que colide com a política existente e quantas entradas
cada família de hash contribui.

## Entradas

1. **Propostas de agenda.** Um ou mais arquivos a seguir
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Passe-os explicitamente com `--proposal <path>` ou aponte o comando para um
   diretório via `--proposal-dir <dir>` e cada arquivo `*.json` nesse caminho
   está incluído.
2. **Registro duplicado (opcional).** Um arquivo JSON correspondente
   `docs/examples/ministry/agenda_duplicate_registry.json`. Os conflitos são
   relatado em `source = "duplicate_registry"`.
3. **Snapshot da política (opcional).** Um manifesto leve que lista todos
   impressão digital já aplicada pela política do GAR/Ministério. O carregador espera que o
   esquema mostrado abaixo (veja
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   para uma amostra completa):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

Qualquer entrada cuja impressão digital `hash_family:hash_hex` corresponda a um alvo de proposta é
relatado em `source = "policy_snapshot"` com o `policy_id` referenciado.

## Uso

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Propostas adicionais podem ser anexadas através de sinalizadores `--proposal` repetidos ou por
fornecendo um diretório que contém um lote inteiro de referendo:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

O comando imprime o JSON gerado em stdout quando `--out` é omitido.

## Saída

O relatório é um artefato assinado (registre-o no pacote do referendo
Diretório `artifacts/ministry/impact/`) com a seguinte estrutura:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Anexe este JSON a cada dossiê de referendo junto com o resumo neutro para
painelistas, jurados e observadores de governança podem ver o raio exato da explosão
cada proposta. A saída é determinística (classificada por família de hash) e segura para
incluir em CI/runbooks; se o registro duplicado ou o instantâneo da política mudar,
execute novamente o comando e anexe o artefato atualizado antes do início da votação.

> **Próxima etapa:** alimentar o relatório de impacto gerado em
> [`cargo xtask ministry-panel packet`](referendum_packet.md) então o
> O dossiê `ReferendumPacketV1` contém o detalhamento da família hash e o
> lista detalhada de conflitos para a proposta em análise.