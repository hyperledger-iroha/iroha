---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbook de lancement Gateway & DNS SoraFS

Cette copie du portail reflète le runbook canonique dans
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Elle capture les garde-fous opérationnels du workstream DNS décentralisé & Gateway
afin que les responsables réseau, ops et documentation puissent répéter la pile
d’automatisation avant le lancement 2025-03.

## Portée et livrables

- Lier les jalons DNS (SF-4) et gateway (SF-5) en répétant la dérivation déterministe
  des hôtes, les releases du répertoire de resolvers, l’automatisation TLS/GAR et la
  capture de preuves.
- Garder les entrées du kickoff (agenda, invitation, suivi de présence, snapshot de
  télémétrie GAR) synchronisées avec les dernières affectations d’owners.
- Produire un bundle d’artefacts auditable pour les reviewers de gouvernance : notes
  de release du répertoire de resolvers, logs de sondes gateway, sortie du harness de
  conformité et synthèse Docs/DevRel.

## Rôles et responsabilités

| Workstream | Responsabilités | Artefacts requis |
|------------|------------------|------------------|
| Networking TL (pile DNS) | Maintenir le plan déterministe des hôtes, exécuter les releases de répertoire RAD, publier les inputs de télémétrie des resolvers. | `artifacts/soradns_directory/<ts>/`, diffs de `docs/source/soradns/deterministic_hosts.md`, métadonnées RAD. |
| Ops Automation Lead (gateway) | Exécuter les drills d’automatisation TLS/ECH/GAR, lancer `sorafs-gateway-probe`, mettre à jour les hooks PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de probe, entrées `ops/drill-log.md`. |
| QA Guild & Tooling WG | Lancer `ci/check_sorafs_gateway_conformance.sh`, curer les fixtures, archiver les bundles self-cert Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | Capturer les minutes, mettre à jour le pré-read de design + annexes, publier la synthèse d’évidence dans ce portail. | Fichiers `docs/source/sorafs_gateway_dns_design_*.md` à jour et notes de rollout. |

## Entrées et prérequis

- Spécification d’hôtes déterministes (`docs/source/soradns/deterministic_hosts.md`) et
  scaffolding d’attestation de resolver (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefacts gateway : manuel opérateur, helpers d’automatisation TLS/ECH,
  guidance direct-mode, workflow self-cert sous `docs/source/sorafs_gateway_*`.
- Tooling : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, et helpers CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets : clé de release GAR, credentials ACME DNS/TLS, routing key PagerDuty,
  token auth Torii pour les fetches de resolvers.

## Checklist pré-vol

1. Confirmer les participants et l’agenda en mettant à jour
   `docs/source/sorafs_gateway_dns_design_attendance.md` et en diffusant l’agenda
   courant (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Préparer les racines d’artefacts telles que
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` et
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Rafraîchir les fixtures (manifests GAR, preuves RAD, bundles de conformité gateway) et
   s’assurer que l’état `git submodule` correspond au dernier tag de rehearsal.
4. Vérifier les secrets (clé de release Ed25519, fichier de compte ACME, token PagerDuty)
   et leur correspondance avec les checksums du vault.
5. Faire un smoke-test des cibles de télémétrie (endpoint Pushgateway, board GAR Grafana)
   avant le drill.

## Étapes de rehearsal d’automatisation

### Carte d’hôtes déterministe & release du répertoire RAD

1. Exécuter l’helper de dérivation déterministe des hôtes sur le set de manifests
   proposé et confirmer l’absence de drift par rapport à
   `docs/source/soradns/deterministic_hosts.md`.
2. Générer un bundle de répertoire de resolvers :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Enregistrer l’ID du répertoire, le SHA-256 et les chemins de sortie imprimés dans
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et dans les minutes du kickoff.

### Capture de télémétrie DNS

- Tailer les logs de transparence des resolvers pendant ≥10 minutes avec
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporter les métriques Pushgateway et archiver les snapshots NDJSON à côté du
  répertoire du run ID.

### Drills d’automatisation du gateway

1. Exécuter la sonde TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Lancer le harness de conformité (`ci/check_sorafs_gateway_conformance.sh`) et
   l’helper self-cert (`scripts/sorafs_gateway_self_cert.sh`) pour rafraîchir le bundle
   d’attestations Norito.
3. Capturer les événements PagerDuty/Webhook pour prouver que la chaîne d’automatisation
   fonctionne de bout en bout.

### Packaging des preuves

- Mettre à jour `ops/drill-log.md` avec timestamps, participants et hashes de probe.
- Stocker les artefacts dans les répertoires de run ID et publier une synthèse exécutive
  dans les minutes Docs/DevRel.
- Lier le bundle de preuves dans le ticket de gouvernance avant la revue de kickoff.

## Animation de session et remise des preuves

- **Timeline du modérateur :**
  - T-24 h — Program Management poste le rappel + snapshot agenda/présence dans `#nexus-steering`.
  - T-2 h — Networking TL rafraîchit le snapshot télémétrie GAR et consigne les deltas dans `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation vérifie la préparation des probes et écrit le run ID actif dans `artifacts/sorafs_gateway_dns/current`.
  - Pendant l’appel — Le modérateur partage ce runbook et assigne un scribe en direct ; Docs/DevRel capturent les actions en ligne.
- **Template de minutes :** Copiez le squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (également miroir dans le bundle
  du portail) et commitez une instance remplie par session. Inclure la liste des
  participants, décisions, actions, hashes de preuves et risques ouverts.
- **Upload des preuves :** Zipper le répertoire `runbook_bundle/` du rehearsal,
  joindre le PDF de minutes rendu, enregistrer les hashes SHA-256 dans les minutes +
  agenda, puis pinguer l’alias des reviewers de gouvernance une fois les uploads
  disponibles dans `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot des preuves (kickoff mars 2025)

Les derniers artefacts rehearsal/live référencés dans la roadmap et les minutes
sont stockés dans le bucket `s3://sora-governance/sorafs/gateway_dns/`. Les hashes
ci-dessous reflètent le manifest canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Bundle tarball : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF des minutes : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier live — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload en attente : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel ajoutera le SHA-256 dès que le PDF rendu sera dans le bundle.)_

## Matériel connexe

- [Playbook d’opérations gateway](./operations-playbook.md)
- [Plan d’observabilité SoraFS](./observability-plan.md)
- [Tracker DNS décentralisé & gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
