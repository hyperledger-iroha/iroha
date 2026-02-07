---
lang: he
direction: rtl
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

חוק: brouillon/esquisse pour accompagner les taches d'implementation de la governance. תליון מחליף Les formes peuvent ליישום. Le determinisme et la politique RBAC sont des contraintes normatives; Torii חתם/סומיטר עסקאות עסקאות `authority` et `private_key` sont fournis, sinon les clients construisent et soumettent a `/transaction`.

אפרקו
- Tous les endpoints renvoient du JSON. Pour les flux qui produisent des טרנזקציות, les reponses incluent `tx_instructions` - un tableau d'une ou plusieurs הוראות squelette:
  - `wire_id`: מזהה רישום עבור סוג ההוראות
  - `payload_hex`: בתים של מטען Norito (hex)
- Si `authority` et `private_key` sont fournis (ou `private_key` sur les DTO de ballots), Torii signe et soumet la transaction et renvoie quand meme Grafana.
- סינון, לקוחות הרכיבו את SignedTransaction עם סמכות ו-chain_id, חתימה ו-POST vers `/transaction`.
- SDK של Couverture:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` renvoie `GovernanceProposalResult` (לנרמל את סטטוס האלופים/סוג), `ToriiClient.get_governance_referendum_typed` renvoie `GovernanceReferendumResult`, Prometheus `GovernanceTally`, `ToriiClient.get_governance_locks_typed` renvoie `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` renvoie `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` renvoie I0000000X סוג של משטחים מבעבעים ל-I018NI50X governance avec des exemples d'usage dans le README.
- לקוח Python leger (`iroha_torii_client`): `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` renvoient des חבילות סוגי `GovernanceInstructionDraft` (qui encapsulent le squelette `tx_instructions` de le0vite), ניתוח JSON מנואל quand les scripts Component des flux Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` לחשוף את סוגי העוזרים להגיש הצעות, משאל, תוצאות, מנעולים, סטטיסטיקות ביטול נעילה, ואחזקת `listGovernanceInstances(namespace, options)` בתוספת למועצה לנקודות קצה (Prometheus, Prometheus, I0000006000, I18NI `governancePersistCouncil`, `getGovernanceCouncilAudit`) הם לקוחות Node.js puissent paginer `/v1/gov/instances/{ns}` ופיילוט של זרימות העבודה VRF במקביל לרישום מקרים של ניגודים קיימים.

נקודות קצה

- POST `/v1/gov/proposals/deploy-contract`
  - בקשה (JSON):
    {
      "namespace": "אפליקציות",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "ih58...?",
      "private_key": "...?"
    }
  - תגובה (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - אימות: les noeuds canonisent `abi_hash` pour l'`abi_version` fourni et rejettent les incoherences. יוצקים `abi_version = "v1"`, la valeur attendue est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.ניגודי API (פריסה)
- POST `/v1/contracts/deploy`
  - בקשה: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - רכיב: calcule `code_hash` depuis le corps du program IVM et `abi_hash` depuis l'en-tete `abi_version`, puis soumet `code_hash` (I0100000070X) (בתים `.to` השלמות) יוצקים `authority`.
  - תגובה: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  -שקר:
    - קבל `/v1/contracts/code/{code_hash}` -> renvoie le manifeste stocke
    - קבל `/v1/contracts/code-bytes/{code_hash}` -> renvoie `{ code_b64 }`
- POST `/v1/contracts/instance`
  - בקשה: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - רכיב: deploie le bytecode fourni ומיפוי מיידי אקטיבי של `(namespace, contract_id)` באמצעות `ActivateContractInstance`.
  - תגובה: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

שירות ד'כינוי
- POST `/v1/aliases/voprf/evaluate`
  - בקשה: { "blinded_element_hex": "..." }
  - תגובה: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete l'implementation de l'evaluateur. Valeur actuelle: `blake2b512-mock`.
  - הערות: evaluateur mock deterministe qui applique Blake2b512 with separation de domaine `iroha.alias.voprf.mock.v1`. Prevus pour l'outillage de test jusqu'a ce que le צינור VOPRF de production soit relie a Iroha.
  - שגיאות: HTTP `400` על קלט hex mal forme. Torii renvoie une enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec le message d'erreur du decoder.
- POST `/v1/aliases/resolve`
  - בקשה: { "כינוי": "GB82 WEST 1234 5698 7654 32" }
  - תגובה: { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - הערות: דרוש שלב ריצה של גשר ISO (`[iso_bridge.account_aliases]` ב-`iroha_config`). Torii לנרמל את הכינוי ב-Retirant les espaces ו-en mettant en majuscules avant le lookup. Retourne 404 הוא בכינוי נעדר ו-503 עבור זמן הריצה ISO bridge לא היה פעיל.
- POST `/v1/aliases/resolve_index`
  - בקשה: { "אינדקס": 0 }
  - תגובה: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - הערות: les index d'alias sont מקצה deterministiquement selon l'ordre de configuration (מבוסס 0). Les clients peuvent mettre en cache hors ligne pour construire des pistes d'audit pour les evenements d'attestation d'alias.

Cap de Taille de Code
- פרמטר מותאם אישית: `max_contract_code_bytes` (JSON u64)
  - Controle la taille maximale autorisee (en bytes) pour le stockage de code de contrat on-chain.
  - ברירת מחדל: 16 MiB. Les noeuds rejettent `RegisterSmartContractBytes` lorsque la taille de l'image `.to` depasse le cap avec une erreur d'invariant.
  - Les operators peuvent ajuster via `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et unload number numerique.- POST `/v1/gov/ballots/zk`
  - בקשה: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות:
    - Quand les inputs publics du circuit incluent `owner`, `amount` et `duration_blocks`, et que la preuve verifie contre la VK configuree, le noeud cree ou etend un verrou de gouvernance pour I1020X 1020X1020NI `owner`. La direction reste cachee (`unknown`); seuls כמות/תפוגה sont mis a jour. Les re-votes sont monotones: amount et expiry ne font qu'augmenter (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Les re-votes ZK qui tentent de reduire amount ou expire sont rejetes cote serveur avec des diagnostics `BallotRejected`.
    - L'execution du contrat doit appeler `ZK_VOTE_VERIFY_BALLOT` avant d'enfiler `SubmitBallot`; המארחים מחייבים את הבריח.

- POST `/v1/gov/ballots/plain`
  - Requete: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "directionay"}: "Aye|tainN
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות: les re-votes sont en extension seule - un nouveau ballot ne peut pas reduire l'amount ou l'expiry du verrou existant. Le `owner` doit egaler l'authority de la עסקה. La duree minimale est `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Requete: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - אפקט על השרשרת (הפיגום בפועל): הכנס הצעה אחת ליישום אישור הוספה של `ContractManifest` תקלה מינימלית `code_hash` עם l'`abi_hash` attendu et marque la proposition נחקק. Si un manifeste existe deja pour le `code_hash` avec un `abi_hash` שונה, l'enactment est jete.
  - הערות:
    - Pour les elections ZK, les chemins de contrat doivent appeler `ZK_VOTE_VERIFY_TALLY` avant d'executer `FinalizeElection`; המארחים הטילו לבטל שימוש ייחודי. `FinalizeReferendum` דחה את משאלי העם ZK tant que le tally n'est pas finalise.
    - La cloture automatique a `h_end` emet ייחוד אושר/נדחה pour les referendums מישור; les referendums ZK restent Jusqu'a ce qu'un סגור לסיים סויט סומיס et que `FinalizeReferendum` soit להורג.
    - אימותי ההצבעה שימושיים באישור+דחה; נמנע ne compte pas pour le participation.- POST `/v1/gov/enact`
  - Requete: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - הערות: Torii soumet la transaction signee quand `authority`/`private_key` sont fournis; sinon il renvoie un squelette pour signature and soumission client. Le preimage est optionnel and informatif pour l'instant.

- קבל את `/v1/gov/proposals/{id}`
  - נתיב `{id}`: id de proposition hex (64 תווים)
  - תגובה: { "נמצא": bool, "הצעה": { ... }? }

- קבל את `/v1/gov/locks/{rid}`
  - נתיב `{rid}`: string id de referendum
  - תגובה: { "נמצא": bool, "referendum_id": "לפטר", "מנעולים": { ... }? }

- קבל `/v1/gov/council/current`
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - הערות: renvoie le Council persiste si present; sinon deriva un fallback deterministe avec l'asset de stake configure et les seuils (מירויר de la spec VRF jusqu'a ce que des preuves VRF en direct soient persistees on-chain).

- POST `/v1/gov/council/derive-vrf` (תכונה: gov_vrf)
  - Requete: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "רגיל|קטן", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement: לאמת la preuve VRF de chaque candidat contre l'input canonique derive de `chain_id`, `epoch` et du beacon du dernier hash de block; נסה par bytes de sortie desc avec שובר שוויון; renvoie les top `committee_size` membres. לא מתמיד.
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - הערות: Normal = pk en G1, proof en G2 (96 בתים). Small = pk en G2, proof en G1 (48 בתים). הקלטים מפרידים בין הדומיין ובין `chain_id`.

### ברירות מחדל של ניהול (iroha_config `gov.*`)

Le Council fallback use par Torii quand aucun roster persiste n'existe est parameter via `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

עוקף מקבילות לסביבה:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` limite le nombre de membres fallback renvoyes quand aucun Council n'est persiste, `parliament_term_blocks` definit la longueur d'epoque utilisee pour la derivation de seed (`parliament_min_stake`), I081NI minimum de stake מאחד את המינימלים) sur l'asset d'eligibilite, et `parliament_eligibility_asset_id` selectne quel solde d'asset est סריקה lors de la construction du set de candidats.

La Verification VK de gouvernance n'a pas de bypass: la Verification de ballot requiert toujours une cle `Active` avec bytes inline, et les environnements ne doivent pas s'appuyer sur des toggles de test pour sauter la אימות.

RBAC
- L'execution on-chain דורש הרשאות:
  - הצעות: `CanProposeContractDeployment{ contract_id }`
  - קלפי: `CanSubmitGovernanceBallot{ referendum_id }`
  - חקיקה: `CanEnactGovernance`
  - הנהלת המועצה (עתיד): `CanManageParliament`חסות מרחבי שמות
- פרמטר מותאם אישית `gov_protected_namespaces` (מערך JSON de strings) active le gating d'admission pour les deploys dans les spaces name lists.
- הלקוחות אינם כוללים את המטא-נתונים של עסקאות עבור הפריסה של מרחבי השמות:
  - `gov_namespace`: le namespace cible (לדוגמה, "אפליקציות")
  - `gov_contract_id`: l'id de contrat logique dans le namespace
- `gov_manifest_approvers`: מערך JSON אופציונלי מזהי חשבונות מאומתים. Quand un manifeste de lane הכרזה על מניין > 1, האישור מחייב את הרשות דה לה עסקאות פלוס רשימות השונות לשביעות רצונך למניין המניפסט.
- La telemetrie expose des compteurs d'admission via `governance_manifest_admission_total{result}` afin que les operateurs distinguent les admits reussis des chemins `missing_manifest`, `non_validator_authority`, Prometheus, Prometheus, Prometheus,0 `runtime_hook_rejected`.
- La telemetrie לחשוף le chemin d'enforcement via `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour auditer les approbations manquantes.
- Les lanes appliquent l'allowlist de namespaces publiee dans leurs manifestes. Toute Transaction qui fixe `gov_namespace` doit fournir `gov_contract_id`, et le namespace doit apparaitre dans le set `protected_namespaces` du manifeste. Les soumissions `RegisterSmartContractCode` sans cette metadata sont rejetees lorsque la protection est active.
- L'admission impose qu'une proposition de governance שנחקק existe pour le tuple `(namespace, contract_id, code_hash, abi_hash)`; sinon la validation echoue avec une rereur NotPermitted.

הוקס לשדרוג זמן ריצה
- Les manifestes de lane peuvent declarer `hooks.runtime_upgrade` pour gater les הוראות לשדרוג זמן ריצה (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Champs du hook:
  - `allow` (bool, ברירת המחדל `true`): quand `false`, מפרט את הוראות השדרוג בזמן ריצה לאחר שנדחו.
  - `require_metadata` (bool, ברירת המחדל `false`): exige l'entree metadata specifiee par `metadata_key`.
  - `metadata_key` (מחרוזת): שם אפליקציית מטא נתונים לפי וו. ברירת מחדל `gov_upgrade_id` quand la metadata est requise ou qu'une allowlist est presente.
  - `allowed_ids` (מערך של מחרוזות): רשימת היתרים optionnelle de valeurs metadata (קצץ אחרי). Rejette quand la valeur fournie n'est pas listee.
- Quand le hook est present, l'admission de la file applique la politique metadata avant l'entree de la transaction dans la file. מטה-נתונים, חשובים צופים או רשימת ההיתרים האפשרית ללא אישור שגיאה.
- La telemetrie עקוב אחר תוצאות דרך `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- הטרנזקציות המספקות להוק כוללות את המטא-נתונים `gov_upgrade_id=<value>` (או ה-cle definie par le manifeste) en plus des approbations de validateurs requises par le quorum du manifeste.נקודת קצה de commodite
- POST `/v1/gov/protected-namespaces` - אפליקציה `gov_protected_namespaces` directement sur le noeud.
  - בקש: { "מרחבי שמות": ["אפליקציות", "מערכת"] }
  - תגובה: { "בסדר": true, "applied": 1 }
  - הערות: destine a l'admin/testing; לדרוש הגדרה של ממשק API של אסימון. יוצקים להפקה, מעדיף את חתימת העסקה עם `SetParameter(Custom)`.

עוזרי CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Recupere les instances de contrat pour le namespace and verifie que:
    - Torii stocke le bytecode pour chaque `code_hash`, et son digest Blake2b-32 correspond au `code_hash`.
    - Le manifeste stocke sous `/v1/contracts/code/{code_hash}` rapporte des valeurs `code_hash` et `abi_hash` correspondantes.
    - Une proposition de governance שנחקקה existe pour `(namespace, contract_id, code_hash, abi_hash)` נגזר מה-meme hashing de offer-id que le noeud use.
  - מיון JSON עם `results[]` לפי קונטרה (בעיות, קורות חיים של מניפסט/קוד/הצעה) בתוספת דיכוי קורות חיים ו-une ligne sauf (`--no-summary`).
  - השתמש במבקר במרחבי השמות מגן או על המאמת של זרימות העבודה לפרוס פקדים לפי ניהול.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Emet le squelette JSON de metadata לנצל את הפריסה של מרחבי השמות, כולל `gov_manifest_approvers` אפשרויות לשביעות רצונם של המניין הכללי.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — les lock hints sont requis lorsque `min_bond_amount > 0`, et tout ensemble de hints fourni doit inclure `owner`, `amount` et Prometheus.
  - מאמת מזהי חשבונות קנוניים, מבצע קנוניזציה של רמזים לביטול של 32 בתים וממזג את הרמזים לתוך `public_inputs_json` (עם `--public <path>` לעקיפות נוספות).
  - המבטל נגזר מהתחייבות ההוכחה (קלט ציבורי) בתוספת `domain_tag`, `chain_id` ו-`election_id`; `--nullifier` מאומת כנגד ההוכחה כאשר היא מסופקת.
  - Le resume en une ligne expose Maintenant un `fingerprint=<hex>` deterministe derive du `CastZkBallot` encode ainsi que les hints decodes (`owner`, `amount`, I002NI00X, I002NI00X, I002NI00X `direction` si fournis).
  - Les reponses CLI annotent `tx_instructions[]` avec `payload_fingerprint_hex` plus des champs decodes pour que les outils downstream verifient le squelette sans reimplementer le decodage Norito.
  - Fournir les hints de lock permet au noeud d'emettre des events `LockCreated`/`LockExtended` pour les ballots ZK une fois que le circuit expose les memes valeurs.
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - שם הכינוי `--lock-amount`/`--lock-duration-blocks` refletent les noms de flags ZK pour la parite de scripting.
  - קורות החיים של המיון `vote --mode zk` הכוללים את טביעת האצבע של ההוראות המקודדות ואפשרויות ההצבעה של ה-Champs (`owner`, `amount`, Prometheus, אישור מהיר, I010000221X, I010eure) avant signature du squelette.רישום מופעים
- קבל את `/v1/gov/instances/{ns}` - רשום את המופעים הבלתי פעילים של מרחב השמות.
  - פרמטרים של שאילתה:
    - `contains`: פילטר par sous-chaine de `contract_id` (תלוי רישיות)
    - `hash_prefix`: מסנן par prefixe hex de `code_hash_hex` (אותיות קטנות)
    - `offset` (ברירת מחדל 0), `limit` (ברירת מחדל 100, מקסימום 10_000)
    - `order`: un des `cid_asc` (ברירת מחדל), `cid_desc`, `hash_asc`, `hash_desc`
  - תגובה: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) או `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Balayage d'unlocks (מפעיל/ביקורת)
- קבל את `/v1/gov/unlocks/stats`
  - תגובה: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - הערות: `last_sweep_height` reflete la hauteur de bloc la plus recente ou les locks expire on ete balayes et persistes. `expired_locks_now` est calcule in scannant les enregistrements de lock avec `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - בקשה (סגנון DTO v1):
    {
      "authority": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "ih58...?",
      "nullifier": "blake2b32:...64hex?"
    }
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (תכונה: `zk-ballot`)
  - קבל את ה-JSON `BallotProof` ישיר ו-revoie un squelette `CastZkBallot`.
  - בקשה:
    {
      "authority": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "הצבעה": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 du conteneur ZK1 ou H2*
        "root_hint": null, // מחרוזת hex אופציונלית של 32 בתים (שורש זכאות)
        "owner": null, // AccountId optionnel סי le le circuit commit בעלים
        "nullifier": null // מחרוזת hex אופציונלית של 32 בתים (רמז לבטל)
      }
    }
  - תגובה:
    {
      "בסדר": נכון,
      "מקובל": נכון,
      "reason": "בנה שלד עסקה",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - הערות:
    - Le serveur map `root_hint`/`owner`/`nullifier` optionnels du ballot vers `public_inputs_json` pour `CastZkBallot`.
    - Les bytes de l'envelope sont recoding en base64 pour le payload d'instruction.
    - La reponse `reason` pass a `submitted transaction` quand Torii soumet le ballot.
    - תכונה `zk-ballot` הייתה פעילה.Parcours de Verification CastZkBallot
- `CastZkBallot` לפענח la preuve base64 fournie et rejette les payloads vides ou mal formes (`BallotRejected` avec `invalid or empty proof`).
- Le host resout la cle de verification du ballot depuis le referendum (`vk_ballot`) או les defaults de governance et exige que l'enregistrement existe, soit `Active` et transporte des bytes inline.
- Les bytes de cle de verification stocks sont re-hashes avec `hash_vk`; tout mismatch de commitment arrete l'execution avant verification pour se proteger des entrees registry corrompues (`BallotRejected` avec `verifying key commitment mismatch`).
- Les bytes de preuve sont dispatches au backend enregistr via `zk::verify_backend`; les transscriptions invalides remontent en `BallotRejected` avec `invalid proof` et l'instruction echoue deterministiquement.
- על ההוכחה לחשוף התחייבות קלפי ושורש זכאות כתשומות ציבוריות; השורש חייב להתאים ל-`eligible_root` של הבחירות, והמבטל הנגזר חייב להתאים לכל רמז שסופק.
- Les preuves reussies emettent `BallotAccepted`; מבטל דופליקס, שורשי d'eligibility perimes ou regressions de lock continuent de produire les raisons de rejet existantes decrites plus haut dans ce document.

## Mauvaise conduite des validateurs et consensus joint

### זרימת עבודה של חיתוך וכלא

Le consensus emet `Evidence` encode en Norito lorsqu'un validateur viole le protocole. Chaque מטען מגיע ב-`EvidenceStore` בזיכרון ו, אין לערוך, הוא מתממש במפה `consensus_evidence` adossee au WSV. Les enregistrements plus anciens que `sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת מחדל `7200` בלוקים) sont rejetes pour garder l'archive bornee, mais le rejet est log pour les operators. הראיות באופק מכבדות גם את `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) ואת עיכוב החיתוך `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`); ממשל יכול לבטל קנסות עם `CancelConsensusEvidencePenalty` לפני שהחתך חל.

Les עבירות reconnues se mappt un-a-un sur `EvidenceKind`; les discriminants sont stables et imposes par le מודל נתונים:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - le validateur a signe des hashes en conflit pour le meme tuple `(phase,height,view,epoch)`.
- **InvalidQc** - un agregateur a gossip un commit certificate dont la forme echoue aux verifications deterministes (לדוגמה, bitmap de signataires vide).
- **InvalidProposal** - un leader a propose un block qui echoue la validation structurelle (לדוגמה, viole la regle de locked-chain).
- **צנזורה** - קבלות הגשה חתומות מציגות עסקה שמעולם לא הוצעה/בוצעה.

מפעילי חוץ ושידורים חוזרים של המטענים באמצעות:

- Torii: `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, et `... submit --evidence-hex <payload>`.

La governance doit traiter les bytes d'evidence comme preuve canonique:1. **Collecter le payload** avant qu'il expire. ארכיון בתים Norito ברוטס עם גובה/תצוגה של מטא נתונים.
2. **Preparer la penalite** en embarquant le payload dans un referendum ou une instruction sudo (לדוגמה, `Unregister::peer`). L'execution re-valide le payload; ראיות מאל צורה או מעופש הוא לדחות קביעה.
3. **Planifier la topologie de suivi** afin que le validateur fautif ne puisse pas revenir immediatement. הזרמים הטיפוסים כוללים `SetParameter(Sumeragi::NextMode)` ו-`SetParameter(Sumeragi::ModeActivationHeight)` עם סגל המחזור.
4. **מבקר את התוצאות** דרך `/v1/sumeragi/evidence` et `/v1/sumeragi/status` pour confirmer que le compteur d'evidence a avance et que la gouvernance a applique le retrait.

### Sequencage du consensus joint

Le consensus joint garantit que l'ensemble de validateurs sortant finalize le bloc de frontiere avant que le nouvel אנסמבל להתחיל מציע. זמן הריצה להטיל את הרגל באמצעות מכשירי הפרמטרים:

- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` דויבנט etre commits dans le **meme blok**. `mode_activation_height` doit etre strictement superieur a la hauteur du bloc qui a porte la mise a jour, donnant au moins un bloc de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) est la garde de configuration qui empeche les hand-offs a lag zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`) מעכב את קיצוץ הקונצנזוס כך שהממשל יכול לבטל עונשים לפני שהם יחולו.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- זמן ריצה ו-CLI חשיפה לפרמטרים מבוימים באמצעות `/v1/sumeragi/params` ו-`iroha --output-format text ops sumeragi params`, מפעילים מאשרים את ההפעלה העליונה ורשימות המאמתים.
- L'automatisation de governance doit toujours:
  1. Finaliser la decision de retrait (או שילוב מחדש) תומך ראיות.
  2. Enfiler une configuration reconfiguration de suivi avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveiller `/v1/sumeragi/status` jusqu'a ce que `effective_consensus_mode` bascule a la hauteur attendue.

הצג תסריט qui fait tourner les validateurs ou applique un slashing **ne doit pas** tenter une הפעלה אפס פיגור או אומטר את הפרמטרים של מסירה; עסקאות ces sont rejetees et laissent le reseau dans le mode תקדים.

## Surfaces de telemetrie- Les metriques Prometheus יצואנית לפעילות ממשלתית:
  - `governance_proposals_status{status}` (מד) suit les compteurs de suggests par status.
  - `governance_protected_namespace_total{outcome}` (מונה) הגדלה וכניסה למרחבי שמות, בני חסות מקבלים או נדחים את הפריסה.
  - `governance_manifest_activations_total{event}` (מונה) לרשום את ה-Insertions de Manifest (`event="manifest_inserted"`) ו-les bindings de namespace (`event="instance_bound"`).
- `/status` כולל חפצים `governance` כדי להחזיר את ההצעות למחשבי הצעות, לכלול את כל ההצעות של מרחבי השמות ואת רשימת ההפעלות האחרונות של המניפסט (מרחב שם, מזהה חוזה, קוד/ABI hash, גובה בלוק, חותמת זמן הפעלה). Les Operaurs peuvent sonder ce champ pour confirmer que les enactments ont mis a jour les manifests and que les ports de namespaces, proteges sont imposes.
- Un template Grafana (`docs/source/grafana_governance_constraints.json`) et le runbook de telemetrie dans `telemetry.md` montrent הערה כבל דה התראות לשפוך הצעות בלוקי, הפעלת מניפסטים, או דוחה חוסר תשומת לב לריצה של מרחבי שמות תלויים ב-proteges.