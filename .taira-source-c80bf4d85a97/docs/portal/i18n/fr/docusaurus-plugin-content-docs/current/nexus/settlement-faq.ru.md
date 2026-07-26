---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : FAQ sur le règlement
description: Ответы для операторов о маршрутизации règlement, конвертации в XOR, телеметрии и аудиторских доказательствах.
---

Cette page contient la FAQ sur le règlement (`docs/source/nexus_settlement_faq.md`), qui sur le portail vous permet de trouver vos recommandations sans dépôt en mono-repo. Nous savons que Settlement Router prend en charge vos plates-formes, que les mesures sont exploitées et que le SDK peut intégrer les éléments les plus récents. Norito.

## Moments agréables

1. **Voie de sélection** — Cet espace de données correspond à `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consultez le catalogue actuel lane dans `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion de service** — Le routeur permet de régler votre compte dans XOR selon les licences d'installation, en effectuant une mise à jour inversée. La voie privée est occupée par les supermarchés XOR ; les coupes de cheveux sont principalement des tampons pour les avant-premières politiques.
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации и датчики coupe de cheveux. Les vols sont envoyés dans `dashboards/grafana/nexus_settlement.json` et les alertes dans `dashboards/alerts/nexus_audit_rules.yml`.
4. **Доказательства** — archivez les configurations, les logiciels de routeur, les téléphones d'exportation et les services d'audio.
5. **Détails du SDK** — Le SDK permet de préparer le règlement des données, la voie des identifiants et les charges utiles de codage Norito, afin de configurer les paramètres de chaque partition. routeur.

## Exemples de photos| Voie de type | Какие доказательства собрать | Qu'est-ce que cela signifie |
|-----------|----------|----------------|
| Privé `xor_hosted_custody` | Routeur d'enregistrement + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les magasins CBDC proposent des produits XOR, les coupes de cheveux étant en vigueur dans la politique précédente. |
| Publique `xor_global` | Routeur de journaux + connexion DEX/TWAP + mesures de latence/conversion | Il y a une opportunité gratuite pour TWAP avec une nouvelle coupe de cheveux. |
| Gibraltar `xor_dual_fund` | Routeur de journalisation, configuration publique vs blindée + télémétrie | Il s'agit d'une mise en place d'un salon de coiffure protégé/public et d'une coupe de cheveux précise pour chaque personne. |

## Vous avez besoin de plus de détails ?

- FAQ française : `docs/source/nexus_settlement_faq.md`
- Spécifications du routeur de règlement : `docs/source/settlement_router.md`
- Bureau politique CBDC : `docs/source/cbdc_lane_playbook.md`
- Fonctionnement du Runbook : [Prise en charge Nexus](./nexus-operations)