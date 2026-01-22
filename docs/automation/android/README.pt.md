---
lang: pt
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Baseline de automação da documentação Android (AND5)

O item AND5 do roadmap exige que a automação de documentação, localização e
publicação seja auditável antes do início do AND6 (CI & Compliance). Esta pasta
registra os comandos, artefatos e o layout de evidências que AND5/AND6
referenciam, espelhando os planos em
`docs/source/sdk/android/developer_experience_plan.md` e
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Pipelines e comandos

| Tarefa | Comando(s) | Artefatos esperados | Notas |
|-------|------------|---------------------|------|
| Sincronização de stubs de localização | `python3 scripts/sync_docs_i18n.py` (opcionalmente passar `--lang <code>` por execução) | Arquivo de log em `docs/automation/android/i18n/<timestamp>-sync.log` mais os commits dos stubs traduzidos | Mantém `docs/i18n/manifest.json` alinhado com os stubs traduzidos; o log registra os códigos de idioma tocados e o commit capturado no baseline. |
| Verificação de fixtures + paridade Norito | `ci/check_android_fixtures.sh` (envolve `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Copiar o resumo JSON gerado para `docs/automation/android/parity/<stamp>-summary.json` | Verifica payloads de `java/iroha_android/src/test/resources`, hashes de manifestos e comprimentos de fixtures assinadas. Anexe o resumo junto com as evidências de cadência em `artifacts/android/fixture_runs/`. |
| Manifesto de amostras e prova de publicação | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (executa testes + SBOM + provenance) | Metadados do bundle de provenance e o `sample_manifest.json` de `docs/source/sdk/android/samples/` armazenado em `docs/automation/android/samples/<version>/` | Conecta os apps de amostra AND5 à automação de releases: capture o manifesto gerado, o hash do SBOM e o log de provenance para a revisão beta. |
| Feed do dashboard de paridade | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` seguido de `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Copiar o snapshot `metrics.prom` ou o export JSON do Grafana para `docs/automation/android/parity/<stamp>-metrics.prom` | Alimenta o plano do dashboard para que AND5/AND7 possam verificar contadores de envios inválidos e adoção de telemetria. |

## Captura de evidências

1. **Carimbe tudo com data/hora.** Nomeie arquivos com timestamps UTC
   (`YYYYMMDDTHHMMSSZ`) para que dashboards de paridade, atas de governança e
   docs publicados possam referenciar a mesma execução.
2. **Referencie commits.** Cada log deve incluir o hash do commit da execução e
   qualquer configuração relevante (por exemplo, `ANDROID_PARITY_PIPELINE_METADATA`).
   Quando a privacidade exigir redação, inclua uma nota e o link para o cofre seguro.
3. **Arquive contexto mínimo.** Apenas resumos estruturados (JSON, `.prom`, `.log`)
   são versionados. Artefatos pesados (bundles APK, screenshots) devem ficar em
   `artifacts/` ou em object storage com um hash assinado registrado no log.
4. **Atualize entradas de status.** Quando os marcos AND5 avançarem em `status.md`,
   cite o arquivo correspondente (por exemplo,
   `docs/automation/android/parity/20260324T010203Z-summary.json`) para que os
   auditores rastreiem o baseline sem vasculhar logs de CI.

Seguir este layout atende ao pré-requisito do AND6 de “baselines de
docs/automation disponíveis para auditoria” e mantém o programa de documentação
Android alinhado com os planos publicados.
