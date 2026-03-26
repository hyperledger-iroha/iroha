---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Cette page reflète `docs/source/sns/bulk_onboarding_toolkit.md` afin que les
Les opérateurs externes découvrent la même direction SN-3b sans cloner le dépôt.
:::

# Toolkit d'intégration du massif SNS (SN-3b)

**Feuille de route de référence :** SN-3b "Outils d'intégration en masse"  
**Artefacts :** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les grands registrars préparent souvent des centaines d'enregistrements `.sora` ou
`.nexus` avec les mèmes approbations de gouvernance et rails de règlement.
Fabriquer des payloads JSON à la main ou relancer la CLI à l'échelle pas, donc SN-3b
livre un builder déterministe CSV vers Norito qui prépare des structures
`RegisterNameRequestV1` pour Torii ou la CLI. L'helper valide chaque ligne en
amont, emet à la fois un manifeste agrégé et du JSON délimité par nouvelles
lignes optionnelles, et peut soumettre automatiquement les payloads tout en
enregistrant des structures de recus pour les audits.

## 1. Schéma CSV

L'analyseur exige la ligne d'en-tête suivante (l'ordre est flexible) :| Colonne | Exigence | Descriptif |
|---------|--------|-------------|
| `label` | Oui | Libelle demande (casse mixte acceptée; l'outil normalise selon Norm v1 et UTS-46). |
| `suffix_id` | Oui | Identifiant numérique de suffixe (décimal ou `0x` hex). |
| `owner` | Oui | AccountId string (domainless encoded literal; canonical i105 only; no `@<domain>` suffix). |
| `term_years` | Oui | Entier `1..=255`. |
| `payment_asset_id` | Oui | Actif de règlement (par exemple `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Oui | Entiers non signes représentant des unités natives de l'actif. |
| `settlement_tx` | Oui | Valeur JSON ou chaîne littérale décrivant la transaction de paiement ou le hachage. |
| `payment_payer` | Oui | AccountId qui a autorisé le paiement. |
| `payment_signature` | Oui | JSON ou chaîne littérale contenant la preuve de signature du steward ou de la tresorerie. |
| `controllers` | Optionnel | Liste séparée par point-virgule ou virgule des adresses de compte du contrôleur. Par défaut `[owner]` si omis. |
| `metadata` | Optionnel | JSON inline ou `@path/to/file.json` fournissant des astuces de résolveur, des enregistrements TXT, etc. Par défaut `{}`. |
| `governance` | Optionnel | JSON inline ou `@path` pointant vers un `GovernanceHookV1`. `--require-governance` impose cette colonne. |Toute colonne peut référencer un fichier externe en préfixant la valeur de cellule
par `@`. Les chemins sont résolus relativement au fichier CSV.

## 2. Exécuteur de l'assistant

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Clés d'options :

- `--require-governance` rejette les lignes sans crochet de gouvernance (utile pour
  les enchères premium ou les affectations réservées).
- `--default-controllers {owner,none}` décide si les cellules contrôleurs vides
  retombent sur le compte propriétaire.
- `--controllers-column`, `--metadata-column`, et `--governance-column` permettent
  de renommer les colonnes optionnelles lors d'exports amont.

En cas de succès, le script écrit un manifeste agrégé :

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

Si `--ndjson` est fourni, chaque `RegisterNameRequestV1` est également écrit comme un
document JSON sur une ligne afin que les automatisations puissent streamer les
demande directement vers Torii :

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

### 3.1 Mode Torii REPOS

Spécifiez `--submit-torii-url` plus `--submit-token` ou `--submit-token-file` pour
pousser chaque entrée du manifeste directement vers Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- L'helper emet un `POST /v1/sns/names` par demande et s'arrête au premier
  erreur HTTP. Les réponses sont ajoutées au journal comme enregistrements NDJSON.
- `--poll-status` réinterroger `/v1/sns/names/{namespace}/{literal}` après chaque
  soumission (jusqu'à `--poll-attempts`, par défaut 5) pour confirmer que
  l'enregistrement est visible. Fournissez `--suffix-map` (JSON de `suffix_id`
  vers des valeurs "suffix") pour que l'outil dérive les littéraux
  `{label}.{suffix}` pour le sondage.
- Ajustables : `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### 3.2 Mode CLI iroha

Pour faire passer chaque entrée du manifeste par la CLI, fournissez le chemin du
binaire :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Les contrôleurs doivent être des entrées `Account` (`controller_type.kind = "Account"`)
  car la CLI expose uniquement des contrôleurs bases sur des comptes.
- Les blobs metadata et gouvernance sont écrits dans des fichiers temporaires par
  requete et transmis a `iroha sns register --metadata-json ... --governance-json ...`.
- Le stdout et stderr de la CLI ainsi que les codes de sortie sont journalisés ;
  les codes non nuls interrompent l'exécution.

Les deux modes de soumission peuvent fonctionner ensemble (Torii et CLI) pour
croiser les déploiements du registraire ou répéter des replis.

### 3.3 Recus de soumission

Quand `--submission-log <path>` est fourni, le script ajoute des entrées NDJSON
capturant :

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Les réponses Torii reussies incluent des champs structures extraits de
`NameRecordV1` ou `RegisterNameResponseV1` (par exemple `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) afin que les tableaux de bord et les
les rapports de gouvernance peuvent analyser le log sans inspecteur du texte libre.
Joignez ce log aux tickets registrar avec le manifeste pour une preuve
reproductible.

## 4. Automatisation de release du portail

Les jobs CI et portail appelant `docs/portal/scripts/sns_bulk_release.sh`, qui
encapsule l'helper et stocke les artefacts sous
`artifacts/sns/releases/<timestamp>/` :

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

Le scénario :

1. Construit `registrations.manifest.json`, `registrations.ndjson`, et copier le
   CSV original dans le répertoire de sortie.
2. Soumet le manifeste via Torii et/ou la CLI (quand configure), en écrivant
   `submissions.log` avec les structures de recus ci-dessus.
3. Emet `summary.json` décrit la version (chemins, URL Torii, chemin CLI,
   timestamp) afin que l'automatisation du portail puisse télécharger le bundle vers
   le stockage d'artefacts.
4. Produit `metrics.prom` (override via `--metrics`) contenant des compteurs
   au format Prometheus pour le total de requêtes, la distribution des suffixes,
   les totaux d'actif et les résultats de soumission. Le CV JSON pointe vers
   ce fichier.Les workflows archivent simplement le répertoire de release comme un seul artefact,
qui contient désormais tout ce dont la gouvernance a besoin pour l'audit.

## 5. Télémétrie et tableaux de bord

Le fichier de métriques génériques par `sns_bulk_release.sh` expose les séries
suivantes :

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` dans votre side-car Prometheus (par exemple via Promtail ou
un import batch) pour aligner registraires, stewards et paires de gouvernance sur
l'avancement en masse. Le tableau Grafana
`dashboards/grafana/sns_bulk_release.json` visualize les memes données avec des
panneaux pour les comptes par suffixe, le volume de paiement et les ratios de
réussite/echec des soumissions. Le tableau filtre par `release` pour que les
les auditeurs peuvent se concentrer sur une seule exécution CSV.

## 6. Validation et modes d'essai- **Normalisation des labels :** les entrées sont normalisées avec Python IDNA plus
  minuscules et filtres de caractères Norme v1. Les étiquettes invalides font écho vite
  avant tout appel reseau.
- **Garde-fous numériques :** les identifiants de suffixe, les années de mandat et les indications de prix doivent
  rester dans les bornes `u16` et `u8`. Les champs de paiement acceptent des
  entiers décimaux ou hex jusqu'à `i64::MAX`.
- **Parsing metadata ou gouvernance :** le JSON inline est analysé directement ; les
  les références aux fichiers sont résolues relativement à l'emplacement du CSV.
  Les métadonnées non objet produisent une erreur de validation.
- **Contrôleurs :** les cellules vides respectent `--default-controllers`. Fournissez
  des listes explicites (par exemple `i105...;i105...`) quand vous déléguez à des
  acteurs non propriétaire.

Les échecs sont des signaux avec des numéros de ligne contextuels (par exemple
`error: row 12 term_years must be between 1 and 255`). Le script sort avec le
code `1` sur erreurs de validation et `2` lorsque le chemin CSV manque.

## 7. Tests et provenance- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` couvre le parsing CSV,
  l'émission NDJSON, l'enforcement gouvernance, et les chemins de soumission CLI ou Torii.
- L'helper est du Python pur (aucune dépendance additionnelle) et tourne partout
  ou `python3` est disponible. L'historique des commits est suivi aux cotes de la
  CLI dans le dépôt principal pour la reproductibilité.

Pour les run de production, joignez le manifeste genre et le bundle NDJSON au
ticket du registrar afin que les stewards puissent rejouer les payloads exacts
soumis un Torii.