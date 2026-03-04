---
lang: pt
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: Materiais de treinamento SNS
description: Currículo, fluxo de localização e captura de evidências de anexos exigidos pela SN-8.
---

> Espelha `docs/source/sns/training_collateral.md`. Use esta página para preparar equipes de registro, DNS, guardians e finanças antes de cada lançamento de sufixo.

## 1. Snapshot do currículo

| Trilha | Objetivos | Pré-leituras |
|-------|------------|-----------|
| Operações de registro | Enviar manifests, monitorar dashboards de KPI, escalar erros. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS e gateway | Aplicar esqueletos de resolver, ensaiar freezes/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians e conselho | Executar disputas, atualizar adendos de governança, registrar anexos. | `sns/governance-playbook`, steward scorecards. |
| Finanças e analytics | Capturar métricas ARPU/bulk, publicar pacotes de anexos. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### Fluxo dos módulos

1. **M1 — Orientação KPI (30 min):** revisar filtros de sufixo, exports e contadores de freeze. Entregável: snapshots PDF/CSV com digest SHA-256.
2. **M2 — Ciclo de vida do manifest (45 min):** construir e validar manifests do registrador, gerar esqueletos de resolver via `scripts/sns_zonefile_skeleton.py`. Entregável: diff do git mostrando o esqueleto + evidência GAR.
3. **M3 — Simulações de disputa (40 min):** simular freeze + apelação de guardian, capturar logs de CLI em `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — Captura de anexos (25 min):** exportar JSON do dashboard e executar:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Entregável: Markdown de anexo atualizado + memo regulatório + blocos do portal.

## 2. Fluxo de localização

- Idiomas: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- Cada tradução fica ao lado do arquivo fonte (`docs/source/sns/training_collateral.<lang>.md`). Atualize `status` + `translation_last_reviewed` após atualizar.
- Assets por idioma ficam em `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- Execute `python3 scripts/sync_docs_i18n.py --lang <code>` depois de editar a fonte em inglês para que os tradutores vejam o novo hash.

### Checklist de entrega

1. Atualize o stub de tradução (`status: complete`) quando estiver localizado.
2. Exporte os slides para PDF e envie para o diretório `slides/` por idioma.
3. Grave um walkthrough KPI ≤10 min; faça o link a partir do stub do idioma.
4. Abra um ticket de governança com a tag `sns-training` contendo digests de slides/workbook, links de gravação e evidências de anexos.

## 3. Ativos de treinamento

- Esboço de slides: `docs/examples/sns_training_template.md`.
- Modelo de workbook: `docs/examples/sns_training_workbook.md` (um por participante).
- Convites + lembretes: `docs/examples/sns_training_invite_email.md`.
- Formulário de avaliação: `docs/examples/sns_training_eval_template.md` (respostas arquivadas em `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. Agenda e métricas

| Ciclo | Janela | Métricas | Notas |
|-------|--------|---------|-------|
| 2026‑03 | Pós revisão de KPI | Presença %, digest de anexo registrado | `.sora` + `.nexus` cohorts |
| 2026‑06 | Pré GA `.dao` | Prontidão financeira ≥90 % | Inclui atualização de política |
| 2026‑09 | Expansão | Simulado de disputa <20 min, SLA de anexo ≤2 dias | Alinhar com incentivos SN-7 |

Capture feedback anônimo em `docs/source/sns/reports/sns_training_feedback.md` para que as próximas coortes melhorem a localização e os labs.

