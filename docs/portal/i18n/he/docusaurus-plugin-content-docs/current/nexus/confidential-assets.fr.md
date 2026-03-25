---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Actifs confidentiels et transferts ZK
תיאור: Blueprint Phase C pour la blindee circulation, les registres et les controls operators.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design des actifs confidentiels and transferts ZK

## מוטיבציה
- Livrer des flux d actifs blindes opt-in afin que les domaines preservant la privacy transactionnelle sans modifier la circulation transparente.
- אבחון ביצועים של חומרה הטרוגנית של validateurs ו-conserver Norito/Kotodama ABI v1.
- Donner aux auditeurs et operateurs des controls de cycle de vie (הפעלה, סיבוב, ביטול) יוצקים מעגלים ופרמטרים קריפטוגרפיים.

## מודל האיום
- Les validateurs sont כנים-אך-סקרנים: ils executent le consensus fidelement mais tentent d inspecter book/state.
- Les observateurs reseau voient les donnees de bloc et transactions gossipees; aucune hypothese de canaux de prives של רכילות.
- היקף השפעות: ניתוח התנועה מחוץ לפנקס החשבונות, קוונטים של יריבים (שלוחים על מפת הדרכים PQ), תקיפות של ניהול חשבונות.

## Vue d ensemble du design
- Les actifs peuvent declarer un *בריכה מוגנת* en plus des balances transparentes existantes; la circulation blindee est representee via des commitments cryptographiques.
- Les notes encapsulent `(asset_id, amount, recipient_view_key, blinding, rho)` avec:
  - התחייבות: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - מבטל: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, עצמאי של הזמנה של הערות.
  - שיפר מטען: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Les transactions transportent des payloads `ConfidentialTransfer` מקודדת תוכן Norito:
  - ציבורים של תשומות: עוגן מרקל, ביטולים, התחייבויות חדשות, מזהה נכס, גרסה דה מעגל.
  - מטענים שיפורים יוצקים נמענים ואפשרויות ביקורת.
  - קודם אפס ידע המוכיח שימור ערך, בעלות ואישור.
- המפתחות המאמתים ואנסמבלים של הפרמטרים מתבצעים באמצעות רישומים על ספר החשבונות עם הפעלת הגדרות; les noeuds refusent de valider des proofs qui referencent des entrees inconnues ou revoquees.
- Les headers de consensus engagent le digest de fonctionnalite confidentielle actif afin que les blocs soient accepts seulement si l etat הרישום/פרמטרים מתאימות.
- La construction des proofs משתמשת ב-one pile Halo2 (Plonkish) ללא התקנה מהימנה; Groth16 או autres variantes SNARK לא תומכים ב-v1.

### מתקנים דטרמיניסטים

Les enveloppes de memo confidentiel livrent desormais un fixture canonique a `fixtures/confidential/encrypted_payload_v1.json`. ללכידת מערך הנתונים היא מעטפת v1 חיובית בתוספת פגמים של ה-Echantillons על מנת לאשר את הניתוח של SDK. Les tests du data-model Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) et la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) chargent direction le fixture, garantissant que l קידוד Norito, les surfaces d erreur et la couverture de regression pendant restencalignant.Les SDKs Swift peuvent Maintenant Emettre des הוראות מגן ללא דבק JSON בהזמנה אישית: Construire un
`ShieldRequest` עם התחייבות של 32-בתים, מטען מטען ו-metadata debit,
puis appeler `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) pour signer et relayer la
עסקה באמצעות `/v1/pipeline/transactions`. Le helper valide les longueurs de commitment,
הכנס את `ConfidentialEncryptedPayload` לקודד Norito, ופריסה מחדש `zk::Shield`
ci-dessous afin que les ארנקים restent סינכרון עם חלודה.

## התחייבויות קונצנזוס ועריכת יכולת
- Les headers de blocs exposent `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; le digest participe au hash de consensus et doit egaler la vue locale du registry pour accepter un bloc.
- La governance peut mettre en scene des שדרוגים בתוכנית `next_conf_features` avec un `activation_height` עתיד; Jusqu a cette hauteur, les producteurs de blocs doivent continuer a mettre le digest תקדים.
- Les noeuds validateurs DOIVENT fonctionner avec `confidential.enabled = true` et `assume_valid = false`. Les checks de demarrage refusent l entree dans le set de validateurs si l une des conditions echoue ou si le `conf_features` מקומית מתפצלת.
- מטא נתונים של לחיצת יד P2P כולל `{ enabled, assume_valid, conf_features }`. Les peers annoncant des features non prises en charge sont rejetes avec `HandshakeConfidentialMismatch` et n entrent jamais dans la rotation de consensus.
- התוצאות של לחיצת יד אומתנים, משקיפים ועמיתים נלכדים בבסיס לחיצת יד [משא ומתן על יכולת צומת](#node-capability-negotiation). Les echec de לחיצת יד חשופה `HandshakeConfidentialMismatch` et gardent le peer hors de la rotation de consensus jusqu a ce que son digest corresponde.
- Les observers non validateurs peuvent definir `assume_valid = true`; ils appliquent les deltas confidentiels a l aveugle mais n influencent pas la securite du consensus.## Politiques d actifs
- Chaque definition d actif transporte un `AssetConfidentialPolicy` defini par le createur ou via governance:
  - `TransparentOnly`: מצב ברירת מחדל; seules les instruktioner transparentes (`MintAsset`, `TransferAsset`, וכו') sont permises et les operations shielded sont rejetees.
  - `ShieldedOnly`: הצגת פליטה והעברת הוראות שימוש חסוי; `RevealConfidential` est interdit pour que les balances ne soient jamais חושף פרסום.
  - `Convertible`: מחזיקי ה-Peuvent deplacer la valeur entre representations transparentes and shielded through les הוראות on/off-ramp ci-dessous.
- Les politiques suivent un FSM contraint pour eviter les fonds bloques:
  - `TransparentOnly -> Convertible` (הפעלה מיידית של בריכה מוגנת).
  - `TransparentOnly -> ShieldedOnly` (מחייב מעבר pendante et fenetre de conversion).
  - `Convertible -> ShieldedOnly` (הטלת מינימום דלאי).
  - `ShieldedOnly -> Convertible` (תוכנית ההגירה דורשת כל הערות ניתנות להשקעה בהשארת מוגן).
  - `ShieldedOnly -> TransparentOnly` est interdit sauf si le shielded pool est video or governance code une migration qui de-shield les restantes notes.
- Les הוראות הממשל fixent `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` באמצעות ISI `ScheduleConfidentialPolicyTransition` ו-Peuvent annuler des changements programs avec `CancelConfidentialPolicyTransition`. La validation mempool assure qu aucune transaction ne chevauche la hauteur de transition et l inclusion echoue deterministiquement si un check de politique changerait au milieu du bloc.
- Les transitions pendantes sont appliquees automatiquement a l ouverture d un nouveau bloc: une fois que la hauteur entre dans la fenetre de conversion (pour les upgrades `ShieldedOnly`) ou atteint `effective_height`, le runtime met a jour80X, rafi06000 met 10000000000, raff `zk.policy` et eface l תליון מנה ראשונה. אם יש אספקה ​​שקופה ללא שינוי, ומעבר `ShieldedOnly` מגיע, זמן הריצה מבטל את השינוי והלוגו ללא מניעת, תקדימי המצב שלם ללא פגע.
- כפתורי התצורה של `policy_transition_delay_blocks` et `policy_transition_window_blocks` ניתנים למינימום מוקדם ותקופות של חסד לשמירת ארנקים נוספים.
- `pending_transition.transition_id` sert aussi de handle d audit; la governance doit le citer lors de la finalization ou l ביטול afin que les operators puissent correler les rapports on/off-ramp.
- `policy_transition_window_blocks` ברירת מחדל a 720 (~12 הורות עם זמן חסימה של 60 שניות). Les noeuds clampent les requetes de governance qui tentent un preavis plus court.
- גילויי בראשית ושטף CLI חשופים לפוליטיקה ותלויות. La logique d admission lit la politique au moment de l execution pour confirmer que chaque instruction confidentielle est autorisee.
- Checklist de migration - voir "רצף הגירה" ci-dessous pour le plan d upgrade par etapes suivi par le Milestone M0.

#### ניטור של מעברים באמצעות Toriiארנקים et auditeurs interrogent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter l `AssetConfidentialPolicy` actif. Le payload JSON inclut toujours l asset id canonique, la derniere hauteur de bloc observee, le `current_mode` de la politique, le mode effectif a cette hauteur (les fenetres de conversion reportt temporairement `Convertible`), et de les identifiants attendus `vk_set_hash`/פוסידון/פדרסן. Quand une transition de governance est en attente la reponse embed aussi:

- `transition_id` - לטפל d audit renvoye par `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` et le `window_open_height` נובעים (הגוש או הארנקים מתחילים להמרה ל-ShieldedOnly).

דוגמה לתגובה:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Une reponse `404` indique qu aucune definition d actif correspondante n existe. Lorsqu aucune transition n est planifiee le champ `pending_transition` est `null`.

### Machine d etats de politique| מצב אקטואל | מצב תואם | תנאי מוקדם | Gestion de la hauteur יעיל | הערות |
|--------------------|----------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| שקוף בלבד | להמרה | ממשל פעיל Les entrees de registre de verificateur/parameters. Soumettre `ScheduleConfidentialPolicyTransition` avec `effective_height >= current_height + policy_transition_delay_blocks`. | La transition s לבצע דרישת `effective_height`; le shielded pool devient disponible מיידי.        | Chemin par defaut pour activer la confidentialite tout en gardant les flux transparents. |
| שקוף בלבד | ShieldedOnly | Idem ci-dessus, בתוספת `policy_transition_window_blocks >= 1`.                                                         | Le time runtime entre automatiquement en `Convertible` a `effective_height - policy_transition_window_blocks`; להעביר `ShieldedOnly` ל-`effective_height`. | Fenetre de conversion deterministe avant de deactiver les הוראות שקופות. |
| להמרה | ShieldedOnly | תוכנית מעבר עם `effective_height >= current_height + policy_transition_delay_blocks`. ממשל DEVRAIT מאשר (`transparent_supply == 0`) באמצעות metadata d audit; le runtime l applique au cut-over. | Semantique de fenetre identique. סי le אספקת שקוף est non nul a `effective_height`, la transition avorte avec `PolicyTransitionPrerequisiteFailed`. | Verrouille l actif en circulation entierement confidentielle.                             |
| ShieldedOnly | להמרה | תוכנית מעבר; aucun retrait d urgence actif (`withdraw_height` לא מוגדר).                                  | L etat bascule a `effective_height`; les ramps לחשוף rouvrent tandis que les הערות shielded restent valides.       | השתמש ב-pour fenetres de maintenance ou revues d auditeurs.                               |
| ShieldedOnly | שקוף בלבד | ממשל doit prouver `shielded_supply == 0` או מכין un plan `EmergencyUnshield` חתימה (חתימות המבקר נדרשות). | Le runtime ouvre une fenetre `Convertible` avant `effective_height`; a la hauteur, les הוראות חסויות echouent durement et l actif revient au mode שקוף בלבד. | תרופות מסוג Sortie de Dernier. ה-transition s auto-annule si une note confidentielle est depensee תליון la fenetre. |
| כל | כמו הנוכחי | `CancelConfidentialPolicyTransition` nettoie le changement en attente.                                              | `pending_transition` משוער לפרישה מיידית.                                                                       | שמירה על המצב הקיים; indique pour completude.                                         |Les transitions non listes ci-dessus sont rejetees Lors de la Soumission ממשל. זמן ריצה לאמת את התנאים המוקדמים רק אוונט d appliquer ותוכנית המעבר; en cas d echec, il repousse l actif au mode precedent et emet `PolicyTransitionPrerequisiteFailed` באמצעות טלמטריה ואירועים דה בלוק.

### רצף הגירה

1. **מכין את הרשומים:** פעילים יותר מפרסמים את המנות הראשונות אימות ופרמטרי ההתייחסות לפוליטיקה. Les noeuds annoncent le `conf_features` התוצאה pour que les peers לאמת את הקוהרנטיות.
2. **Planifier la transition:** soumettre `ScheduleConfidentialPolicyTransition` avec un `effective_height` respectant `policy_transition_delay_blocks`. En allant vers `ShieldedOnly`, יחידה מדויקת יותר של המרה (`window >= policy_transition_window_blocks`).
3. **מפרסם את מפעיל ההדרכה:** רושם את `transition_id` חזרה ותפזר עם ספר הפעלה/יציאה. ארנקים וביקורת נוכחים ב-`/v1/confidential/assets/{id}/transitions` pour connaitre la hauteur d ouverture de fenetre.
4. **יישום של חלון:** ליצירת קשר, זמן ריצה בסיסי לפוליטיקה ב-`Convertible`, אמט `PolicyTransitionWindowOpened { transition_id }`, והתחל בסירוב לדרישות ממשל.
5. **מסיים או מחליף:** `effective_height`, זמן ריצה מאמת את התנאים המוקדמים (אספקת אפס שקוף, דחיפות חוזרת וכו'). En success, la politique passe au mode demande; en echec, `PolicyTransitionPrerequisiteFailed` est emis, la transition pendante est nettoyee et la politique reste inchangee.
6. **שדרוגי סכימה:** לאחר המעבר מחדש, הממשל גדל בגרסה של סכימת האקטיב (בדוגמה `asset_definition.v2`) ו-CLI כלי עבודה נוסעים `confidential_policy` למידע על סדרת מניפסטים. Les docs d upgrade genesis instruisent les operators and ajouter les settings de politique et emreintes registry avant de redmarrer les validateurs.

Les nouveaux reseaux qui demarrent avec la confidentialite activee codent la politique desiree directement dans genesis. Ils suivent quand meme la checklist ci-dessus lors des changements לאחר ההשקה afin que les fenetres de conversion restent deterministes et que les wallets aient le temps de s ajuster.

### גרסה והפעלה של מניפסטים Norito- Les genesis manifests DOIVENT inclure un `SetParameter` pour la key custom `confidential_registry_root`. Le payload est un Norito כתב JSON a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omettre le champ (`null`) quand aucune entree n est active, sinon fournir une chaine hex 32-byte (`null`) quand aucune entree n est active, sinon fournir une chaine hex 32-byte (`null`) egale paruit hashX `compute_vk_set_hash` לפי הוראות ה-Verificateur du Manifest. Les noeuds refusent de demarrer si le parameter manque ou si le hash diverge des ecritures registry encodees.
- Le on-wire `ConfidentialFeatureDigest::conf_rules_version` embarque la version de layout du manifest. Pour les reseaux v1 il DOIT rester `Some(1)` et egaler `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Quand le Regels evolue, incrementer la constante, regenerer les manifests and deployer les binaires in lock-step; melanger les versions fait rejeter les blocs par les validateurs avec `ConfidentialFeatureDigestMismatch`.
- ההפעלה מציגה את הארגון מחדש של DEVRAIENT מפספס את תאריך הרישום, שינויים במחזוריות של פרמטרים ומעברים פוליטיים ומאפיינים את שאר הקוהרנטיים:
  1. אפליקציית מוטציות של רישום מתאימות (`Publish*`, `Set*Lifecycle`) ב-`compute_confidential_feature_digest` באופן לא מקוון וחישוב של פוסט ההפעלה.
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` en utilisant le hash calcule pour que les peers en retard recuperent le digest correct meme s ils ratent des instruktioner intermediairs.
  3. Ajouter les הוראות `ScheduleConfidentialPolicyTransition`. הוראת Chaque doit citer le `transition_id` emis par governance; les manifests qui l oblient seront rejetes par le runtime.
  4. מתמשך les bytes du manifest, une empreinte SHA-256 et le digest לנצל את הפעלת תוכנית ד. Les operators verifient les trois artefacts Avant de voter le manifest pour eviter les partitions.
- מספר ההשקה הדרושות ללא חיתוך שונה, רושם את העלות הבסיסית בהתאמה אישית של פרמטרים (בדוגמה `custom.confidential_upgrade_activation_height`). Cela fournit aux auditeurs une preuve Norito מקודד ee que les validateurs ont respecte la fenetre de preavis avant que le changement de digest prenne effet.## Cycle de vie des verificateurs et parameters
### רשם ZK
- Leedger stocke `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` או `proving_system` est actuellement fixe a `Halo2`.
- Les paires `(circuit_id, version)` sont globalment uniques; התחזוקה של הרישום ואינדקס משני לרישום מטה נתונים במעגל. Les tentatives d enregistrer un pare dupliquee sont rejetees lors de l admission.
- `circuit_id` doit etre non vide et `public_inputs_schema_hash` doit etre fourni (טיפוס ו-hash Blake2b-32 de l קידוד canonique des public inputs du verificateur). L admission rejette les enregistrements qui omettent ces champs.
- הוראות הממשל כוללות:
  - `PUBLISH` pour ajouter une entree `Proposed` avec metadata seulement.
  - `ACTIVATE { vk_id, activation_height }` לשפוך מתכנת l הפעלה לתקופה מוגבלת אחת.
  - `DEPRECATE { vk_id, deprecation_height }` pour marquer la hauteur finale ou des proofs peuvent referencer l entree.
  - `WITHDRAW { vk_id, withdraw_height }` לשפוך כיבוי דחוף; les actifs touches geleront les depenses confidentielles apres למשוך גובה jusqu א הפעלה de nouvelles מנות ראשונות.
- Les genesis manifests emettent automatiquement un parameter custom `confidential_registry_root` dont `vk_set_hash` תואם aux entrees actives; la validation croise ce digest avec l etat local du registry avant qu un noeud puisse rejoindre le consensus.
- הרשם או רשם את דרישתו של `gas_schedule_id`; la verification impose que l entree soit `Active`, presente dans l index `(circuit_id, version)`, et que les proofs Halo2 fournissent un `OpenVerifyEnvelope` dont `circuit_id`, I016NI000, et0116NI010, et016NI010, et כתב א l entree du registry.

### מפתחות הוכחה
- המפתחות המוכיחים את המפתחות מחוץ לפנקס החשבונות, ומספקים כתובות מזהות לתוכן (`pk_cid`, `pk_hash`, `pk_len`) מפרסמת את המטא נתונים של אימות.
- Les SDKs הארנק recuperent les PK, אימות של hashes, et les mettent and cache localement.

### פרמטרים של פדרסן ופוסידון
- Des registries separes (`PedersenParams`, `PoseidonParams`) mirrorrent les controls de cycle de vie des verificateurs, chacun avec `params_id`, hashes de genereurs/constantes, activering, deprecation withdraw ו-hauteur.
- Les commitments et hashes font une separation de domaine par `params_id` afin que la rotation de parameters ne reutilise jamais des bit patterns d ensembles depreces; אני מזהה est embarque dans les התחייבויות הערות ו les tags de domaine nullifier.
- Les מעגלים תומכים בבחירה מרובת פרמטרים a la אימות; ההרכבים של הפרמטרים מפחיתים את ההשקעה ב-Restent Jusqu a leur `deprecation_height`, ו-les ההרכבים פורשים ולא פוסלים את ה-`withdraw_height`.## סדר קבע ומבטל
- Chaque actif maintient un `CommitmentTree` avec `next_leaf_index`; les blocs ajoutent les התחייבויות dans un ordre deterministe: iterer les transactions dans l ordre de bloc; dans chaque עסקאות iterer les sorties shielded par `output_idx` serialise ascendant.
- `note_position` להפיק את קיזוז de l arbre mais **ne** fait pas partie du nullifier; il ne sert qu aux chemins de membership dans le witness de preuve.
- La stabilite des nullifiers sous reorgs est garantie par le design PRF; l קלט PRF lie `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, et les anchors referencent des Merkle roots historiques limites par `max_anchor_age_blocks`.

## זרימת ספר חשבונות
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requiert la politique d actif `Convertible` ou `ShieldedOnly`; אני מוודא את הכניסה ל-autorite, recupere `params_id`, echantillonne `rho`, Emet le מחויבות, פגשתי ב-jour l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` avec le nouveau commitment, delta de Merkle root, et hash d appel de transaction pour audit trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Le syscall VM לאמת את ההוכחה באמצעות l entree du registry; המארח להבטיח que les nullifiers sont inutilizes, que les התחייבויות sont ajoutes deterministiquement, et que l anchor est האחרון.
   - Le Ledger enregistre des entrees `NullifierSet`, stocke les payloads chiffres pour נמענים/מבקרים et emet `ConfidentialEvent::Transferred` res umant nullifiers, outputs ordonnes, hash de proof et Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - ייחודיות זמינה pour les actifs `Convertible`; la proof valide que la valeur de la note egale le montant revele, le ledger credite le balance transparent et brule la note shielded en marquant le nullifier comme depense.
   - Emet `ConfidentialEvent::Unshielded` avec le montant public, nullifiers consommes, identifiers de proof et hash d appel de transaction.## מודל הנתונים של Ajouts au
- `ConfidentialConfig` (נוuvelle section de config) עם הפעלת דגל ד, `assume_valid`, כפתורי גז/מגבלות, עוגן קצה, קצה אחורי של אימות.
- סכימות `ConfidentialNote`, `ConfidentialTransfer`, et `ConfidentialMint` Norito עם byte de version מפורש (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` enveloppe des memo bytes AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, par defaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour le layout XChaCha20-Poly1305.
- Les vecteurs canoniques de derivation de cle vivent dans `docs/source/confidential_key_vectors.json`; le CLI et l נקודת קצה Torii מתקנים רגרסנטיים.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste le binding `(backend, name, commitment)` pour les verifiers transfer/unshield; ביצוע דחיית ההוכחות אינו מאפשר אימות מפתח או מובנה ולא מתכתב או נרשם מחויבות.
- `CommitmentTree` (par actif avec frontier checkpoints), `NullifierSet` cle `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` es en world state stock.
- Mempool maintient des structures transitoires `NullifierIndex` et `AnchorIndex` pour detection precoce des doublens et checks d age d anchor.
- עדכוני הסכמה Norito כוללים הזמנה קנונית של תשומות ציבוריות; הבדיקות של הקידוד הלוך ושוב בטוחות לקידוד הדטרמיניזם.
- Less roundtrips d מטען מוצפן sont verrouilles באמצעות בדיקות יחידה (`crates/iroha_data_model/src/confidential.rs`). Des vecteurs wallet de suivi attacheront des transcripts AEAD canoniques pour auditeurs. `norito.md` תיעוד לכותרת על גבי המעטפה.

## אינטגרציה IVM et syscall
- Introduire le syscall `VERIFY_CONFIDENTIAL_PROOF` מקבל:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, et le `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` התוצאה.
  - Le syscall charge la metadata verificateur depuis le registry, applique des limites de taille/temps, facture un gas deterministe, et n applique le delta que si la proof reussit.
- לחשוף את המארח ללא תכונה לקריאה בלבד `ConfidentialLedger` להחלים את תמונות המצב Merkle root et le statut des nullifiers; la librairie Kotodama fournit des helpers d assembly de witness et de validation de schema.
- Les docs pointer-ABI לא יכול להבהיר את הפריסה של ה-buffer de proof ו-les מטפל ברישום.## משא ומתן עצום
- הודעת לחיצת היד `feature_bits.confidential` avec `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participation des validateurs דורשים `confidential.enabled=true`, `assume_valid=false`, זיהויים של זיהויים אחוריים ומאומתים מתכתבים; les mismatches echouent le handshake avec `HandshakeConfidentialMismatch`.
- La config supporte `assume_valid` pour les observers seulement: quand desactive, rencontrer des instruktioner confidentielles produit `UnsupportedInstruction` deterministe sans panic; quand active, les observers appliquent les deltas מצהיר sans verifier les proofs.
- Mempool דחתה את עסקאות סודיות עבור המקום היכולות להפסיק. Les filtres de gossip evitent d avoyer des transactions shielded aux peers sans capacites correspondantes tout en relayant a l aveugle des Verifier IDs inconnus dans les limites de taille.

### Matrice de לחיצת יד

| מודעה רחוקה | תוצאות עבור validateurs | מפעיל הערות |
|----------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, התאמה עורפית, התאמה לעיכול | קבל | יש להתייחס עמיתים ל-`Ready` ולהשתתף בהצעה, או הצבעה, או מעריצים RBC. Aucune action manuelle requise. |
| `enabled=true`, `assume_valid=false`, התאמה עורפית, תקציר מעופש או נעדר | Rejete (`HandshakeConfidentialMismatch`) | האפליקציה מרחוק להפעיל את הרישום/פרמטרים של הפעלות ב-`activation_height`. Tant que corrige, le noeud reste decouvrable mais n entre jamais en rotation de consensus. |
| `enabled=true`, `assume_valid=true` | Rejete (`HandshakeConfidentialMismatch`) | Les validateurs דרושים לה אימות הוכחות; configurer le מרוחק comme observer avec ingress Torii רק ou basculer `assume_valid=false` לפני שההפעלה של האימות הושלמה. |
| `enabled=false`, champs omis (מבנה מיושן), או backend verificateur שונה | Rejete (`HandshakeConfidentialMismatch`) | עמיתים מיושנים או שדרוגים חלקיים ne peuvent pas rejoindre le reseau de consensus. Mettez a jour vers la release courante et assurez que le tuple backend + digest corresponde avant de reconnecter. |

Les observers qui omettent volontairement la Verification de proofs ne doivent pas ouvrir de connexions consensus contre les validateurs actifs. Ils peuvent toujours ingester des blocs via Torii ou des APIs d archivage, mais le reseau de consensus les rejette jusqu a ce qu ils annoncent des capacites correspondantes.

### Politique de pruning Reveal et retention des nullifiers

Les Ledgers Confidentiels doivent conserver assez d historique pour prouver la fraicheur des notes et rejouer des audits governance. La politique par defaut, appliquee par `ConfidentialLedger`, משוער:- **Retention des nullifiers:** conserver les nullifiers depenses pour un *minimum* de `730` jours (24 mois) apres la hauteur de depense, ou la fenetre imposee par le regulateur si plus longue. Les operateurs peuvent etendre la fenetre via `confidential.retention.nullifier_days`. Les nullifiers plus jeunes que la fenetre DOIVENT rester interrogeables via Torii afin que les auditeurs prouvent l absence de double-spend.
- **Pruning des reveals:** les מגלה שקופים (`RevealConfidential`) prunent les התחייבויות שותפים מיידי לאחר סיום הבלוק, mais le nullifier consomme reste soumis a la regle de retention ci-dessus. Les events `ConfidentialEvent::Unshielded` נרשם le montant public, le recipient, et le hash de proof pour que la reconstruction des reveals historiques ne requiere pas le ciphertext prune.
- **מחסומי גבולות:** les frontiers de commitment maintiennent des checkpoints roulants couvrant le plus grand de `max_anchor_age_blocks` et de la fenetre de retention. Les noeuds compactent les checkpoints plus anciens seulement apres expiration de tous les nullifiers dans l intervalle.
- **עיכוב תיקון מעופש:** si `HandshakeConfidentialMismatch` survient a cause d un drift de digest, les operateurs doivent (1) verifier que les fenetres de retention des nullifiers sont alignees dans le cluster, (2) lancer Sumeragi po digenerll en regenerll retenus, et (3) redeployer le manifest rafraichi. Tout nullifier prune trop tot doit etre restaure depuis le stockage froid avant de rejoindre le reseau.

Documentez les עוקף את locaux dans le runbook d operations; les politiques de governance qui etendent la fenetre de retention doivent mettre a jour la configuration des noeuds et les plans de stockage d archivage en lockstep.

### זרימת פינוי והחלמה

1. תליון חיוג, `IrohaNetwork` השווה פרסומות. חוסר התאמה לרמה `HandshakeConfidentialMismatch`; la connexion est fermee et le peer reste dans la file de discovery sans etre promu a `Ready`.
2. לחשוף דרך Le Log du Service Reseau (כולל שלט עיכול ו-backend), ו-Sumeragi לא מתכנן להצביע על הצעה.
3. Les operateurs remedient en alignant les registries verificateur et ensembles de paramets (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) או תוכנת `next_conf_features`500 un I102u. Une fois le digest aligne, le prochain לחיצת יד חוזרת לשימוש אוטומטי.
4. אם אתה משתמש מחדש ב-Peer Stale, מפיץ ללא גוש (לדוגמה דרך ארכיון חוזר), les validateurs le rejettent deterministiquement avec `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, Gardant l etat du Ledger coherent dans le reseau.

### זרימת לחיצת יד בטוחה בהפעלה חוזרת1. Chaque tentative sortante alloue un nouveau materiel de cle Noise/X25519. Le payload de handshake signe (`handshake_signature_payload`) concatene les cles publiques ephimeres locale et distante, l adresse socket annoncee encodee Norito, et - quand compile avec `handshake_chain_id` - l identifiant. Le message est chiffre AEAD avant de quitter le noeud.
2. Le responder recompute le payload avec l order de cles peer/local inverse et verifié la signature Ed25519 embarquee dans `HandshakeHelloV1`. Parce que les deux cles ephimeres et l adresse annoncee font partie du domaine de signature, rejouer un capture message contre un autre peer ou recuperer une connexion stale echoue deterministiquement.
3. Les flags de capacite confidentielle et le `ConfidentialFeatureDigest` voyagent dans `HandshakeConfidentialMeta`. Le recepteur השווה le tuple `{ enabled, assume_valid, verifier_backend, digest }` בן `ConfidentialHandshakeCaps` מקומי; tout mismatch sort avec `HandshakeConfidentialMismatch` avant que le transport ne pass a `Ready`.
4. המפעילים DOIVENT מחשבים מחדש את ה-digest (דרך `compute_confidential_feature_digest`) ו-Remarrer les nouds avec registries/politiques misses a jour avant de reconnecter. Les peers annoncant des digests anciens continunt d echouer le לחיצת יד, empechant un etat stale de reentrer dans le set de validateurs.
5. Les success et echecs de handshake mettent a jour les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomie d'reur) and emettent des logs structures avec l ID du peer distant and l empreinte du digest. Surveillez ces מציין לשפוך גלאי לשידורים חוזרים או קונפיגורציות les mauvaises תליון להפצה.

## Gestion des cles et payloads
- חשבון היררכיה של גזירה:
  - `sk_spend` -> `nk` (מפתח מבטל), `ivk` (מפתח צפייה נכנס), `ovk` (מפתח צפייה יוצא), `fvk`.
- מטענים משותפים הם שימושיים ב-AEAD עם מפתחות משותפים שמקורם ב-ECDH; des view keys d auditeur optionnelles peuvent etre attachees aux outputs selon la politique de l actif.
- Ajouts CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, מבקר כלי עבודה pour dechiffrer les memos, et le helper `iroha app zk envelope` pour produire/inspecter des envelopes I0108030X offline. Torii לחשוף את ה-meme flux de derivation דרך `POST /v1/confidential/derive-keyset`, retournant des formes hex et base64 pour que les wallets puissent recuperer les hierarchies de cles programtiquement.## גז, מגבלות ושליטה ב-DoS
- לוח זמנים לקביעת גז:
  - Halo2 (Plonkish): בסיס `250_000` גז + `2_000` גז בשווי קלט ציבורי.
  - `5` בייט חסין גז, בתוספת חיובים פרטניים (`300`) והתחייבות ממוצעת (`500`).
  - Les operateurs peuvent surcharger ces constantes via la configuration node (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); les changements se propagent au demarrage ou quand la couche de config hot-reload ו-sont appliques deterministiquement באשכול.
- מגבלת משך זמן (ברירת מחדל ניתנות להגדרה):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Les proofs depassant `verify_timeout_ms` הפלה l קביעת הוראה (הצבעות ממשל eettent `proof verification exceeded timeout`, `VerifyProof` retourne une reur).
- תוספות מכסות המבטיחות חיים: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, et `max_public_inputs` בוני בלוקים נולדים; `reorg_depth_bound` (>= `max_anchor_age_blocks`) governe la retention des נקודות הגבול.
- זמן הריצה לביצוע דחוי לתחזוקה של טרנזקציות ללא מגבלות על עסקאות או בלוק, שגיאות שגיאות `InvalidParameter` קובעות את תוצאות החשבונות ללא פגע.
- Mempool prefiltre les טרנזקציות סודיות par `vk_id`, longueur de proof et age de anchor avant d appeler le verificateur pour borner l usage dessources.
- La Verification s arrete deterministiquement sur timeout ou violation de borne; פחות עסקאות מהדהדות עם שגיאות מפורשות. הרכיבים האחוריים של SIMD אינם אופציונליים אלא מתאימים לתאימות גז.

### קווי בסיס של שערי כיול וקבלה
- **לוחות-formes de reference.** Les runs de calibration DOIVENT couvrir les trois profils hardware ci-dessous. Les runs ne couvrant pas tous les profiles sont rejetes in review.

  | פרופיל | אדריכלות | מעבד / מופע | מהדר דגלים | Objectif |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) או Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Etablit des valeurs plancher sans intrinsics vectorielles; לנצל pour regler les tables de cout fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | שחרור par defaut | Valide le path AVX2; ודא que les speedups SIMD restent dans la tolerance du gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | שחרור par defaut | הבטח את ה-backend NEON שאר הקבוע ויישר את לוחות הזמנים x86. |

- **רתמת אמת מידה.** Tous les rapports de calibration gas DOIVENT etre produits avec:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` pour confirmer le fixture deterministe.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` quand les couts d opcode VM changent.- **תיקון אקראיות.** יצואן `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` avant de lancer les benches pour que `iroha_test_samples::gen_account_in` bascule sur la voie deterministe `KeyPair::from_seed`. Le harness imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` une seule fois; si la manque משתנה, la review DOIT echouer. Toute nouvelle utilite de calibration doit continuer a honorer cette env var lors de l introduction d alea auxiliaire.

- **Capture Des Resultats.**
  - העלאת קורות חיים קריטריון (`target/criterion/**/raw.csv`) יוצקים את הפרופיל ב-Artefact de release.
  - Stocker les metriques derivees (`ns/op`, `gas/op`, `ns/gas`) ב-[פנקס כיול גז סודי](./confidential-gas-calibration) עם commit git and la use decompilateur.
  - Conserver les deux derniers baselines par profil; supprimer les snapshots plus anciens une fois le rapport le plus valide לאחרונה.

- **סובלנות קבלה.**
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-avx2` DOIVENT rester <= +/-1.5%.
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-neon` DOIVENT rester <= +/-2.0%.
  - הצעות הכיול הנחוצות וההתאמות בלוח הזמנים נחוצות או ה-RFC מובהקות.

- **רשימת בדיקה לביקורת.** Les submitters sont responsables de:
  - כלול `uname -a`, תוספות של `/proc/cpuinfo` (דגם, דריכה), ו-`rustc -Vv` בלוח הכיול.
  - Verifier que `IROHA_CONF_GAS_SEED` apparait dans la sortie bench (les benches impriment la seed active).
  - הבטחת דגלים תכונה קוצב לב ואימות סודי מראה את הייצור (`--features confidential,telemetry` lors des benches עם טלמטריה).

## תצורה ופעולות
- `iroha_config` ajoute la section `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetry emet des metriques agregees: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `effective_height`000X, Sumeragi, Sumeragi, Sumeragi,500X ללא שם: sans Jamais Exposer Des Donnees en Clair.
- RPC משטחים:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## אסטרטגיית בדיקות
- דטרמיניזם: דשדוש aleatoire des transactions dans les blocs donne des Merkle roots et nullifier sets identiques.
- Resilience aux reorg: simuler des reorgs multi-blocks avec anchors; les nullifiers restent stables et les anchors stale sont rejetes.
- Invariants de gas: מאמת את השימוש של גז זהות קוד זמין ללא תאוצה SIMD.
- מבחנים מוגבלים: הוכחות aux plafonds de taille/gas, ספירת כניסה/יציאה מקסימלית, אכיפה של פסקי זמן.
- מחזור חיים: פעולות ממשל הפעלה/ביטול של אימות ופרמטרים, בדיקות לפי סיבוב אפריז.
- מדיניות FSM: מעברים אוטוריסטים/בינתיים, נדל"ן ותליית מעבר, ודחיית מפול אוטומטית ליעילות עילית.
- דחיפות של רישום: retrait d urgence fige les actifs a `withdraw_height` et rejette les proofs apres.
- שער יכולת: validateurs עם `conf_features` לא תואמים לגושים דחויים; משקיפים עם `assume_valid=true` ללא קונצנזוס.
- Equivalence d etat: noeuds validator/full/observer produisent des roots d etat identiques sur la chaine canonique.
- שלילה מטומטמת: מוכיחה פגמים, מטעינים מידות, והתנגשויות מבטלות את הקביעה.

## הגירה
- תכונה מגודרת להפצה: שלב C3 הוא סוף סוף, ברירת המחדל של `enabled` a `false`; les noeuds annoncent leurs capacites avant de rejoindre le set de validateurs.
- Les actifs transparents ne sont pas affectes; הוראות סודיות הנדרשות לרישום מנות ראשונות וניהול משא ומתן.
- Les noeuds compiles sans support confidentiel distinent les blocks pertinents deterministiquement; ils ne peuvent pas rejoindre le set de validateurs mais peuvent fonctionner comme observers avec `assume_valid=true`.
- בראשית מופיעה רישום ראשי תיבות, הרכבי פרמטרים, פוליטיקה סודית לפעילות, ומפתחות אופציונליות.
- Les operators suivent les runbooks publies pour rotation de registry, transitions de politique et retrait d urgence afin de maintenir des upgrades deterministes.## חסין תנועה
- בנצ'מרקר להרכבי הפרמטרים של Halo2 (המעגל, אסטרטגיית חיפוש) ורשום התוצאות ב-Playbook של כיול עבור מספר ברירות מחדל של גז/פסק זמן עם רענון פרוצ'יין `confidential_assets_calibration.md`.
- מסיים את המדיניות הפוליטית של מבקר הגילוי ואת ממשקי ה-API של עמיתים לצפייה סלקטיבית, באישור זרימת העבודה ב-Torii בחתימת טיוטת הממשל.
- Etendre le schema de Witness הצפנת לשלוט על פלטי ריבוי נמענים ותזכירים אצווה, בפורמט המעטפה המתועד עבור ה-SDK של implementateurs.
- הממונה על מעגלים אבטחה חיצוניים, רישומים ונהלים של רוטציה של פרמטרים וארכיון מסקנות אקדמיות לביקורת פנימית.
- מפרט APIs de reconciliation de spendness pour auditors and publier la guidance de scope view-key afin que les vendors de wallets implementent les memes smantiques d attestation.## יישום שלב ד
1. **שלב M0 - עצור התקשות הספינה**
   - [x] La derivation de nullifier suit Maintenant le design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) avec un ordering deterministe des updates du impose ledger des updates.
   - [x] אפליקציית ביצוע L execution des plafonds de taille de proof et des quotas confidentiels par transaction/par bloc, rejetant les transactions hors budget avec desreurs deterministes.
   - [x] מודעה לחיצת יד P2P `ConfidentialFeatureDigest` (תקציר אחורי + רישום טביעות אצבע) ו-echoue les mismatches deterministiquement באמצעות `HandshakeConfidentialMismatch`.
   - [x] Retirer les panics dans les paths d execution confidentielle et ajouter un role gating pour les noeuds non pris en charge.
   - [ ] Appliquer les budgets de timeout du verificateur et les bornes de profondeur de reorg pour les frontier checkpoints.
     - [x] תקציבי זמן קצוב של יישומי אימות; les proofs depassant `verify_timeout_ms` echouent maintenant deterministiquement.
     - [x] Les frontier checkpoints מכבדים תחזוקה `reorg_depth_bound`, prunant les checkpoints plus anciens que la fenetre configuree tout en gardant des snapshots deterministes.
   - הקדמה ל-`AssetConfidentialPolicy`, מדיניות FSM ו-gates d enforcement pour les הוראות שנקבעו / העברה / לחשוף.
   - Commit `conf_features` בכותרות של גוש וסרבן להשתתפות של validateurs quand les digests הרישום/פרמטרים משתנים.
2. **שלב M1 - רישום ופרמטרים**
   - ספר רישום `ZkVerifierEntry`, `PedersenParams`, et `PoseidonParams` עם ממשל אופציונלי, התפתחות התפתחות ותנועה של מטמון.
   - Cablage du syscall לשפוך רישום חיפושי חיפוש, מזהי לוח זמנים של גז, גיבוב סכימה וכו'.
   - מפרסם פורמט של מטען חדש v1, וקטורים של גזירת מפתחות לשפוך ארנקים, ותמיכה ב-CLI עבור התנועה של המפתחות הסודיים.
3. **שלב M2 - גז וביצועים**
   - מיישם את לוח הזמנים של גז קביעת, מחברי בלוק, ומרתם את המדדים עם טלמטריה (השהיית אימות, נקודות הוכחה, דחיית מפול).
   - מחסומי CommitmentTree של Durcir, LRU לחיוב, ואינדיקטורים לביטול עבור עומסי עבודה מרובי נכסים.
4. **שלב M3 - סיבוב וארנק כלי עבודה**
   - קבלה פעילה של הוכחות מרובות פרמטרים וגירסאות מרובות; תומך אני הפעלה/הדחה טייס לפי ממשל avec runbooks de transition.
   - ספר זרימות העברת SDK/CLI, זרימות עבודה של מבקר סריקה, וכלים לפיוס הוצאות.
5. **שלב M4 - ביקורת ופעולות**
   - פורניר זרימות העבודה של מפתחות המבקר, APIs של גילוי סלקטיבי, ופעולות ההפעלה של ספרי ההפעלה.
   - Planifier une revue externe cryptographie/securite et publier les מסקנות ב-`status.md`.

שלב צ'אק פגש את אבני הדרך של מפת הדרכים ועמיתי הבדיקות לביצוע הביצוע הסופי של הבלוקצ'יין.