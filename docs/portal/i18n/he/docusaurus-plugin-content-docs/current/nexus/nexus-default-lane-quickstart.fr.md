---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-default-lane-quickstart
כותרת: Guide rapide de la lane par defaut (NX-5)
sidebar_label: Guide rapide de la lane par defaut
תיאור: קונפיגורטור ומאמת את ה-fallback de lane par defaut de Nexus afin que Torii et les SDK puissent omettre lane_id sur les lanes publiques.
---

:::הערה מקור קנוניק
Cette עמוד reprend `docs/source/quickstart/default_lane.md`. Gardez les deux copies alignees jusqu'a ce que le balayage de localization arrive sur le portail.
:::

# Guide rapide de lane par defaut (NX-5)

> **מפת הדרכים של הקשר:** NX-5 - אינטגרציה של נתיב פרסום רגיל. לחשוף את זמן הריצה של `nexus.routing_policy.default_lane` ניתן למצוא נקודות קצה REST/gRPC de Torii et chaque SDK puissent omettre en toute securite un `lane_id` lorsque le trafic publiquent a la laque. Ce guide accompagne les operators pour configurer le catalogue, verifier le fallback dans `/status` et tester le comportement client de bout en bout.

## תנאי מוקדם

- Un build Sora/Nexus de `irohad` (לנסר `irohad --sora --config ...`).
- Un acces au depot de configuration pour pouvoir modifier les sections `nexus.*`.
- `iroha_cli` הגדר את ה-Pur parler au Cible Cible.
- `curl`/`jq` (או שווה ערך) pour inspecter le payload `/status` de Torii.

## 1. קבע את הקטלוג של נתיבים ומרחבי נתונים

Declarez les lanes and dataspaces qui doivent exister sur le reseau. L'extrait ci-dessous (tire de `defaults/nexus/config.toml`) נרשום את trois lanes publiques ainsi que les alias de dataspace correspondants:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Chaque `index` doit etre unique et contigu. Les ids de dataspace sont des valeurs 64 סיביות; les exemples ci-dessus utilisent les memes valeurs numeriques que les index de lane pour plus de clarte.

## 2. Definir les valeurs par defaut de routage et les surcharges optionnelles

הסעיף `nexus.routing_policy` מפקח על נתיב ה-fallback ו-permet de surcharger le routage pour des specifiques des prefixes ou des prefixes de compte. אם אתה מתכתב, מסלול מתזמן לטרנזקציה לעומת הגדרות `default_lane` ו-`default_dataspace`. La logique du router vit dans `crates/iroha_core/src/queue/router.rs` et applique la politique de maniere transparente aux משטחים REST/gRPC de Torii.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Lorsque vous ajoutez plus tard de nouvelles lanes, mettez d'abord a jour le catalogue, puis etendez les regles de routage. Lane de fallback doit continuer a pointer vers lane publicque qui porte la majorite du trafic utilisateur afin que les SDK herites continunt de fonctionner.

## 3. Demarrer un noeud avec la politique appliquee

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le noeud journalise la politique de routage derivee au demarrage. Toute erreur de validation (אינדקס, alias dupliques, IDs de dataspace invalides) est remontee avant le debut du gossip.

## 4. Confirmer l'etat de governance de la laneUne fois le noeud en ligne, utilisez l'helper CLI pour verifier que la lane par defaut est scellee (אישום ברור) et prete pour le trafic. La vue recap affiche une ligne par lane :

```bash
iroha_cli app nexus lane-report --summary
```

פלט לדוגמה:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si lane par defaut affiche `sealed`, suivez le runbook de governance des lanes avant d'autoriser du trafic externe. Le flag `--fail-on-sealed` est utile pour la CI.

## 5. Inspecter les payloads de statut de Torii

La reponse `/status` לחשוף את la politique de routage ainsi que l'instantane du scheduler par lane. השתמשו ב-`curl`/`jq` כדי לאשר את הערכים של הגדרות ברירת מחדל ומוודא למוצר ה-fallback של טלמטריה:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

פלט לדוגמה:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Pour inspecter les compteurs live du scheduler pour la lane `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Cela confirme que l'instantane TEU, les metadonnees d'alias et les indicateurs de manifest s'alignent avec la configuration. Le meme payload est use par les panneaux Grafana pour le dashboard d'egestion des lanes.

## 6. Tester les valeurs par defaut des clients

- **Rust/CLI.** `iroha_cli` et le crate client Rust omettent le champ `lane_id` lorsque vous ne passez pas `--lane-id` / `LaneSelector`. Le router de queue retombe donc sur `default_lane`. Utilisez les flags מפורשת `--lane-id`/`--dataspace-id` ייחודיות lorsque vous ciblez une lane non par defaut.
- **JS/Swift/Android.** Les dernieres versions des SDK traitent `laneId`/`lane_id` comme optionnels et retombent sur la valeur annoncee par `/status`. Gardez la politique de routage Syncee entre staging and production afin que les apps mobiles n'aient pas besoin de reconfigurations d'urgence.
- **בדיקות צינור/SSE.** Les filtres d'evenements de transactions acceptent les predicats `tx_lane_id == <u32>` (voir `docs/source/pipeline.md`). Abonnez-vous a `/v2/pipeline/events/transactions` עם מסנן לשפוך que les ecritures שליחים sans ליין הגיע מפורש sos l'id de lane de fallback.

## 7. Observabilite et points d'accroche de governance

- `/status` publie aussi `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` afin qu'Alertmanager puisse avertir lorsqu'une lane perd son manifest. Gardez ces alertes actives meme sur les devnets.
- La carte de telemetrie du scheduler et le dashboard de governance des lanes (`dashboards/grafana/nexus_lanes.json`) משתתפים les champs alias/slug du catalogue. Si vous renommez un alias, reetiquetez les repertoires Kura correspondants afin que les auditeurs conservent des chemins deterministes (suivi sous NX-1).
- Les approbations parlementaires pour les lanes par defaut doivent כוללים un plan de rollback. Enregistrez le hash du manifest et les preuves de governance a cote de ce quickstart dans votre runbook operator afin que les rotations futures n'aient pas a deviner l'etat requis.ללא צורך באימותים מסתיימים, אתה יכול לבחור את הקוד `nexus.routing_policy.default_lane` במקור אמיתי עבור תצורה של SDK ומתחיל לנטרל את ה-chemins de code mono-lane herites sur le reseau.