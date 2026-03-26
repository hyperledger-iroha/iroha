---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/sns/bulk_onboarding_toolkit.md` afin que les
Operators externes voient la meme guidance SN-3b sans cloner le depot.
:::

# ערכת כלים ל- onboarding massif SNS (SN-3b)

**מפת דרכים התייחסות:** SN-3b "כלי שילוב בכמות גדולה"  
**חפצי אמנות:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les grands registrars preparent souvent des centaines de registrations `.sora` ou
`.nexus` avec les memes approbations de governance et rails de settlement.
Fabriquer des payloads JSON a la main או relancer la CLI ne scale pas, donc SN-3b
livre un Builder deterministe CSV vers Norito איך להכין את המבנים
`RegisterNameRequestV1` pour Torii ou la CLI. L'helper valide chaque ligne en
amont, Emet a la fois un manifeste agrege et du JSON delimite par nouvelles
lignes optionnel, et peut soumettre les payloads automatiquement tout en
נרשם des recus structures pour les audits.

## 1. סכימת CSV

Le parseur exige la ligne d'en-tete suivante (l'ordre est גמיש):

| קולון | דרישה | תיאור |
|--------|--------|------------|
| `label` | Oui | Libelle demande (case mixte acceptee; אני רוצה לנרמל את התקן של Norm v1 et UTS-46). |
| `suffix_id` | Oui | זיהוי מספרי דה סיומת (עשרוני או `0x` hex). |
| `owner` | Oui | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Oui | Entier `1..=255`. |
| `payment_asset_id` | Oui | Actif de Settlement (למשל `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Oui | Entiers non signnes representant des unites natives de l'actif. |
| `settlement_tx` | Oui | Valeur JSON או שרשרת מילותי התוצאה של טרנזקציית ה-paiement או hash. |
| `payment_payer` | Oui | AccountId qui a autorise le paiement. |
| `payment_signature` | Oui | JSON או שרשרת המחזיקה הרשמית לה מקדמת חתימה של המנהל או דה לה טרסורריה. |
| `controllers` | אופציונלי | רשימה נפרדת של נקודה-virgule או כתובות של קונטרולר. Par defaut `[owner]` סי אומיס. |
| `metadata` | אופציונלי | JSON inline ou `@path/to/file.json` fournissant des hints de resolver, des enregistrements TXT, וכו'. Par defaut `{}`. |
| `governance` | אופציונלי | JSON inline או `@path` pointant לעומת `GovernanceHookV1`. `--require-governance` להטיל cette colonne. |

Toute colonne peut referencer un fichier externe en prefixant la valeur de cellule
par `@`. Les chemins sont resolus relativement au fichier CSV.

## 2. מוציא לפועל

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

אפשרויות קלס:

- `--require-governance` rejette les lignes sans hook de governance (utile pour
  les encheres premium ou les affectations reservees).
- `--default-controllers {owner,none}` מחליטים על סרטונים של בקרי תאים
  הבעלים של retombent sur le compte.
- `--controllers-column`, `--metadata-column`, et `--governance-column` קבוע
  de renommer les colonnes optionnelles lors d'exports amont.

בסופו של דבר, התסריט כתוב בכתב הסכמה:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```Si `--ndjson` est fourni, chaque `RegisterNameRequestV1` est aussi ecrit comme un
מסמך JSON sur une ligne afin que les automatisations puissent streamer les
מבקש הכוונה גרסה Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Soumissions אוטומציה

### מצב 3.1 Torii REST

Specificez `--submit-torii-url` פלוס `--submit-token` או `--submit-token-file` לשפוך
pousser chaque entree du manifeste direction vers Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- L'helper emet un `POST /v1/sns/names` par requete et s'arrete au premier
  שגיאה ב-HTTP. התשובות נשלחות לכניסה לרישום NDJSON.
- `--poll-status` תחקור מחדש `/v1/sns/names/{namespace}/{literal}` אפר צ'אק
  soumission (jusqu'a `--poll-attempts`, defaut 5) pour confirmer que
  הרישום נראה לעין. Fournissez `--suffix-map` (JSON de `suffix_id`
  vers des valeurs "סיומת") pour que l'outil derive les litteraux
  `{label}.{suffix}` pour le polling.
- מתכווננים: `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### מצב 3.2 עבור CLI

Pour faire passer chaque entree du manifeste par la CLI, fournissez le chemin du
בינארי:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Les controllers doivent etre des entrees `Account` (`controller_type.kind = "Account"`)
  car la CLI לחשוף את הייחודיות של בקרי בסיסים סור des comptes.
- Les blobs metadata et governance sont ecrits dans des fichiers temporaires par
  בקש ושלח את `iroha sns register --metadata-json ... --governance-json ...`.
- Le stdout et stderr de la CLI ainsi que les codes de sortie sont journalises;
  les codes ללא אפס interrompent l'execution.

Les deux modes de soumission peuvent fonctionner אנסמבל (Torii et CLI) pour
croiser les deployments du registrar או repeter des fallbacks.

### 3.3 Recus de soumission

Quand `--submission-log <path>` est fourni, le script ajoute des entrees NDJSON
לוכד:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Les reponses Torii reussies incluent des champs structures extraits de
`NameRecordV1` או `RegisterNameResponseV1` (לדוגמה `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) afin que les dashboards et les
דוחות de governance puissent parser le log sans inspecter du texte libre.
Joignez ce log aux tickets registrar avec le manifeste pour une evidence
ניתן לשחזור.

## 4. Automatization de release du portail

Les jobs CI et portail appellent `docs/portal/scripts/sns_bulk_release.sh`, qui
encapsule l'helper et stocke les artefacts sous
`artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

התסריט:1. צור `registrations.manifest.json`, `registrations.ndjson`, et copie le
   CSV מקורי ברפרטואר השחרור.
2. Soumet le manifeste דרך Torii et/ou la CLI (קונפיגורציית quand), en ecrivant
   `submissions.log` avec les recus structures ci-dessus.
3. Emet `summary.json` מגדיר את השחרור (chemins, URL Torii, chemin CLI,
   חותמת זמן) afin que l'automatisation du portail puisse uploader le bundle vers
   le stockage d'artefacts.
4. מוצר `metrics.prom` (עקיפה דרך `--metrics`) Content des compteurs
   au format Prometheus pour le total de requetes, la distribution des suffixes,
   les totaux d'asset et les resultats de soumission. קורות חיים של Le JSON pointe vers
   ce fichier.

Les זרימות עבודה ארכיון פשוטות הרפרטואר של שחרור comme un seul artefact,
qui contient desormais tout ce dont la governance a besoin pour l'audit.

## 5. טלמטריה ודשבורדים

Le fichier de metriques genere par `sns_bulk_release.sh` expose les series
suivantes:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` dans votre car sidecar Prometheus (לדוגמה דרך Promtail ou
un import batch) pour aligner aligners, stewards et pars de governance sur
l'avancement בהמוניהם. Le tableau Grafana
`dashboards/grafana/sns_bulk_release.json` הדמיית les memes donnees avec des
panneaux pour les comptes par suffixe, le volume de paiement et les ratios de
reussite/echec des soumissions. Le tableau filter par `release` pour que les
המבקרים מתרכזים בביצוע CSV.

## 6. Validation et modes d'echec

- **נורמליזציה של תוויות:** מנות ראשונות בעלות נורמליזציה עם Python IDNA plus
  אותיות קטנות ומסנני תכונות נורמה v1. Les labels invalides echouent vite
  avant tout appel reseau.
- **מספרים גרדיים:** מזהי סיומת, שנות טווח ורמזים לתמחור
  rester dans les bornes `u16` et `u8`. Les champs de paiement acceptent des
  entiers decimaux ou hex jusqu'a `i64::MAX`.
- **ניתוח מטא נתונים או ממשל:** כיוון הניתוח המוטבע של JSON; les
  הפניות a des fichiers sont resolues relativement a l'emplacement du CSV.
  Les metadata non-objet produisent une reur de validation.
- **בקרים:** les cellules מגלה את `--default-controllers`. פורניסז
  des lists explicites (לדוגמה `soraカタカナ...;soraカタカナ...`) quand vous deleguez a des
  שחקנים שאינם בעלי.

Les echecs sont signales avec des numeros de ligne contextuels (לדוגמה
`error: row 12 term_years must be between 1 and 255`). מיון התסריט עם התסריט
קוד `1` sur erreurs de validation et `2` lorsque le chemin CSV manque.

## 7. בדיקות ומוצא

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` couvre le parsing CSV,
  l'emission NDJSON, l'enforcement governance, et les chemins de soumission CLI ou Torii.
- L'helper est du Python pur (aucune dependance additionnelle) et tourne partout
  ou `python3` זמין. L'historique des commits est suivi aux cotes de la
  CLI dans le depot principal pour la reproductibilite.Pour les runs de production, joignez le manifeste genere et le bundle NDJSON au
ticket du registrar afin que les stewards puissent rejouer les payloads exacts
soumis a Torii.