---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflete `docs/source/sns/address_checksum_failure_runbook.md`. Mettez a jour le fichier source d'abord, puis synchronisez cette copie.
:::

Les echecs de checksum apparaissent comme `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) dans Torii, les SDK et les clients wallet/explorer. Les items de roadmap ADDR-6/ADDR-7 exigent maintenant que les operateurs suivent ce runbook quand des alertes de checksum ou des tickets de support se declenchent.

## Quand lancer le play

- **Alertes:** `AddressInvalidRatioSlo` (defini dans `dashboards/alerts/address_ingest_rules.yml`) se declenche et les annotations listent `reason="ERR_CHECKSUM_MISMATCH"`.
- **Derive de fixtures:** Le textfile Prometheus `account_address_fixture_status` ou le dashboard Grafana signale un checksum mismatch pour une copie de SDK.
- **Escalades support:** Les equipes wallet/explorer/SDK citent des erreurs de checksum, une corruption IME ou des scans du presse-papiers qui ne decodent plus.
- **Observation manuelle:** Les logs Torii montrent de maniere repetee `address_parse_error=checksum_mismatch` pour les endpoints de production.

Si l'incident concerne specifiquement les collisions Local-8/Local-12, suivez plutot les playbooks `AddressLocal8Resurgence` ou `AddressLocal12Collision`.

## Checklist d'evidence

| Preuve | Commande / Emplacement | Notes |
|--------|-------------------------|-------|
| Snapshot Grafana | `dashboards/grafana/address_ingest.json` | Capture la repartition des raisons invalides et les endpoints touches. |
| Payload d'alerte | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Inclure les labels de contexte et les horodatages. |
| Sante des fixtures | `artifacts/account_fixture/address_fixture.prom` + Grafana | Prouve si les copies SDK ont derive de `fixtures/account/address_vectors.json`. |
| Requete PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Exporter le CSV pour le doc d'incident. |
| Logs | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (ou agregation de logs) | Nettoyer les PII avant partage. |
| Verification du fixture | `cargo xtask address-vectors --verify` | Confirme que le generateur canonique et le JSON committe concordent. |
| Controle parite SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Lancer pour chaque SDK cite dans les alertes/tickets. |
| Sanite presse-papiers/IME | `iroha address inspect <literal>` | Detecte les caracteres invisibles ou les reecritures IME; citer `address_display_guidelines.md`. |

## Reponse immediate

1. Accuser l'alerte, lier les snapshots Grafana + la sortie PromQL dans le fil d'incident, et noter les contextes Torii affectes.
2. Geler les promotions de manifest / releases SDK qui touchent le parsing d'adresses.
3. Sauvegarder les snapshots de dashboard et les artefacts textfile Prometheus dans le dossier d'incident (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. Extraire des echantillons de logs montrant les payloads `checksum_mismatch`.
5. Notifier les owners SDK (`#sdk-parity`) avec des payloads d'exemple pour qu'ils puissent trier.

## Isolation de la cause racine

### Derive de fixture ou du generateur

- Relancer `cargo xtask address-vectors --verify`; regenerer si echec.
- Executer `ci/account_fixture_metrics.sh` (ou `scripts/account_fixture_helper.py check` individuellement) pour chaque SDK et confirmer que les fixtures inclus correspondent au JSON canonique.

### Regressions encodeurs client / IME

- Inspecter les literaux fournis par les utilisateurs via `iroha address inspect` pour trouver des joins a largeur zero, des conversions kana ou des payloads tronques.
- Verifier les flux wallet/explorer avec `docs/source/sns/address_display_guidelines.md` (objectifs de copie double, avertissements, helpers QR) pour s'assurer qu'ils suivent l'UX approuvee.

### Problemes de manifest ou de registre

- Suivre `address_manifest_ops.md` pour revalider le dernier manifest bundle et s'assurer qu'aucun selecteur Local-8 n'est revenu.

### Trafic malveillant ou malforme

- Analyser les IPs/app IDs en cause via les logs Torii et `torii_http_requests_total`.
- Conserver au moins 24 heures de logs pour le suivi Security/Governance.

## Mitigation et reprise

| Scenario | Actions |
|----------|---------|
| Derive de fixture | Regenerer `fixtures/account/address_vectors.json`, relancer `cargo xtask address-vectors --verify`, mettre a jour les bundles SDK, et joindre des snapshots `address_fixture.prom` au ticket. |
| Regression SDK/client | Ouvrir des issues avec le fixture canonique + la sortie de `iroha address inspect`, et bloquer les releases via la CI de parite SDK (ex.: `ci/check_address_normalize.sh`). |
| Corruption de manifest | Reconstruire le manifest via `address_manifest_ops.md`, relancer `cargo xtask address-manifest verify`, et garder `torii.strict_addresses=true` jusqu'a ce que la telemetrie redevienne saine. |
| Soumissions malveillantes | Appliquer du rate-limit ou bloquer les principals fautifs, escalader a Governance si un tombstone de selecteurs est requis. |

Une fois les mitigations appliquees, relancer la requete PromQL ci-dessus pour confirmer que `ERR_CHECKSUM_MISMATCH` reste a zero (hors `/tests/*`) pendant au moins 30 minutes avant de degrader l'incident.

## Cloture

1. Archiver les snapshots Grafana, le CSV PromQL, les extraits de logs, et `address_fixture.prom`.
2. Mettre a jour `status.md` (section ADDR) ainsi que la ligne du roadmap si les outils/docs ont change.
3. Ajouter des notes post-incident sous `docs/source/sns/incidents/` quand de nouvelles lecons emergent.
4. S'assurer que les notes de release SDK mentionnent les correctifs de checksum quand applicable.
5. Confirmer que l'alerte reste verte pendant 24h et que les checks de fixture restent verts avant de clore.
