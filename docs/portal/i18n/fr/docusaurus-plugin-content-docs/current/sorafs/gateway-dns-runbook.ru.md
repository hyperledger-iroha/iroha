---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска Gateway et DNS SoraFS

Cette copie est disponible sur le portail du réseau canonique.
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Pour configurer les garde-corps de fonctionnement pour le travail DNS et passerelle décentralisés,
ce qui concerne les réseaux, les opérations et la documentation peut être utilisé
automatisations avant le coup d'envoi 2025-03.

## Область и livrables

- Connectez votre DNS (SF-4) et votre passerelle (SF-5) à la répétition de la détection.
  Dérivations d'hébergements, résolveurs de catalogue de réponses, automatismes TLS/GAR et
  сбора доказательств.
- Держать kickoff-артефакты (agenda, invitation, suivi des présences, instantané
  телеметрии GAR) синхронизированными с последними назначениями propriétaires.
- Ajouter des éléments du bundle audio pour les réviseurs de gouvernance : notes de version
  catalogue de résolveurs, de sondes de passerelle de connexion, de harnais de conformité et de câbles
  Docs/DevRel.

## Rôles et réponses| Flux de travail | Ответственности | Objets d'art à collectionner |
|------------|-----------------|--------------------------|
| Mise en réseau TL (pile DNS) | Prend en charge la détermination des plans d'hébergement, la publication des versions du répertoire RAD et la publication de tous les résolveurs télémétriques. | `artifacts/soradns_directory/<ts>/`, différences pour `docs/source/soradns/deterministic_hosts.md`, métadonnées RAD. |
| Responsable de l'automatisation des opérations (passerelle) | Sélectionnez les forets automatiquement TLS/ECH/GAR, installez `sorafs-gateway-probe` et utilisez les crochets PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonde JSON, téléchargée dans `ops/drill-log.md`. |
| Guilde d'assurance qualité et groupe de travail sur les outils | Achetez `ci/check_sorafs_gateway_conformance.sh`, recherchez les luminaires et archivez les bundles d'autocertification Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documents/DevRel | Rédigez le procès-verbal, rédigez la conception pré-lue + les annexes, publiez le résumé des preuves sur le portail. | Mise à jour `docs/source/sorafs_gateway_dns_design_*.md` et notes de déploiement. |

## Voies et prérequis

- Spécifications des hôtes de détection (`docs/source/soradns/deterministic_hosts.md`) et
  attestation de carte pour les résolveurs (`docs/source/soradns/resolver_attestation_directory.md`).
- Passerelle Artefact : manuel de l'opérateur, aides à l'automatisation TLS/ECH,
  conseils pour le mode direct et le flux de travail d'autocertification selon `docs/source/sorafs_gateway_*`.
- Outillage : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` et aides CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets : clé de connexion GAR, informations d'identification DNS/TLS ACME, clé de routage PagerDuty,
  Jeton d'authentification Torii pour récupérer les résolveurs.

## Liste de contrôle avant le vol1. Mettre à jour les tâches et l'agenda, en cours
   `docs/source/sorafs_gateway_dns_design_attendance.md` et le téléphone de votre pays
   ordre du jour (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Ajouter des objets d'art à base de maïs, par exemple
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` et
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Créer des appareils (manifestes GAR, preuves RAD, passerelle de conformité des bundles) et
   убедитесь, что состояние `git submodule` sоответствует последнему tag de répétition.
4. Vérifiez les secrets (clé de version Ed25519, fichier de compte ACME, jeton PagerDuty) et
   Il s'agit des sommes de contrôle dans le coffre-fort.
5. Vérifier les cibles de télémétrie des tests de fumée (point final Pushgateway, carte GAR Grafana)
   перед perceuse.

## Répétition automatique

### Détermination de la carte d'hébergement et du catalogue de versions RAD

1. Запустите helper детерминированной деривации хостов на предложенном наборе
   manifeste et подтвердите отсутствие dérive относительно
   `docs/source/soradns/deterministic_hosts.md`.
2. Créer un catalogue de résolveurs :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Téléchargez le catalogue d'identification approprié, SHA-256 et votre appareil
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et dans les minutes de coup d'envoi.

### DNS des services téléphoniques

- Journaux de transparence du résolveur de queue en technologie ≥10 minutes
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporter les mesures Pushgateway et archiver les instantanés NDJSON directement vers
  директориями exécuter ID.

### Passerelle automatique des forets

1. Sélectionnez la sonde TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. Installez le harnais de conformité (`ci/check_sorafs_gateway_conformance.sh`) et
   assistant d'auto-certification (`scripts/sorafs_gateway_self_cert.sh`) pour la mise à jour
   Ensemble d'attestations Norito.
3. Sélectionnez PagerDuty/Webhook pour gérer le travail de bout en bout
   пути автоматизации.

### Упаковка доказательств

- Consultez `ops/drill-log.md` avec les horodatages, les données et les sondes de hachage.
- Sélectionnez les éléments d'art dans les répertoires d'exécution et ouvrez le résumé.
  в minutes Docs/DevRel.
- Examinez l'ensemble des preuves dans le cadre de la gouvernance lors de l'examen de lancement.

## Modération des sessions et présentation des documents- **Modérateur de la chronologie :**
  - T-24 h — Gestion du programme, publication des informations + agenda instantané/présence dans `#nexus-steering`.
  - T-2 h — Networking TL met à jour les instantanés télémétriques GAR et fixe les deltas dans `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation vérifie la qualité des sondes et affiche l'ID d'exécution actif dans `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор делится этим ранбуком и назначает live scribe ; Docs/DevRel définit les éléments d'action à ce jour.
- **Шаблон minutes:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (disponible dans le bundle de portail)
  и коммитьте заполненный экземпляр на каждую сессию. Включите список участников,
  résolution, éléments d'action, preuves de hachage et risques d'ouverture.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из répétition,
  Vous pouvez ajouter des minutes PDF supplémentaires, ajouter des hachages SHA-256 dans les minutes + l'agenda,
  затем уведомите gouvernance reviewer alias после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (coup d'envoi de mars 2025)

Après la répétition/les œuvres en direct, ajoutées à la feuille de route et aux minutes, placées dans le seau
`s3://sora-governance/sorafs/gateway_dns/`. Ce n'est pas un sujet canonique
manifeste (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Exécution à sec — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Pack tarball : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Procès-verbal PDF : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier en direct — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Téléchargement en attente : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel ajoute SHA-256 après la publication du PDF dans le bundle.)_

## Matériaux suisses

- [Livre de jeu des opérations pour la passerelle](./operations-playbook.md)
- [Plan de démarrage SoraFS](./observability-plan.md)
- [Трекер децентраLISованного DNS и gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)