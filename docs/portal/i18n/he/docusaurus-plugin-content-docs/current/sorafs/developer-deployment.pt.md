---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פריסת מפתח
כותרת: Notas deployment da SoraFS
sidebar_label: פריסת Notas
תיאור: רשימת רשימות עבור מקדם צינור של SoraFS de CI עבור ייצור.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/deployment.md`. Mantenha ambas as copias sincronizadas.
:::

# פריסת Notas

O זרימת עבודה של empacotamento da SoraFS fortalece o determinismo, יש מעבר CI עבור producao לבקש מעקות בטיחות עיקריים. השתמש ב-esta checklist ao levar כ-ferramentas para gateways e provedores de armazenamento reais.

## לפני טיסה

- **Alinhamento do registro** - confirme que os perfis de chunker e manifests referenciam a mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politica de admission** - עדכון פרסומות של ספק OS assinados e alias ecessarios necessarios para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin registry** - mantenha `docs/source/sorafs/runbooks/pin_registry_ops.md` por perto para cenarios de recuperacao (rotacao de alias, falhas de replicacao).

## Configuracao do ambiente

- שערים המפותחים או הזרמת נקודת קצה להוכחה (`POST /v2/sorafs/proof/stream`) עבור CLI פולט קורות חיים של טלמטריה.
- הגדר פוליטיקה `sorafs_alias_cache` usando os padroes em `iroha_config` או עוזר לעשות CLI (`sorafs_cli manifest submit --alias-*`).
- אסימוני זרם של Forneca (ou credenciais Torii) באמצעות אום מנהל סודי seguro.
- היצואנים של טלמטריה (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e envie para seu stack Prometheus/OTel.

## אסטרטגיית השקה

1. **מתבטא בכחול/ירוק**
   - השתמש ב-`manifest submit --summary-out` למען השקת תשובות חדשות.
   - שים לב ל-`torii_sorafs_gateway_refusals_total` עבור אי-התאמות של קפטר.
2. **Validacao de הוכחות**
   - Trate falhas em `sorafs_cli proof stream` como bloqueadores פריסה; picos de latencia costumam מצביעים על מצערת לעשות מבחן או רמות קונפיגוראדו.
   - `proof verify` deve fazer parte לעשות בדיקת עשן pos-pin para garantir que o CAR hospedado pelos provedores ainda corresponde ao digest do manifest.
3. **לוחות מחוונים של טלמטריה**
   - ייבוא `docs/examples/sorafs_proof_streaming_dashboard.json` לא Grafana.
   - Adicione paineis para saude do pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) estatisticas de chunk range.
4. **Habilitacao רב מקורות**
   - Siga os passos de rollout em etapas em `docs/source/sorafs/runbooks/multi_source_rollout.md` ao ativar o orquestrador e arquive artefatos de scoreboard/telemetria para auditorias.

## Tratamento de incidentes

- Siga os caminhos de escalonamento em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` עבור קוד השער ו-Esgotamento de stream-token.
  - `dispute_revocation_runbook.md` quando ocorrerem disputas de replicacao.
  - `sorafs_node_ops.md` para manutencao no nivel de nodo.
  - `multi_source_rollout.md` עבור מעקפים לעשות אופקסטרדור, רשימה שחורה של עמיתים והשקפות.
- Registre falhas de proofs and anomalias de latencia no GovernanceLog via as APIs de PoR tracker existentes para que a governanca avalie o desempenho dos provedores.

## Proximos passos- שלב אוטומאטי או מדרור (`sorafs_car::multi_fetch`) או מתקן אחזור ריבוי מקורות (SF-6b).
- שדרוגים של Acompanhe de PDP/PoTR sob SF-13/SF-14; o CLI e a documentacao vao evoluir para expor prazos e selecao de tiers quando essas proofs estabilizarem.

Ao combinar estas notas deployment com o quickstart e as receitas de CI, as equipes podem passer de experimentos locais para pipelines SoraFS em producao com um processo repetivevel e observavel.