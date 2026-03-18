---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS et DNS et DNS

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) ہے۔
یہ DNS et passerelle décentralisés
نیٹ ورکنگ، آپس، اور ڈاکیومنٹیشن 2025-03 کے کک آف سے پہلے آٹومیشن اسٹیک کی
مشقیق کر سکیں۔

## اسکوپ اور ڈلیوریبلز

- Jalons du DNS (SF-4) et de la passerelle (SF-5) pour la dérivation déterministe de l'hôte
  versions du répertoire de résolution, automatisation TLS/GAR, capture de preuves et capture d'écran
- کک آف ان پٹس (agenda, invitation, suivi des présences, instantané de télémétrie GAR) کو
  تازہ ترین affectations du propriétaire کے ساتھ ہم آہنگ رکھنا۔
- Le bundle d'artefacts du groupe d'artefacts est le répertoire du résolveur : répertoire du résolveur
  notes de version, journaux de sonde de passerelle, sortie du faisceau de conformité, et résumé Docs/DevRel

## رولز اور ذمہ داریاں| ورک اسٹریم | ذمہ داریاں | artefacts مطلوبہ |
|------------|------------|--------|
| Mise en réseau TL (pile DNS) | plan d'hôte déterministe pour les versions du répertoire RAD et les entrées de télémétrie du résolveur | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` et différences et métadonnées RAD |
| Responsable de l'automatisation des opérations (passerelle) | Forets d'automatisation TLS/ECH/GAR pour `sorafs-gateway-probe` pour crochets PagerDuty | `artifacts/sorafs_gateway_probe/<ts>/`, sonde JSON, entrées `ops/drill-log.md`۔ |
| Guilde d'assurance qualité et groupe de travail sur les outils | `ci/check_sorafs_gateway_conformance.sh` Luminaires et luminaires Norito Ensembles d'autocertification Pièces détachées | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documents/DevRel | minutes ریکارڈ کرنا، conception pré-lecture + annexes اپڈیٹ کرنا، اور اسی پورٹل میں résumé des preuves شائع کرنا۔ | Voir `docs/source/sorafs_gateway_dns_design_*.md` et notes de déploiement |

## ان پٹس اور پری ریکوائرمنٹس

- spécification d'hôte déterministe (`docs/source/soradns/deterministic_hosts.md`) et résolveur
  échafaudage d’attestation (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefacts de passerelle : manuel de l'opérateur, aides à l'automatisation TLS/ECH, guidage en mode direct,
  Flux de travail d'autocertification selon `docs/source/sorafs_gateway_*` pour plus de détails
- Outillage : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, pour les assistants CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets : clé de version GAR, informations d'identification DNS/TLS ACME, clé de routage PagerDuty,
  Le résolveur récupère le jeton d'authentification Torii

## پری فلائٹ چیک لسٹ1. `docs/source/sorafs_gateway_dns_design_attendance.md` اپڈیٹ کر کے شرکاء اور agenda
   کنفرم کریں اور موجودہ agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`)
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ici
   `artifacts/soradns_directory/<YYYYMMDD>/` Racines d'artefacts تیار کریں۔
3. luminaires (manifestes GAR, preuves RAD, ensembles de conformité de passerelle)
   Il s'agit d'une étiquette de répétition `git submodule` pour un tag de répétition.
4. secrets (clé de version Ed25519, fichier de compte ACME, jeton PagerDuty) et
   sommes de contrôle du coffre-fort
5. percer des cibles de télémétrie (point final Pushgateway, carte GAR Grafana) et tester la fumée

## آٹومیشن étapes de répétition

### carte d'hôte déterministe et version du répertoire RAD

1. L'outil de dérivation d'hôte manifeste un assistant de dérivation déterministe d'hôte.
   Le système `docs/source/soradns/deterministic_hosts.md` est un système de dérive de l'eau
2. bundle de répertoires de résolution à propos :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Utilisez l'ID de répertoire, SHA-256 et les chemins de sortie.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et minutes du coup d'envoi

### Capture de télémétrie DNS

- Journaux de transparence du résolveur ≥10 minutes de la queue :
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Les métriques Pushgateway exportent des instantanés NDJSON et un répertoire d'ID d'exécution ainsi que des instantanés NDJSON et des répertoires d'identification d'exécution.

### Exercices d'automatisation de la passerelle

1. Sonde TLS/ECH ici :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. harnais de conformité (`ci/check_sorafs_gateway_conformance.sh`) et assistant d'auto-certification
   (`scripts/sorafs_gateway_self_cert.sh`) Ensemble d'attestations Norito blanc
3. Les événements PagerDuty/Webhook capturent les fichiers de bout en bout.

### Emballage de preuves

- `ops/drill-log.md` pour les horodatages des participants et les hachages de sonde
- exécuter les répertoires d'identification et les artefacts ainsi que les minutes Docs/DevRel et le résumé exécutif
- examen du lancement du ticket de gouvernance et de l'ensemble de preuves

## سیشن فیسلیٹیشن اور transfert de preuves- **Chronologie du modérateur :**
  - T-24 h — Gestion du programme `#nexus-steering` Rappel + instantané de l'agenda/présence پوسٹ کرے۔
  - T-2 h — Instantané de télémétrie Networking TL GAR par `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` pour les deltas
  - T-15 m — Préparation de la sonde Ops Automation et l'ID d'exécution est `artifacts/sorafs_gateway_dns/current`.
  - کال کے دوران — Modérateur یہ رن بک شیئر کرے اور live scribe assign کرے؛ Éléments d'action en ligne Docs/DevRel
- **Modèle de procès-verbal :**
  `docs/source/sorafs_gateway_dns_design_minutes.md` pour un squelette de projet (ensemble de portails pour un projet de validation d'instance) liste des participants, décisions, mesures à prendre, hachages de preuves, risques en suspens
- **Téléchargement des preuves :** répétition du répertoire `runbook_bundle/` et zip des minutes rendues PDF en pièce jointe des minutes + ordre du jour et hachages SHA-256 et téléchargements et alias du réviseur de gouvernance. ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## Aperçu des preuves (coup d'envoi de mars 2025)

feuille de route et minutes میں حوالہ دیے گئے تازہ ترین répétition/live artefacts
`s3://sora-governance/sorafs/gateway_dns/` godet blanc Il existe de nombreux hachages
manifeste canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) et reflète le manifeste canonique- **Exécution à sec — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - archive tar du pack : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Procès-verbal PDF : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier en direct — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Téléchargement en attente : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF pour SHA-256 شامل کرے گا۔)_

## Matériel connexe

- [Livre de jeu des opérations de passerelle](./operations-playbook.md)
- [Plan d'observabilité SoraFS](./observability-plan.md)
- [Traqueur DNS et passerelle décentralisé] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)