---
lang: he
direction: rtl
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Reflete `docs/source/da/replication_policy.md`. Gardez les deux versions en
:::

# Politique de Replication זמינות נתונים (DA-4)

_Statut: En cours -- אחראים: Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingest DA applique maintenance des objectifs de retention
deterministes pour chaque classe de blob decrite dans `roadmap.md` (זרם עבודה
DA-4). Torii refuse de persister des envelopes de retention fournis par le
מתקשר qui ne כתב pas a la politique configuree, garantissant que
chaque noeud validateur/stockage reient le nombre requis d'epoques et de
העתקים ללא תלות a l'intention de l'emetteur.

## פוליטיקה שוויונית

| Classe de blob | שימור חם | שמירה על קור | רפליקות דרישות | Classe de stockage | Tag de gouvernance |
|--------------|----------------|----------------|------------------|------------------------|--------------------|
| `taikai_segment` | 24 שעות | 14 יומנים | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 יומנים | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 היורה | 180 יו"ם | 3 | `cold` | `da.governance` |
| _ברירת מחדל (מחלקות toutes les autres)_ | 6 heures | 30 ימים | 3 | `warm` | `da.default` |

Ces valeurs sont integrees dans `torii.da_ingest.replication_policy` et appliquees
מסר להגשות `/v2/da/ingest`. Torii שחזר את המניפסטים עם הרשימה
פרופיל דה שימור להטיל et emet un adertissement quand les callers fournissent
des valeurs incoherentes afin que les operators detectent des SDKs מיושן.

### Classes de disponibilite Taikai

Les manifests de routage Taikai (`taikai.trm`) מוכרז une `availability_class`
(`hot`, `warm`, או `cold`). Torii applique la politique correspondante avant le
chunking afin que les operators puissent ajuster les comptes de replicas par
stream sans editor la table globale. ברירות מחדל:

| Classe de disponibilite | שימור חם | שמירה על קור | רפליקות דרישות | Classe de stockage | Tag de gouvernance |
|------------------------|---------------|----------------|------------------------|------------------------|--------------------|
| `hot` | 24 שעות | 14 יומנים | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 ימים | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 היורה | 180 יו"ם | 3 | `cold` | `da.taikai.archive` |

Les hints manquants reviennent a `hot` afin que les שידורים חיים retiennent la
פוליטיקה לה פלוס פורטה. ברירת המחדל של Remplacez les via
`torii.da_ingest.replication_policy.taikai_availability` אני רוצה להשתמש
des cibles differentes.

## תצורה

La politique vit sous `torii.da_ingest.replication_policy` ו-expose un template
*ברירת מחדל* בתוספת un tableau d'overrides par classe. Les identifiants de classe sont
לא רגיש לאותיות גדולות ומקובל `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` pour des extensions approuvees par la
ממשל. Les classes de stockage acceptable `hot`, `warm`, ou `cold`.```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Laissez le block intact pour utiliser les defaults ci-dessus. יוצקים durcir une
classe, mettez a jour l'override כתב; pour changer la base pour de
class classes, editez `default_retention`.

Les classes de disponibilite Taikai peuvent etre surcharges independamment via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## סמנטיקה לאכיפה

- Torii remplace le `RetentionPolicy` fourni par l'utilisateur par le profil
  להטיל Avant le chunking ou l'emission de manifest.
- Les manifests preconstruits qui declarent un profil de retention שונה
  sont rejetes avec `400 schema mismatch` afin que les clients מיושן ne
  puissent pas affaiblir le contrat.
- Chaque evenement d'override est log (`blob_class`, מדיניות soumise לעומת attendue)
  pour mettre en evidence les callers non conformes pendant le rollout.

Voir [Data Availability Ingest Plan](ingest-plan.md) (רשימת אימות) לשפוך
le gate mis a jour couvrant l'enforcement de retention.

## זרימת עבודה של שכפול מחדש (Suivi DA-4)

L'enforcement de retention n'est que la premiere etape. Les Operators doivent
aussi prouver que les manifests live et les ordres de replication restent
alignes avec la politique configuree afin que SoraFS puisse re-repliquer les blobs
hors conformite automatiquement.

1. **Surveillez le drift.** Torii emet
   `overriding DA retention policy to match configured network baseline` quand
   un caller soumet des valeurs de retention מיושן. Associez ce log avec la
   telemetrie `torii_sorafs_replication_*` pour reperer des shortfalls de replicas
   ou des reployments retardes.
2. **הכוונה שונה לעומת העתקים חיים.** Utilisez le nouvel helper d'audit:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande charge `torii.da_ingest.replication_policy` depuis la config
   fournie, decode chaque manifest (JSON ou Norito), et associe optionnlement
   les loads `ReplicationOrderV1` par digest de manifest. אות קורות החיים
   תנאי deux:

   - `policy_mismatch` - le profil de retention du manifest diverge du profil
     להטיל (ceci ne devrait jamais arriver sauf si Torii est mal configuration).
   - `replica_shortfall` - l'ordre de replication live demande moins de replicas
     que `RetentionPolicy.required_replicas` או fournit moins d'assignations que
     sa cible.

   Un status de sortie non zero indique une insuffisance active afin que
   l'automatization CI/on-call מיידית של חיתוך puisse. Joignez le rapport JSON
   au paquet `docs/examples/da_manifest_review_template.md` pour les votes du
   פרלמנט.
3. **Declenchez la replication.** Quand l'audit signale une suffisance,
   emettez un nouveau `ReplicationOrderV1` via les outils de governance decrits
   dans [שוק קיבולת אחסון SoraFS](../sorafs/storage-capacity-marketplace.md)
   et relancez l'audit jusqu'a convergence du set de replicas. יוצקים les overrides
   d'urgence, associez la sortie CLI avec `iroha app da prove-availability` afin que
   les SREs puissent referencer le meme digest et la preuve PDP.La couverture de regression vit dans `integration_tests/tests/da/replication_policy.rs`;
la suite soumet une politique de retention non conforme a `/v2/da/ingest` ואימות
que le manifest recupere לחשוף le profil להטיל plutot que l'intention du מתקשר.