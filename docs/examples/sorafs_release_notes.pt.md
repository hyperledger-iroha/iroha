---
lang: pt
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CLI e SDK - Notas de release (v0.1.0)

## Destaques
- `sorafs_cli` agora cobre todo o pipeline de empacotamento (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) para que runners de CI chamem um
  unico binario em vez de helpers sob medida. O novo fluxo de assinatura sem chaves usa por padrao
  `SIGSTORE_ID_TOKEN`, entende provedores OIDC do GitHub Actions e emite um resumo JSON deterministico
  junto com o bundle de assinaturas.
- O *scoreboard* de fetch multi-source vem como parte do `sorafs_car`: normaliza a telemetria de
  provedores, aplica penalidades de capacidade, persiste relatorios JSON/Norito, e alimenta o
  simulador do orquestrador (`sorafs_fetch`) pelo registry handle compartilhado. Os fixtures em
  `fixtures/sorafs_manifest/ci_sample/` demonstram as entradas e saidas deterministicas que CI/CD
  deve comparar.
- A automacao de releases esta codificada em `ci/check_sorafs_cli_release.sh` e
  `scripts/release_sorafs_cli.sh`. Cada release agora arquiva o bundle de manifesto,
  a assinatura, os resumos `manifest.sign/verify` e o snapshot do scoreboard para que reviewers de
  governance possam rastrear artefatos sem reexecutar o pipeline.

## Compatibilidade
- Mudancas breaking: **Nenhuma.** Todas as adicoes de CLI sao flags/subcommands aditivos; as
  invocacoes existentes continuam funcionando sem modificacao.
- Versoes minimas de gateway/node: requer Torii `2.0.0-rc.2.0` (ou mais recente) para que a
  API de range de chunks, quotas de stream-token, e headers de capacidade expostos por
  `crates/iroha_torii` estejam disponiveis. Os nodes de storage devem executar a stack host do SoraFS no
  commit `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (inclui os novos inputs do scoreboard e o cabeamento
  de telemetria).
- Dependencias upstream: nao ha bumps de terceiros alem da base do workspace; a release reutiliza as
  versoes fixadas de `blake3`, `reqwest`, e `sigstore` em `Cargo.lock`.

## Passos de upgrade
1. Atualize os crates alinhados no seu workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Reexecute a gate de release localmente (ou no CI) para confirmar cobertura de fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Regenere artefatos assinados e resumos com a config curada:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Copie bundles/proofs renovados para `fixtures/sorafs_manifest/ci_sample/` se a release
   atualizar fixtures canonicos.

## Verificacao
- Commit do release gate: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` imediatamente apos o gate passar).
- Saida de `ci/check_sorafs_cli_release.sh`: arquivada em
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (anexada ao bundle de release).
- Digest do bundle de manifesto: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Digest do resumo de proof: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Digest do manifesto (para cross-checks de attestation downstream):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (de `manifest.sign.summary.json`).

## Notas para operadores
- O gateway Torii agora aplica o header de capacidade `X-Sora-Chunk-Range`. Atualize
  allowlists para admitir clientes que apresentem os novos scopes de stream token; tokens antigos
  sem o claim de range serao throttled.
- `scripts/sorafs_gateway_self_cert.sh` integra a verificacao de manifestos. Ao rodar o
  harness de self-cert, forneca o bundle de manifesto recem gerado para que o wrapper falhe rapido
  se houver drift de assinatura.
- Dashboards de telemetria devem ingerir o novo export do scoreboard (`scoreboard.json`) para
  reconciliar elegibilidade de provedores, atribuicao de pesos e motivos de recusa.
- Arquive os quatro resumos canonicos em cada rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Tickets de governance referenciam esses arquivos
  exatos durante a aprovacao.

## Agradecimentos
- Storage Team - consolidacao end-to-end do CLI, renderer de chunk-plan, e cabeamento de
  telemetria do scoreboard.
- Tooling WG - pipeline de release (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) e bundle deterministico de fixtures.
- Gateway Operations - capability gating, revisao de politica de stream-token, e playbooks
  atualizados de self-certificacao.
