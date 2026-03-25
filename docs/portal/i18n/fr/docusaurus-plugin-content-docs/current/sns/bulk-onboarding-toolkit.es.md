---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Refleja `docs/source/sns/bulk_onboarding_toolkit.md` pour les opérateurs externes
le même guide SN-3b sans cloner le référentiel.
:::

# Boîte à outils d'intégration de Masivo SNS (SN-3b)

**Référence de la feuille de route :** SN-3b "Outils d'intégration en masse"  
**Artéfacts :** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les registraires grandes à menu pré-préparent les centres de registre `.sora` ou `.nexus`
avec les mêmes autorisations de gouvernement et les rails de règlement. Charges utiles Armar JSON
à la main ou retourner à l'exécution de la CLI sans escalader, alors SN-3b entre dans un constructeur
déterministe de CSV à Norito pour préparer les structures `RegisterNameRequestV1` pour
Torii ou CLI. El helper valida cada fila de antemano, émet tanto un manifeste
agrégé comme JSON délimité par des sauts de ligne facultatifs, et vous pouvez les envoyer
les charges utiles sont automatiquement enregistrées lors de l'enregistrement des recettes structurées pour les auditoires.

## 1. Esquema CSV

L'analyseur nécessite le fil d'encabezado suivant (l'ordre est flexible) :| Colonne | Requerido | Description |
|---------|-----------|-------------|
| `label` | Si | Étiquette sollicitée (si vous acceptez peut-être/moins ; l'outil de normalisation à partir de la norme v1 et de l'UTS-46). |
| `suffix_id` | Si | Identificateur numérique du suffixe (décimal ou `0x` hex). |
| `owner` | Si | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Si | Entrez `1..=255`. |
| `payment_asset_id` | Si | Actif de règlement (par exemple `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Si | Enteros sin signo que representan unidades natives del activo. |
| `settlement_tx` | Si | Valeur JSON ou chaîne littérale qui décrit la transaction de paiement ou de hachage. |
| `payment_payer` | Si | AccountId qui autorise le paiement. |
| `payment_signature` | Si | JSON ou chaîne littérale avec la vérification de l'entreprise de steward ou du trésorier. |
| `controllers` | Optionnel | Liste séparée par point et par la virgule des directions du contrôleur de compte. Par défaut, `[owner]` est omise. |
| `metadata` | Optionnel | JSON en ligne ou `@path/to/file.json` qui prouve des astuces de résolveur, d'enregistrement TXT, etc. Par défaut `{}`. |
| `governance` | Optionnel | JSON en ligne ou `@path` correspondant à un `GovernanceHookV1`. `--require-governance` exige cette colonne. |Chaque colonne peut référencer un fichier externe indiquant la valeur de la zone avec `@`.
Les itinéraires sont générés par rapport aux archives CSV.

## 2. Exécuter l'assistant

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Options clés :

- `--require-governance` rechaza filas sin un hook de gobernanza (utile pour
  subastas premium ou asignaciones reservadas).
- `--default-controllers {owner,none}` décide si les celdas sont vides de contrôleurs
  vuelven a la compte propriétaire.
- `--controllers-column`, `--metadata-column` et `--governance-column` permis
  renommez les colonnes optionnelles lorsque vous travaillez avec des exportations en amont.

En cas de sortie du script, écrivez un manifeste agrégé :

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

Si vous proposez `--ndjson`, chaque `RegisterNameRequestV1` peut également être écrit comme un
document JSON d'une seule ligne pour que les automatisations puissent transmettre
sollicite directement le Torii :

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envois automatisés

### 3.1 Mode Torii REST

Spécifique `--submit-torii-url` mais `--submit-token` ou `--submit-token-file` pour
Emprunter chaque entrée du manifeste directement à Torii :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- L'assistant émet un `POST /v1/sns/names` pour solliciter et abandonner avant le
  erreur d'amorce HTTP. Les réponses sont liées à la route du journal comme les registres
  NDJSON.
- `--poll-status` voir consulter `/v1/sns/names/{namespace}/{literal}` après
  chaque envoi (jusqu'à `--poll-attempts`, par défaut 5) pour confirmer l'enregistrement
  est visible. Proporcione `--suffix-map` (JSON de `suffix_id` a valeurs "suffixe")
  pour que les outils dérivent les littéraux `{label}.{suffix}` pour effectuer un sondage.
- Ajuste: `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### 3.2 Mode CLI iroha

Pour entrer chaque fois dans le manifeste par la CLI, indiquez l'itinéraire du binaire :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Les contrôleurs doivent être entrés `Account` (`controller_type.kind = "Account"`)
  parce que la CLI n'expose actuellement que les contrôleurs basés sur des comptes.
- Les blobs de métadonnées et de gouvernance sont décrits dans les archives temporelles par
  sollicitude et se pasan a `iroha sns register --metadata-json ... --governance-json ...`.
- El stdout y stderr de la CLI mas los codigos de salida se registran; les codigos
  pas de zéro pour interrompre l’éjection.

Les modes d'envoi peuvent être exécutés conjointement (Torii et CLI) pour vérifier
despliegues del registrar ou ensayar fallbacks.

### 3.3 Recettes d'envoi

Lorsque vous proposez `--submission-log <path>`, le script annexe entre NDJSON que
capturant :

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Les réponses exitosas de Torii incluent des champs structurés extrados de
`NameRecordV1` ou `RegisterNameResponseV1` (par exemple `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) pour les tableaux de bord et rapports de
Le gouvernement peut analyser le journal sans inspecter le texte libre. Adjunte este log a
les billets du registraire sont accompagnés du manifeste pour des preuves reproductibles.

## 4. Automatisation de la publication du portail

Les travaux de CI et du portail appellent à `docs/portal/scripts/sns_bulk_release.sh`,
qui enveloppe l'assistant et garde les objets sous `artifacts/sns/releases/<timestamp>/` :

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

Le script :

1. Construisez `registrations.manifest.json`, `registrations.ndjson` et copiez le
   CSV original dans le répertoire de sortie.
2. Envoyez le formulaire en utilisant Torii et/ou la CLI (lorsque vous l'avez configuré), en l'écrivant
   `submissions.log` avec les recettes structurées de l'arrière.
3. Emite `summary.json` décrit la version (route, URL Torii, route CLI,
   timestamp) pour que l'automatisation du portail puisse charger le bundle a
   stockage d'objets.
4. Produire `metrics.prom` (remplacement via `--metrics`) qui contient des contacts
   au format Prometheus pour l'ensemble des sollicitudes, distribution de soufijos,
   total des actifs et résultats de l'envoi. Le JSON de reprendre en ligne à est
   archives.Les workflows archivent simplement le directeur de publication comme un seul artefact,
qui maintenant contient tout ce que la gouvernance a besoin pour les auditoires.

## 5. Télémétrie et tableaux de bord

Le fichier de mesures généré par `sns_bulk_release.sh` expose les éléments suivants
série :

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimentez `metrics.prom` sur votre side-car de Prometheus (par exemple via Promtail ou
un importador batch) para mantener registrars, stewards y pares de gobernanza
alineados sur le progrès masivo. Le tableau Grafana
`dashboards/grafana/sns_bulk_release.json` visualiser les mêmes données avec les panneaux
pour les conteos por sufijo, volumen de pago y ratios de exito/fallo de envios. El
tablero filtre por `release` pour que les auditeurs puissent entrer en une seule fois
corrida de CSV.

## 6. Validation et modes de chute- **Normalisation du label :** les entrées sont normalisées avec Python IDNA plus
  minuscules et filtres de caractères Norm v1. Étiquettes invalidos fallan rapido antes
  de cualquier llamada de red.
- **Numéros de garde-corps :** identifiants de suffixe, années de mandat et conseils sur les prix deben caer
  à l’intérieur des limites `u16` et `u8`. Les champs de paiement acceptent les valeurs décimales ou
  hex hasta `i64::MAX`.
- **Analyse des métadonnées ou de la gouvernance :** JSON en ligne est analysé directement ; las
  les références aux archives se rapportent à l'emplacement du CSV. Métadonnées
  qu'aucun objet marin ne produit une erreur de validation.
- **Contrôleurs :** celdas en blanco respetan `--default-controllers`. Proportion
  listes de contrôleurs explicites (par exemple `i105...;i105...`) au délégué
  acteurs sans propriétaire.

Los fallos se reportan con numeros de fila contextuales (par exemple
`error: row 12 term_years must be between 1 and 255`). El script sale avec code
`1` en erreurs de validation et `2` lorsque la route du CSV échoue.

## 7. Tests et procédure

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` analyse du cube CSV,
  émission NDJSON, application de la gouvernance et des chemins d'envoi par CLI ou Torii.
- L'assistant est Python pur (sans dépendances supplémentaires) et corre en n'importe quoi
  la place de la sonde `python3` est disponible. L'historial de commits se rastrea junto
  à la CLI dans le référentiel principal pour la reproductibilité.Pour les courses de production, ajouter le manifeste généré et le bundle NDJSON à
ticket del registrar pour que les stewards puissent reproduire exactement les charges utiles
que se enviaron a Torii.