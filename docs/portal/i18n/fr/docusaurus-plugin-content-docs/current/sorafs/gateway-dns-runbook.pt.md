---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de kickoff de Gateway et DNS par SoraFS

Cette copie du portail s'affiche ou du runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Elle capture les garde-fous opérationnels du flux de travail du DNS décentralisé et de la passerelle
pour que les responsables des réseaux, des opérations et de la documentation puissent étudier le pilha de
automatisation avant le coup d'envoi 2025-03.

## Escopo et entregaveis

- Connecter les repères DNS (SF-4) et la passerelle (SF-5) en ensaiando dérivation
  détermination des hôtes, versions du répertoire des résolveurs, automatisation TLS/GAR
  et la capture des preuves.
- Manter os insumos do kickoff (agenda, convite, tracker de presenca, snapshot de
  telemetria GAR) synchronisé avec les derniers attributs des propriétaires.
- Produire un ensemble d'articles audités pour les réviseurs de gouvernance : notes de
  libération du répertoire des résolveurs, journaux de sonde de la passerelle, dit du harnais de
  conforme et le curriculum vitae de Docs/DevRel.

## Papeis et responsabilités| Flux de travail | Responsabilités | Artefatos requis |
|------------|---------|----------------------|
| Mise en réseau TL (pile DNS) | Manœuvrez au plan déterministe des hôtes, exécutez les versions du répertoire RAD, publiez les entrées de télémétrie des résolveurs. | `artifacts/soradns_directory/<ts>/`, différences de `docs/source/soradns/deterministic_hosts.md`, métadonnées RAD. |
| Responsable de l'automatisation des opérations (passerelle) | Exécuter des exercices d'automatisation TLS/ECH/GAR, rodar `sorafs-gateway-probe`, actualiser les crochets de PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonde JSON, entrée `ops/drill-log.md`. |
| Guilde d'assurance qualité et groupe de travail sur les outils | Rodar `ci/check_sorafs_gateway_conformance.sh`, luminaires curar, bundles arquivar de self-cert Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documents/DevRel | Capturer les minutes, actualiser la pré-lecture de la conception + les annexes, et publier le résumé des preuves sur ce portail. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` actualisés et notes de déploiement. |

## Entrées et pré-requis- Spécification des hôtes déterministes (`docs/source/soradns/deterministic_hosts.md`) et
  o échafaudage de atestacao de résolveurs (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos de gateway: manuel de l'opérateur, aides d'automatisation TLS/ECH,
  conseils en mode direct et flux de travail d'auto-certification dans `docs/source/sorafs_gateway_*`.
- Outillage : `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, et aides de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos : clé de version GAR, informations d'identification ACME DNS/TLS, clé de routage de PagerDuty,
  le jeton d'authentification du Torii pour récupérer les résolveurs.

## Liste de contrôle avant le vol

1. Confirmer les participants et l'agenda actualisé
   `docs/source/sorafs_gateway_dns_design_attendance.md` et diffuser un ordre du jour
   réel (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Préparez les raizes de artefatos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` f
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Actualiser les luminaires (manifestes GAR, preuves RAD, bundles de conformité do gateway) et
   garanta que l'état de `git submodule` est alinhado à la dernière étiquette d'ensaio.
4. Vérifiez séparément (avec la version Ed25519, stocké avec ACME, jeton de PagerDuty)
   et se batem avec les sommes de contrôle du coffre-fort.
5. Faça smoke-test nos cibles de télémétrie (point final Pushgateway, carte GAR Grafana)
   les antes font des exercices.

## Étapes d'ensaio de automatisation

### Carte déterministe des hôtes et de la version du répertoire RAD1. Aide à la dérivation déterministe des hôtes par rapport à l'ensemble des manifestes
   propose et confirme que nao ha dérive em relation a
   `docs/source/soradns/deterministic_hosts.md`.
2. Consultez le répertoire des résolveurs :

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Enregistrez l'ID du répertoire, le SHA-256 et les chemins dits imprimés à l'intérieur de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` et quelques minutes du coup d'envoi.

### Capture de télémétrie DNS

- Faça tail dos logs de transparence des résolveurs por ≥10 minutes d'utilisation
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporter les métriques de Pushgateway et archiver les instantanés du système d'exploitation NDJSON à ce sujet
  répertoire d'exécution de l'ID.

### Exercices d'automatisation de la passerelle

1. Exécutez la sonde TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Rode ou harnais de conformité (`ci/check_sorafs_gateway_conformance.sh`) et
   l'assistant d'auto-certification (`scripts/sorafs_gateway_self_cert.sh`) pour actualiser
   le bundle de certificats Norito.
3. Capturez les événements de PagerDuty/Webhook pour vérifier le chemin d'automatisation
   funciona de ponta a ponta.

### Empacotamento de evidencias

- Actualisez `ops/drill-log.md` avec les horodatages, les participants et les hachages des sondes.
- Armazene artefatos nos diretorios de run ID e publique um curriculum vitae
  pendant les minutes de Docs/DevRel.
- Lien vers le paquet de preuves sans ticket de gouvernance avant la révision du coup d'envoi.

## Facilitation de la session et du transfert des preuves- **Linha do tempo do modérateur :**
  - T-24 h — Gestion du programme posta o lembrete + snapshot de agenda/presenca em `#nexus-steering`.
  - T-2 h — Networking TL actualise l'instantané de télémétrie GAR et enregistre les deltas dans `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation vérifie la présence des sondes et enregistre l'ID d'exécution sur `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada — Le modérateur partage ce runbook et désigne un écrit ao vivo ; Docs/DevRel capture les éléments de cacao en ligne.
- **Modèle de minutes :** Copie du squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (tambem espelhado no bundle
  do portal) et comite une instance preenchida por sessao. Inclut la liste de
  participants, décisions, éléments de cacao, hachages de preuves et risques pendants.
- **Télécharger les preuves :** Zip ou le répertoire `runbook_bundle/` de l'enregistrement,
  annexe ou PDF des minutes rendues, enregistrement des hachages SHA-256 pendant les minutes +
  ordre du jour, et après avise ou alias des réviseurs de gouvernance lors des téléchargements
  chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot des preuves (coup d'envoi du mois de mars 2025)

Les derniers artefatos de ensaio/live referenciados no roadmap e nas minutos
ficam pas de seau `s3://sora-governance/sorafs/gateway_dns/`. Os hachages abaixo
espelham ou manifeste canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Exécution à sec — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Bundle Tarball : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF des minutes : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier ao vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Télécharger en attente : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel annexe au SHA-256 lorsque le rendu PDF est téléchargé vers le bundle.)_

## Matériel lié

- [Le playbook des opérations fait la passerelle](./operations-playbook.md)
- [Plan d'observation du SoraFS](./observability-plan.md)
- [Tracker de DNS décentralisé et passerelle](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)