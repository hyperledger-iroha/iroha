---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
Le module de commande SN-3b est doté d'un connecteur SN-3b.
:::

# Boîte à outils d'intégration groupée SNS (SN-3b)

**روڈمیپ حوالہ :** SN-3b « Outils d'intégration en masse »  
**Artefacts :** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les bureaux d'enregistrement sont `.sora` et les enregistrements `.nexus` et la gouvernance
approbations et rails de règlement
ہیں۔ Il s'agit de charges utiles JSON et d'une CLI à l'échelle supérieure.
Pour SN-3b, un générateur déterministe CSV-à-Norito est utilisé pour créer une interface CLI avec Torii.
Structures `RegisterNameRequestV1` pour les structures `RegisterNameRequestV1` assistant ہر rangée کو پہلے
valider le manifeste agrégé et le JSON facultatif délimité par une nouvelle ligne
Il y a des audits et des reçus structurés et des charges utiles.
خودکار طور پر submit کر سکتا ہے۔

## 1. Fichier CSV

L'analyseur analyse la ligne d'en-tête ici (commande flexible ici) :| Colonne | Obligatoire | Descriptif |
|--------|----------|-------------|
| `label` | Oui | Étiquette demandée (cas mixte accepté ; outil Norm v1 et UTS-46 pour normaliser les choses). |
| `suffix_id` | Oui | Identificateur de suffixe numérique (décimal یا `0x` hex). |
| `owner` | Oui | AccountId string (domainless encoded literal; canonical i105 only; no `@<domain>` suffix). |
| `term_years` | Oui | Entier `1..=255`. |
| `payment_asset_id` | Oui | Actif de règlement (مثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Oui | Les entiers non signés et les unités natives de l'actif représentent کریں۔ |
| `settlement_tx` | Oui | Valeur JSON, chaîne littérale, transaction de paiement, hachage, par exemple |
| `payment_payer` | Oui | AccountId pour l'autorisation de paiement |
| `payment_signature` | Oui | JSON ou chaîne littérale comme preuve de signature de l'intendant/du trésor |
| `controllers` | Facultatif | Adresses du compte du contrôleur کی liste séparée par des points-virgules/virgules۔ Il s'agit de `[owner]`. |
| `metadata` | Facultatif | JSON en ligne avec `@path/to/file.json` et astuces de résolution, enregistrements TXT et autres Par défaut `{}`۔ |
| `governance` | Facultatif | Inline JSON par `@path` et `GovernanceHookV1` par ici `--require-governance` Colonne pour le bureau |

La colonne est en ligne et est `@` pour un fichier externe et se référer à un fichier externe
Fichier CSV des chemins et résolution relative

## 2. Aide چلانا```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Options clés :

- Le crochet de gouvernance `--require-governance` rejette les lignes de rejet (premium
  enchères یا missions réservées کے لئے مفید).
- `--default-controllers {owner,none}` pour le propriétaire des cellules du contrôleur
  compte پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, et `--governance-column` en amont
  exporte les colonnes facultatives et les colonnes facultatives.

Voici le manifeste agrégé du script :

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
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
```

`--ndjson` est un code JSON monoligne `RegisterNameRequestV1`.
document pour les demandes d'automatisation et les demandes d'automatisation Torii
میں stream کر سکیں:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Soumissions automatisées

### 3.1 Mode REST Torii

`--submit-torii-url` pour `--submit-token` et `--submit-token-file` pour `--submit-token`
manifeste et entrée pour le code Torii et push:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Helper request pour `POST /v1/sns/names` par HTTP
  erreur پر abandonner کرتا ہے۔ Chemin du journal des réponses avec enregistrements NDJSON et ajout
  ہوتے ہیں۔
- `--poll-status` pour la soumission par `/v1/sns/names/{namespace}/{literal}` pour
  دوبارہ requête کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, par défaut 5) تاکہ record
  visible ہونے کی تصدیق ہو۔ `--suffix-map` (JSON et `suffix_id` et valeurs "suffixe"
  سے map کرے) فراہم کریں تاکہ outil `{label}.{suffix}` les littéraux dérivent کر سکے۔
- Paramètres réglables : `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 Mode CLI irohaL'entrée du manifeste et la CLI sont également associées au chemin binaire :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Contrôleurs comme entrées `Account` et comme entrées (`controller_type.kind = "Account"`)
  Les contrôleurs basés sur les comptes exposent les contrôleurs CLI et les contrôleurs basés sur les comptes.
- Métadonnées et blobs de gouvernance et requêtes pour fichiers temporaires et fichiers temporaires
  Il s'agit de `iroha sns register --metadata-json ... --governance-json ...`.
  کئے جاتے ہیں۔
- CLI stdout/stderr et journal des codes de sortie les codes non nuls s'exécutent et abandonnent

دونوں modes de soumission ایک ساتھ چل سکتے ہیں (Torii اور CLI) registraire
les déploiements recoupent et les replis répètent

### 3.3 Reçus de soumission

Dans `--submission-log <path>`, les entrées du script NDJSON ajoutent les éléments suivants :

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Réponses Torii réussies par `NameRecordV1` et `RegisterNameResponseV1` par `RegisterNameResponseV1`
Les champs structurés sont en anglais (`record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) Tableaux de bord et journaux de rapports de gouvernance et texte libre
analyser کر سکیں۔ اس journal کو manifeste کے ساتھ tickets du registraire پر joindre کریں تاکہ
preuve reproductible رہے۔

## 4. Automatisation des versions du portail Docs

Emplois sur le portail CI `docs/portal/scripts/sns_bulk_release.sh` et appelez ici
assistant pour envelopper les objets et les artefacts `artifacts/sns/releases/<timestamp>/`
تحت store کرتا ہے:

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

Scénario :1. `registrations.manifest.json`, `registrations.ndjson` pour l'original
   CSV et répertoire de versions pour les fichiers CSV
2. Torii pour la CLI et la soumission du manifeste (configuration et configuration)
   `submissions.log` میں اوپر والے recettes structurées لکھتا ہے۔
3. `summary.json` émet des fichiers et publie des fichiers et décrit des fichiers (chemins, URL Torii,
   Chemin CLI, horodatage) comme bundle d'automatisation du portail et stockage d'artefacts
   télécharger کر سکے۔
4. `metrics.prom` pour (`--metrics` pour remplacement), ou Prometheus-
   compteurs de format ہوتے ہیں : total des requêtes, répartition des suffixes, totaux des actifs,
   اور résultats de la soumission۔ résumé JSON fichier fichier et lien lien ici

Workflows dans le répertoire des versions et dans les artefacts et dans les archives
اب وہ سب کچھ ہے جو gouvernance کو audit کے لئے درکار ہے۔

## 5. Tableaux de bord de télémétrie et

`sns_bulk_release.sh` Le fichier de métriques de la série expose les éléments suivants :

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` pour Prometheus side-car pour alimentation (pour Promtail
یا importateur de lots کے ذریعے) تاکہ registraires, stewards et pairs de gouvernance en vrac
progrès پر aligné رہیں۔ Carte Grafana
`dashboards/grafana/sns_bulk_release.json` et panneaux de données par suffixe
nombres, volume de paiement, et taux de réussite/échec des soumissions Carte `release`
filtre les auditeurs et l'exécution CSV pour les exercices

## 6. Validation et modes de défaillance- **Canonisation de l'étiquette :** entrées Python IDNA en minuscules et Norm v1
  filtres de caractères سے normaliser ہوتے ہیں۔ étiquettes invalides appels réseau سے پہلے
  échouer rapidement ہوتے ہیں۔
- **Garde-corps numériques :** identifiants de suffixe, années de mandat et indications de prix `u16` et `u8`
  limites کے اندر ہونے چاہئیں۔ Champs de paiement décimal et entiers hexadécimaux `i64::MAX`
  تک accepter کرتے ہیں۔
- **Analyse des métadonnées et de la gouvernance :** JSON en ligne parse parse ہوتا ہے؛ fichier
  références emplacement CSV et résolution relative ہوتی ہیں۔ Métadonnées non-objet
  erreur de validation دیتا ہے۔
- **Contrôleurs :** Cellules `--default-controllers` et Honor Honor non-propriétaire
  acteurs et délégués et listes de contrôleurs explicites (`i105...;i105...`)

Numéros de ligne contextuels des échecs rapport de rapport ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Erreurs de validation de script ici
`1` et chemin CSV manquant et `2` quitter le chemin d'accès ici.

## 7. Test de la provenance اور

- Analyse CSV `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py`, NDJSON
  émissions, application de la gouvernance, chemins de soumission CLI/Torii et couverture ہے۔
- Helper pure Python ہے (dépendances supplémentaires ici) et `python3` دستیاب
  ہو وہاں چلتا ہے۔ Valider l'historique CLI dans le référentiel principal et dans la piste
  تاکہ reproductibilité ہو۔Exécutions de production avec manifeste généré et bundle NDJSON et ticket du registraire
Ajouter une pièce jointe aux stewards Torii et soumettre une relecture exacte des charges utiles
کر سکیں۔