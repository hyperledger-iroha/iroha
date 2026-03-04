---
lang: pt
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

# Pacote do Parlamento de incentivos de relay SoraNet

Este bundle captura os artefatos exigidos pelo Parlamento Sora para aprovar pagamentos automaticos de relay (SNNet-7):

- `reward_config.json` - configuracao do motor de recompensas serializavel Norito, pronta para ingestao por `iroha app sorafs incentives service init`. O `budget_approval_id` corresponde ao hash listado nas atas de governance.
- `shadow_daemon.json` - mapeamento de beneficiarios e bonds consumido pelo harness de replay (`shadow-run`) e pelo daemon de producao.
- `economic_analysis.md` - resumo de fairness para a simulacao shadow 2025-10 -> 2025-11.
- `rollback_plan.md` - playbook operacional para desabilitar pagamentos automaticos.
- Artefatos de suporte: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Checagens de integridade

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

Compare os digests com os valores registrados nas atas do Parlamento. Verifique a assinatura do shadow-run conforme descrito em
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Atualizacao do pacote

1. Atualize `reward_config.json` sempre que os pesos de recompensa, o pagamento base ou o hash de aprovacao mudarem.
2. Reexecute a simulacao shadow de 60 dias, atualize `economic_analysis.md` com os novos achados, e commite o JSON + assinatura destacada.
3. Apresente o bundle atualizado ao Parlamento junto com exports de dashboards do Observatory ao solicitar renovacao.
