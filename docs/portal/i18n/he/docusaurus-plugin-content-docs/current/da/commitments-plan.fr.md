---
lang: he
direction: rtl
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Reflete `docs/source/da/commitments_plan.md`. גרסאות גרדז לסינכרון
:::

# Plan des engagements זמינות נתונים Sora Nexus (DA-3)

_Redige: 2026-03-25 -- אחראים: Core Protocol WG / צוות חוזה חכם / צוות אחסון_

DA-3 etend le format de bloc Nexus pour que chaque lane integre des enregistrements
deterministes decrivant les blobs מקבל את תקן DA-2. Cette הערות לכידת les
structures de donnees canoniques, les hooks du pipeline de blocs, les preuves de
client leger, et les surfaces Torii/RPC qui doivent arriver avant que les
validateurs puissent s'appuyer sur les engagements DA lors des checks
d'admission ou de governance. Tous les payloads sont codes en Norito; pas de
SCALE ב-JSON אד-הוק.

## אובייקטים

- Porter des engagements par blob (שורש נתח + חשיש מניפסט + התחייבות KZG
  optionnel) dans chaque bloc Nexus afin que les pairs puissent reconstruire
  l'etat d'availability sans consulter le Stockage Hors Ledger.
- Fournir des preuves de membership deterministes afin que les clients legers
  אימות qu'un manifest hash a ete finalize dans un bloc donne.
- Exposer des requetes Torii (`/v1/da/commitments/*`) et des preuves permettant
  ממסרי עזר, ערכות SDK ואוטומציות של ניהול הביקורת לזמינות
  sans rejouer chaque blok.
- Conserver l'enveloppe `SignedBlockWire` canonique en transmettant les nouvelles
  מבנים באמצעות le header de metadata Norito et la derivation du hash de bloc.

## Vue d'ensemble du scope

1. **Ajouts au data model** ב-`iroha_data_model::da::commitment` plus
   שינויים בכותרת דה בלוק ב-`iroha_data_model::block`.
2. **Hooks d'executor** pour que `iroha_core` ingeste les receipts DA emis par
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **התמדה/מדדים** afin que le WSV reponde rapidement aux requetes de
   התחייבויות (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** pour les endpoints de list/הרצאה/הוכחה
   `/v1/da/commitments`.
5. **בדיקות אינטגרציה + מתקנים** תקף פריסת תיל ו-le flux de proof
   dans `integration_tests/tests/da/commitments.rs`.

## 1. מודל הנתונים של Ajouts au

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` השתמש מחדש בנקודה 48 אוקטטים ב-`iroha_crypto::kzg`.
  Quand il est נעדר, על retombe sur des preuves מרקל ייחודיות.
- `proof_scheme` נובעים מקטלוג הנתיבים; les lanes Merkle דוחה les
  מטענים KZG tandis que les lanes `kzg_bls12_381` דרושות התחייבויות KZG
  לא נולים. Torii ne produit actuellement que des התחייבויות Merkle et rejette
  les lanes configurees en KZG.
- `KzgCommitment` השתמש מחדש בנקודה 48 אוקטטים ב-`iroha_crypto::kzg`.
  Quand il est נעדר sur les lanes Merkle על retombe sur des preuves Merkle
  ייחודיות.
- `proof_digest` anticipe l'integration DA-5 PDP/PoTR afin que le meme record
  למנות את לוח הזמנים של דגימה לנצל את התחזוקה של הכתמים.

### 1.2 הרחבה du header de bloc

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```Le hash du bundle entre a la fois dans le hash de bloc et dans la metadata de
`SignedBlockWire`. Quand un bloc ne transporte pas de donnees DA, le champ reste

הערה ליישום: `BlockPayload` et le `BlockBuilder` חשיפה שקופה
Maintenant des setters/getters `da_commitments` (voir
`BlockBuilder::set_da_commitments` et `SignedBlock::set_da_commitments`), donc
les hosts peuvent attacher un bundle preconstruit avant de sceller un bloc. טוס
les helpers laissent le champ a `None` tant que Torii ne fournit pas de bundles
סלילים.

### 1.3 חוט קידוד

- `SignedBlockWire::canonical_wire()` ajoute le header Norito pour
  `DaCommitmentBundle` מיידי לפני רשימת העסקאות הקיימות.
  Le byte de version est `0x01`.
- `SignedBlockWire::decode_wire()` rejette les bundles dont `version` est
  inconnue, en ligne avec la politique Norito decrite dans `norito.md`.
- Les mises a jour de derivation du hash vivent uniquement dans `block::Hasher`;
  les clients legers qui decodent le wire format existant gagnent le nouveau
  champ automatiquement car le header Norito מודעה נוכחות.

## 2. Flux de production de blocks

1. L'ingest DA Torii finalize un `DaIngestReceipt` et le publie sur la queue
   אינטרנט (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` לאסוף כל קבלות שלא מתאימות `lane_id`
   en construction, en dedupliquant par `(lane_id, client_blob_id,
   manifest_hash)`.
3. Juste avant le scellage, le builder trye les commitments par `(lane_id, epoch,
   sequence)` pour garder le hash deterministe, coded le bundle avec le codec
   Norito, et met a jour `da_commitments_hash`.
4. Le bundle complet est stocke dans le WSV et emis avec le bloc dans
   `SignedBlockWire`.

Si la creation du bloc echoue, les receipts restent dans la queue pour que la
prochaine tentative les reprenne; le Builder רשם את le dernier `sequence`
כולל par lane afin d'eviter les attaques de replay.

## 3. RPC משטח ובקשות

Torii חושפים את נקודות הקצה של הטרואס:

| מסלול | מתודה | מטען | הערות |
|-------|--------|--------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (מסנן טווח נתיב/תקופה/רצף, עימוד) | Renvoie `DaCommitmentPage` עם סך הכל, התחייבויות ו-hash de bloc. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (מסלול + מניפסט hash ou tuple `(epoch, sequence)`). | Repond avec `DaCommitmentProof` (תקליט + chemin Merkle + hash de bloc). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | עוזר חסר אזרחות qui rejoue le calcul du hash de bloc et valide l'inclusion; השתמש ב-SDKs qui ne peuvent pas lier direction `iroha_crypto`. |

Tous les payloads vivent sous `iroha_data_model::da::commitment`. Les routers
Torii montent les handlers a cote des endpoints d'ingest DA existants pour
שימוש מחדש ב-les politiques token/mTLS.

## 4. Preuves d'inclusion and clients legers- Le producteur de bloc construit un arbre Merkle binaire sur la list
  serialisee des `DaCommitmentRecord`. La racine alimente `da_commitments_hash`.
- `DaCommitmentProof` emballe le record cible plus un vecteur de
  `(sibling_hash, position)` afin que les verificateurs puissent reconstruire la
  racine. Les preuves incluent aussi le hash de bloc et le header signe pour que
  les clients legers puissent verifier la finalite.
- Les helpers CLI (`iroha_cli app da prove-commitment`) מחזור מקיף
  בקשה/אמת וחשיפה של מיונים Norito/hex pour les operators.

## 5. אחסון ואינדקס

Le WSV stocke les Commitments dans une column family dediee, cle par
`manifest_hash`. Des indexes secondaires couvrent `(lane_id, epoch)` et
`(lane_id, sequence)` afin que les requetes eviten de scanner des bundles
משלים. חליפת צ'אק תקליט la hauteur de bloc qui l'a scelle, permettant aux
nouds en rattrapage de reconstruire l'index rapidement a partir du block log.

## 6. Telemetrie et observabilite

- `torii_da_commitments_total` incremente des qu'un bloc scelle au moins un
  להקליט.
- `torii_da_commitment_queue_depth` חליפת קבלות
  (נתיב נקוב).
- לוח המחוונים Grafana `dashboards/grafana/da_commitments.json` להמחיש
  l'inclusion de blocs, la profondeur de queue, et le throughput de preuve pour
  que les gates de release DA-3 puissent auditer le comportement.

## 7. אסטרטגיית בדיקות

1. **מבחן יחידות** pour l'encoding/פענוח de `DaCommitmentBundle` et les
   mises a jour de derivation de hash de bloc.
2. **מתקן זהוב** sous `fixtures/da/commitments/` לכידת לסיביות
   canoniques du bundle et les preuves Merkle.
3. **מבדקי אינטגרציה** demarrant deux validateurs, ingestant des blobs de
   test, et verifiant que les deux noeuds concordent sur le contenu du bundle et
   les reponses de query/proof.
4. **בודק קל-לקוח** ב-`integration_tests/tests/da/commitments.rs`
   (Rust) qui appellent `/prove` et verifient la preuve sans parler a Torii.
5. **עשן CLI** avec `scripts/da/check_commitments.sh` pour garder l'outillage
   ניתן לשחזור מפעיל.

## 8. תוכנית השקה

| שלב | תיאור | קריטריוני יציאה |
|-------|-------------|------------|
| P0 - מיזוג מודל נתונים | אינטגרר `DaCommitmentRecord`, חסר לך כותרת קודקים Norito. | `cargo test -p iroha_data_model` מתקנים vert avec nouvelles. |
| P1 - ליבת חיווט/WSV | לוגיקה של תור + בונה בלוקים, מתמידים באינדקסים וחושפים את המטפלים RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` נוסעים עם הוכחות חבילה. |
| P2 - מפעיל כלי עבודה | ספר עוזרי CLI, לוח מחוונים Grafana, et mises a jour des docs de verification de proof. | `iroha_cli app da prove-commitment` fonctionne sur devnet; le dashboard affiche des donnees בשידור חי. |
| P3 - שער שלטון | Activer le validateur de blocs requerant les התחייבויות DA sur les lanes signalees dans `iroha_config::nexus`. | כניסה לסטטוס + עדכון מפת הדרכים של DA-3 comme TERMINE. |

## שאלות אוברטיות1. **ברירת המחדל של KZG נגד מרקל** - המשך פעולות מתעלם מההתחייבויות KZG pour
   les petits blobs afin de reduire la taille des blocs? הצעה: גארדר
   `kzg_commitment` optionnel et le gater דרך `iroha_config::da.enable_kzg`.
2. **פערים ברצף** - Autorise-t-on des lanes hors order? Le plan actuel rejette
   les gaps sauf si la governance active `allow_sequence_skips` pour un replay
   דחוף.
3. **מטמון קל לקוח** - L'equipe SDK דורש מטמון SQLite leger pour les
   הוכחות; suivi en attente sous DA-8.

ענה על שאלות ב-PRs d'implementation fera passer DA-3 de
BROUILLON (ce document) a EN COURS des que le travail de code מתחילים.