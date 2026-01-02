---
lang: fr
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CLI & SDK - Notes de release (v0.1.0)

## Points forts
- `sorafs_cli` couvre maintenant tout le pipeline de packaging (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) afin que les runners CI invoquent un
  seul binaire au lieu d'helpers sur mesure. Le nouveau flux de signature sans cles utilise par defaut
  `SIGSTORE_ID_TOKEN`, comprend les fournisseurs OIDC de GitHub Actions et emet un resume JSON deterministe
  avec le bundle de signatures.
- Le *scoreboard* de multi-source fetch est livre avec `sorafs_car`: il normalise la telemetrie des
  providers, applique des penalites de capacite, persiste des rapports JSON/Norito, et alimente le
  simulateur d'orchestrateur (`sorafs_fetch`) via le registry handle partage. Les fixtures sous
  `fixtures/sorafs_manifest/ci_sample/` montrent les inputs et outputs deterministes que CI/CD doit comparer.
- L'automatisation des releases est codee dans `ci/check_sorafs_cli_release.sh` et
  `scripts/release_sorafs_cli.sh`. Chaque release archive maintenant le bundle de manifeste,
  la signature, les resumes `manifest.sign/verify` et le snapshot du scoreboard afin que les reviewers de
  governance puissent tracer les artefacts sans relancer le pipeline.

## Compatibilite
- Changements breaking: **Aucun.** Toutes les additions CLI sont des flags/subcommands additifs; les
  invocations existantes continuent de fonctionner sans modification.
- Versions minimales gateway/node: requiert Torii `2.0.0-rc.2.0` (ou plus recent) pour que l'API de
  chunk-range, les quotas de stream-token, et les headers de capacite exposes par `crates/iroha_torii`
  soient disponibles. Les nodes de storage doivent executer la stack host SoraFS du commit
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (inclut les nouveaux inputs du scoreboard et le cablage
  de telemetrie).
- Dependances upstream: aucun bump tiers au-dela de la base du workspace; la release reutilise les
  versions epinglees de `blake3`, `reqwest`, et `sigstore` dans `Cargo.lock`.

## Etapes de mise a niveau
1. Mettez a jour les crates alignes dans votre workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Relancez la gate de release localement (ou en CI) pour confirmer la couverture fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Regenerez les artefacts signes et les resumes avec la config prescriptive:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Copiez les bundles/proofs rafraichis dans `fixtures/sorafs_manifest/ci_sample/` si la release
   met a jour les fixtures canoniques.

## Verification
- Commit de release gate: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` juste apres la reussite du gate).
- Sortie de `ci/check_sorafs_cli_release.sh`: archivee dans
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (jointe au bundle de release).
- Digest du bundle de manifeste: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Digest du resume de proof: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Digest du manifeste (pour les croisements d'attestation downstream):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (depuis `manifest.sign.summary.json`).

## Notes pour les operateurs
- Le gateway Torii applique maintenant le header de capacite `X-Sora-Chunk-Range`. Mettez a jour les
  allowlists pour admettre les clients presentant les nouveaux scopes de stream token; les anciens tokens
  sans claim de range seront throttled.
- `scripts/sorafs_gateway_self_cert.sh` integre la verification du manifeste. Lors de l'execution du
  harness self-cert, fournissez le bundle de manifeste fraichement genere pour que le wrapper echoue
  rapidement en cas de derive de signature.
- Les dashboards de telemetrie doivent ingerer le nouvel export du scoreboard (`scoreboard.json`) pour
  concilier l'eligibilite des providers, les assignations de poids et les raisons de refus.
- Archivez les quatre resumes canoniques a chaque rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Les tickets de governance referencent ces fichiers exacts lors de
  l'approbation.

## Remerciements
- Storage Team - consolidation end-to-end du CLI, renderer de chunk-plan, et cablage de telemetrie du
  scoreboard.
- Tooling WG - pipeline de release (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) et bundle deterministe de fixtures.
- Gateway Operations - capability gating, revue de politique stream-token, et playbooks
  self-cert mis a jour.
