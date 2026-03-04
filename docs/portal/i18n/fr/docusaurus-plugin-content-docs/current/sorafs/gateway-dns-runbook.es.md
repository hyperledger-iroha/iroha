---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de kickoff de Gateway et DNS de SoraFS

Cette copie du portail reflète le runbook canonique en
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Reconnaître les sauvegardes opérationnelles du flux de travail de DNS et de la passerelle décentralisée
pour que les responsables du réseautage, des opérations et de la documentation puissent analyser la pile de
Automatisation avant le coup d'envoi 2025-03.

## Alcance et entregables

- Vincular les hitos de DNS (SF-4) et gateway (SF-5) en essayant la dérivation
  déterministe des hôtes, versions du répertoire des résolveurs, automatisation TLS/GAR
  et capture de preuves.
- Gérer les informations du coup d'envoi (agenda, invitation, tracker d'assistance, instantané)
  de telemetría GAR) synchronisé avec les dernières attributions des propriétaires.
- Produire un bundle de artefactos auditables para revisores de gobernanza: notas de
  libération du répertoire des résolveurs, journaux des sondes de la passerelle, sortie du harnais
  de conformité et le résumé de Docs/DevRel.

## Rôles et responsabilités| Flux de travail | Responsabilités | Artefacts requis |
|------------|---------|-------------|
| Mise en réseau TL (pile DNS) | Maintenir le plan déterministe des hôtes, exécuter les versions du directeur RAD, publier les entrées de télémétrie des résolveurs. | `artifacts/soradns_directory/<ts>/`, différences de `docs/source/soradns/deterministic_hosts.md`, métadonnées RAD. |
| Responsable de l'automatisation des opérations (passerelle) | Exécuter des exercices d'automatisation TLS/ECH/GAR, corriger `sorafs-gateway-probe`, actualiser les crochets de PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonde JSON entrée en `ops/drill-log.md`. |
| Guilde d'assurance qualité et groupe de travail sur les outils | Exécuter `ci/check_sorafs_gateway_conformance.sh`, curar luminaires, archive bundles de self-cert Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documents/DevRel | Enregistrez les minutes, actualisez la pré-lecture du dessin + les annexes et publiez le résumé des preuves sur ce portail. | Fichiers actualisés `docs/source/sorafs_gateway_dns_design_*.md` et notes de déploiement. |

## Entrées et prérequis- Spécification des hôtes déterministes (`docs/source/soradns/deterministic_hosts.md`) et
  l'andamiaje de attestación de résolveurs (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefacts del gateway : manuel de l'opérateur, aides à l'automatisation TLS/ECH,
  Guide de mode direct et flux d'auto-certification sous `docs/source/sorafs_gateway_*`.
- Outillage : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, et aides de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets : clé de version GAR, informations d'identification ACME DNS/TLS, clé de routage de PagerDuty,
  jeton d'authentification de Torii pour obtenir des résolveurs.

## Liste de contrôle pré-vuelo

1. Confirmer les assistants et l'actualisation de l'agenda
   `docs/source/sorafs_gateway_dns_design_attendance.md` et faire circuler l'agenda
   vigente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Préparer les variétés d'objets comme
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` et
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Refresca luminaires (manifestes GAR, pruebas RAD, bundles de conformidad del gateway) et
   assurez-vous que l'état de `git submodule` coïncide avec la dernière étiquette d'analyse.
4. Vérifier les secrets (clé de version Ed25519, fichier ACME, jeton de PagerDuty)
   et qui coïncide avec les sommes de contrôle du coffre-fort.
5. Test de fumée des cibles de télémétrie (point final de Pushgateway, tableau GAR Grafana)
   avant l'exercice.

## Étapes d'essai d'automatisation

### Carte déterminant les hôtes et la version du répertoire RAD1. Exécuter l'assistant de dérivation déterministe des hôtes contre l'ensemble des manifestes
   propose et confirme qu'il n'y a pas de dérive par rapport à
   `docs/source/soradns/deterministic_hosts.md`.
2. Générez un bundle de répertoire de résolveurs :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Enregistrez l'ID du directeur, le SHA-256 et les routes de sortie imprimées à l'intérieur de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et dans les minutes du coup d'envoi.

### Capture de télémétrie DNS

- Il y a des journaux de transparence des résolveurs pendant ≥10 minutes d'utilisation
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporter les paramètres de Pushgateway et archiver les instantanés NDJSON conjointement avec
  répertoire de l'ID d'exécution.

### Exercices d'automatisation de la passerelle

1. Éjectez la sonde TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Exécutez le harnais de conformité (`ci/check_sorafs_gateway_conformance.sh`) et
   l'aide d'auto-certification (`scripts/sorafs_gateway_self_cert.sh`) pour rafraîchir
   le paquet d'attestations Norito.
3. Capturez les événements de PagerDuty/Webhook pour démontrer l'automatisation
   funciona de extremo à extremo.

### Empaqueté de preuves

- Actualisation `ops/drill-log.md` avec horodatages, participants et hachages de sondes.
- Gardez les artefacts sous les directeurs de run ID et publiez un CV ejecutivo
  dans les minutes de Docs/DevRel.
- Ajouter le paquet de preuves sur le ticket de gouvernement avant la révision
  du coup d'envoi.

## Facilitation de la session et échange de preuves- **Ligne de temps du modérateur :**
  - T-24 h — Gestion du programme publica el recordatorio + snapshot de agenda/asistencia en `#nexus-steering`.
  - T-2 h — Networking TL rafraîchit l'instantané de télémétrie GAR et enregistre les deltas sur `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation vérifie la préparation des sondes et écrit l'ID d'exécution activé en `artifacts/sorafs_gateway_dns/current`.
  - Durante la llamada — Le modérateur partage ce runbook et attribue un écrit en vivo ; Docs/DevRel capture les éléments d'action en ligne.
- **Plante de minutes :** Copie du squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (également observé dans le bundle
  du portail) et comitea une instance terminée par session. Inclut la liste de
  assistants, décisions, éléments d'action, hachages de preuves et risques pendants.
- **Carga de evidencias:** Comprenez le répertoire `runbook_bundle/` del ensayo,
  ajouter le PDF des minutes rendues, enregistrer les hachages SHA-256 dans les minutes +
  ordre du jour et notification aux alias des réviseurs d'État lorsqu'ils sont chargés
  aterricen en `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot des preuves (coup d'envoi de mars 2025)

Les derniers artefacts d’analyse/production référencés dans la feuille de route et les minutes
viven en el bucket `s3://sora-governance/sorafs/gateway_dns/`. Les hachages en bas
reflejan el manifest canónico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Exécution à sec — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball du bundle : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF des minutes : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier en vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Charger pendant : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel annexe le SHA-256 lorsque le PDF est rendu dans le bundle.)_

## Matériel lié

- [Playbook des opérations de la passerelle](./operations-playbook.md)
- [Plan d'observabilité de SoraFS](./observability-plan.md)
- [Tracker de DNS décentralisé et passerelle](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)