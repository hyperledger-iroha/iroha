---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f291c973ed526fe87063f0a699f2a5dd2dfaa7c6b5037e8493edda27569095be
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: bulk-onboarding-toolkit
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflete `docs/source/sns/bulk_onboarding_toolkit.md` afin que les
operateurs externes voient la meme guidance SN-3b sans cloner le depot.
:::

# Toolkit d'onboarding massif SNS (SN-3b)

**Reference roadmap:** SN-3b "Bulk onboarding tooling"  
**Artefacts:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les grands registrars preparent souvent des centaines de registrations `.sora` ou
`.nexus` avec les memes approbations de gouvernance et rails de settlement.
Fabriquer des payloads JSON a la main ou relancer la CLI ne scale pas, donc SN-3b
livre un builder deterministe CSV vers Norito qui prepare des structures
`RegisterNameRequestV1` pour Torii ou la CLI. L'helper valide chaque ligne en
amont, emet a la fois un manifeste agrege et du JSON delimite par nouvelles
lignes optionnel, et peut soumettre les payloads automatiquement tout en
enregistrant des recus structures pour les audits.

## 1. Schema CSV

Le parseur exige la ligne d'en-tete suivante (l'ordre est flexible):

| Colonne | Requis | Description |
|---------|--------|-------------|
| `label` | Oui | Libelle demande (casse mixte acceptee; l'outil normalise selon Norm v1 et UTS-46). |
| `suffix_id` | Oui | Identifiant numerique de suffixe (decimal ou `0x` hex). |
| `owner` | Oui | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Oui | Entier `1..=255`. |
| `payment_asset_id` | Oui | Actif de settlement (par exemple `xor#sora`). |
| `payment_gross` / `payment_net` | Oui | Entiers non signes representant des unites natives de l'actif. |
| `settlement_tx` | Oui | Valeur JSON ou chaine litterale decrivant la transaction de paiement ou hash. |
| `payment_payer` | Oui | AccountId qui a autorise le paiement. |
| `payment_signature` | Oui | JSON ou chaine litterale contenant la preuve de signature du steward ou de la tresorerie. |
| `controllers` | Optionnel | Liste separee par point-virgule ou virgule des adresses de compte controller. Par defaut `[owner]` si omis. |
| `metadata` | Optionnel | JSON inline ou `@path/to/file.json` fournissant des hints de resolver, des enregistrements TXT, etc. Par defaut `{}`. |
| `governance` | Optionnel | JSON inline ou `@path` pointant vers un `GovernanceHookV1`. `--require-governance` impose cette colonne. |

Toute colonne peut referencer un fichier externe en prefixant la valeur de cellule
par `@`. Les chemins sont resolus relativement au fichier CSV.

## 2. Executer l'helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Options cles:

- `--require-governance` rejette les lignes sans hook de gouvernance (utile pour
  les encheres premium ou les affectations reservees).
- `--default-controllers {owner,none}` decide si les cellules controllers vides
  retombent sur le compte owner.
- `--controllers-column`, `--metadata-column`, et `--governance-column` permettent
  de renommer les colonnes optionnelles lors d'exports amont.

En cas de succes le script ecrit un manifeste agrege:

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
        "asset_id":"xor#sora",
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

Si `--ndjson` est fourni, chaque `RegisterNameRequestV1` est aussi ecrit comme un
document JSON sur une ligne afin que les automatisations puissent streamer les
requetes directement vers Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Soumissions automatisees

### 3.1 Mode Torii REST

Specifiez `--submit-torii-url` plus `--submit-token` ou `--submit-token-file` pour
pousser chaque entree du manifeste directement vers Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- L'helper emet un `POST /v1/sns/registrations` par requete et s'arrete au premier
  erreur HTTP. Les reponses sont ajoutees au log comme enregistrements NDJSON.
- `--poll-status` re-interroge `/v1/sns/registrations/{selector}` apres chaque
  soumission (jusqu'a `--poll-attempts`, defaut 5) pour confirmer que
  l'enregistrement est visible. Fournissez `--suffix-map` (JSON de `suffix_id`
  vers des valeurs "suffix") pour que l'outil derive les litteraux
  `{label}.{suffix}` pour le polling.
- Ajustables: `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### 3.2 Mode iroha CLI

Pour faire passer chaque entree du manifeste par la CLI, fournissez le chemin du
binaire:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Les controllers doivent etre des entrees `Account` (`controller_type.kind = "Account"`)
  car la CLI expose uniquement des controllers bases sur des comptes.
- Les blobs metadata et governance sont ecrits dans des fichiers temporaires par
  requete et transmis a `iroha sns register --metadata-json ... --governance-json ...`.
- Le stdout et stderr de la CLI ainsi que les codes de sortie sont journalises;
  les codes non zero interrompent l'execution.

Les deux modes de soumission peuvent fonctionner ensemble (Torii et CLI) pour
croiser les deployments du registrar ou repeter des fallbacks.

### 3.3 Recus de soumission

Quand `--submission-log <path>` est fourni, le script ajoute des entrees NDJSON
capturant:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Les reponses Torii reussies incluent des champs structures extraits de
`NameRecordV1` ou `RegisterNameResponseV1` (par exemple `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) afin que les dashboards et les
reports de gouvernance puissent parser le log sans inspecter du texte libre.
Joignez ce log aux tickets registrar avec le manifeste pour une evidence
reproductible.

## 4. Automatisation de release du portail

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

Le script:

1. Construit `registrations.manifest.json`, `registrations.ndjson`, et copie le
   CSV original dans le repertoire de release.
2. Soumet le manifeste via Torii et/ou la CLI (quand configure), en ecrivant
   `submissions.log` avec les recus structures ci-dessus.
3. Emet `summary.json` decrivant la release (chemins, URL Torii, chemin CLI,
   timestamp) afin que l'automatisation du portail puisse uploader le bundle vers
   le stockage d'artefacts.
4. Produit `metrics.prom` (override via `--metrics`) contenant des compteurs
   au format Prometheus pour le total de requetes, la distribution des suffixes,
   les totaux d'asset et les resultats de soumission. Le JSON resume pointe vers
   ce fichier.

Les workflows archivent simplement le repertoire de release comme un seul artefact,
qui contient desormais tout ce dont la gouvernance a besoin pour l'audit.

## 5. Telemetrie et dashboards

Le fichier de metriques genere par `sns_bulk_release.sh` expose les series
suivantes:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` dans votre sidecar Prometheus (par exemple via Promtail ou
un import batch) pour aligner registrars, stewards et pairs de gouvernance sur
l'avancement en masse. Le tableau Grafana
`dashboards/grafana/sns_bulk_release.json` visualise les memes donnees avec des
panneaux pour les comptes par suffixe, le volume de paiement et les ratios de
reussite/echec des soumissions. Le tableau filtre par `release` pour que les
auditeurs puissent se concentrer sur une seule execution CSV.

## 6. Validation et modes d'echec

- **Normalisation des labels:** les entrees sont normalisees avec Python IDNA plus
  lowercase et filtres de caracteres Norm v1. Les labels invalides echouent vite
  avant tout appel reseau.
- **Garde-fous numeriques:** suffix ids, term years, et pricing hints doivent
  rester dans les bornes `u16` et `u8`. Les champs de paiement acceptent des
  entiers decimaux ou hex jusqu'a `i64::MAX`.
- **Parsing metadata ou governance:** le JSON inline est parse directement; les
  references a des fichiers sont resolues relativement a l'emplacement du CSV.
  Les metadata non-objet produisent une erreur de validation.
- **Controllers:** les cellules vides respectent `--default-controllers`. Fournissez
  des listes explicites (par exemple `i105...;i105...`) quand vous deleguez a des
  acteurs non owner.

Les echecs sont signales avec des numeros de ligne contextuels (par exemple
`error: row 12 term_years must be between 1 and 255`). Le script sort avec le
code `1` sur erreurs de validation et `2` lorsque le chemin CSV manque.

## 7. Tests et provenance

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` couvre le parsing CSV,
  l'emission NDJSON, l'enforcement governance, et les chemins de soumission CLI ou Torii.
- L'helper est du Python pur (aucune dependance additionnelle) et tourne partout
  ou `python3` est disponible. L'historique des commits est suivi aux cotes de la
  CLI dans le depot principal pour la reproductibilite.

Pour les runs de production, joignez le manifeste genere et le bundle NDJSON au
ticket du registrar afin que les stewards puissent rejouer les payloads exacts
soumis a Torii.
