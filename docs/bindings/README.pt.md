---
lang: pt
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Governança de bindings e fixtures do SDK

O WP1-E no roadmap aponta “docs/bindings” como o local canônico para manter o
estado dos bindings entre linguagens. Este documento registra o inventário de
bindings, os comandos de regeneração, os guardas de desvio e os locais de
 evidência para que os gates de paridade GPU (WP1-E/F/G) e o conselho de cadência
entre SDKs tenham uma única referência.

## Guardrails compartilhados
- **Playbook canônico:** `docs/source/norito_binding_regen_playbook.md` descreve a
  política de rotação, a evidência esperada e o fluxo de escalonamento para
  Android, Swift, Python e futuros bindings.
- **Paridade de esquema Norito:** `scripts/check_norito_bindings_sync.py` (invocado
  via `scripts/check_norito_bindings_sync.sh` e com gate em CI por
  `ci/check_norito_bindings_sync.sh`) bloqueia builds quando os artefatos de
  esquema Rust, Java ou Python divergem.
- **Watchdog de cadência:** `scripts/check_fixture_cadence.py` lê os arquivos
  `artifacts/*_fixture_regen_state.json` e aplica as janelas de Ter/Sex (Android,
  Python) e Qua (Swift) para que os gates do roadmap tenham timestamps auditáveis.

## Matriz de bindings

| Binding | Pontos de entrada | Comando de fixture/regeneração | Guardas de desvio | Evidência |
|---------|-------------------|-------------------------------|-------------------|-----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (opcionalmente `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Detalhes dos bindings

### Android (Java)
O SDK Android fica em `java/iroha_android/` e consome as fixtures Norito
canônicas geradas por `scripts/android_fixture_regen.sh`. Esse helper exporta
blobs `.norito` frescos a partir do toolchain Rust, atualiza
`artifacts/android_fixture_regen_state.json` e registra metadados de cadência que
`scripts/check_fixture_cadence.py` e dashboards de governança consomem. A deriva
é detectada por `scripts/check_android_fixtures.py` (também ligado ao
`ci/check_android_fixtures.sh`) e por `java/iroha_android/run_tests.sh`, que
exercita os bindings JNI, a reprodução de fila WorkManager e os fallbacks de
StrongBox. Evidências de rotação, notas de falha e transcrições de rerun ficam em
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` espelha os mesmos payloads Norito via `scripts/swift_fixture_regen.sh`.
O script registra o dono da rotação, o rótulo de cadência e a origem (`live` vs
`archive`) em `artifacts/swift_fixture_regen_state.json` e alimenta o verificador
 de cadência. `scripts/swift_fixture_archive.py` permite que mantenedores ingiram
 arquivos gerados pelo Rust; `scripts/check_swift_fixtures.py` e
`ci/check_swift_fixtures.sh` impõem paridade byte a byte e limites de idade SLA,
enquanto `scripts/swift_fixture_regen.sh` suporta `SWIFT_FIXTURE_EVENT_TRIGGER`
para rotações manuais. O fluxo de escalonamento, KPIs e dashboards estão
 documentados em `docs/source/swift_parity_triage.md` e nos briefs de cadência em
`docs/source/sdk/swift/`.

### Python
O cliente Python (`python/iroha_python/`) compartilha as fixtures Android.
Executar `scripts/python_fixture_regen.sh` puxa os payloads `.norito` mais
recentes, atualiza `python/iroha_python/tests/fixtures/` e emitirá metadados de
cadência em `artifacts/python_fixture_regen_state.json` quando a primeira rotação
pós-roadmap for capturada. `scripts/check_python_fixtures.py` e
`python/iroha_python/scripts/run_checks.sh` bloqueiam pytest, mypy, ruff e a
paridade de fixtures localmente e em CI. A documentação end-to-end
(`docs/source/sdk/python/…`) e o playbook de regeneração descrevem como coordenar
rotações com os responsáveis de Android.

### JavaScript
`javascript/iroha_js/` não depende de arquivos `.norito` locais, mas WP1-E
acompanha sua evidência de releases para que as lanes de CI de GPU herdem
provenance completo. Cada release captura provenance via `npm run release:provenance`
(impulsionado por `javascript/iroha_js/scripts/record-release-provenance.mjs`),
gera e assina bundles SBOM com `scripts/js_sbom_provenance.sh`, executa o staging
assinado (`scripts/js_signed_staging.sh`) e verifica o artefato do registry com
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Os metadados resultantes
ficam em `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/` e `artifacts/js/verification/`, fornecendo evidência
 determinística para os runs de roadmap JS5/JS6 e WP1-F. O playbook de publicação
em `docs/source/sdk/js/` conecta toda a automação.
