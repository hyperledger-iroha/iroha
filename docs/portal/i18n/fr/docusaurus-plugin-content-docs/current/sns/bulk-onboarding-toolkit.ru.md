---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page concerne `docs/source/sns/bulk_onboarding_toolkit.md`, ce qui signifie
Les opérateurs ont présenté les recommandations du SN-3b en dehors du dépôt de clonage.
:::

# Boîte à outils d'intégration groupée SNS (SN-3b)

**Feuille de route suivante :** SN-3b "Outils d'intégration en masse"  
**Articles :** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les bureaux d'enregistrement des entreprises doivent fournir un enregistrement `.sora` ou
`.nexus` pour les installations et les rails de règlement. Ручная сборка
Les charges utiles JSON ou la CLI disponible ne sont pas disponibles, lorsque SN-3b est installé
Détermination du constructeur CSV-to-Norito, pour la structure de votre bâtiment
`RegisterNameRequestV1` pour Torii ou CLI. Хелпер заранее валидирует каждую строку,
Vous pouvez ajouter un manifeste agrégé et un JSON fonctionnel, et vous pouvez l'utiliser
charges utiles автоматически, записывая структурированные reçus для аудитов.

## 1. Schéma CSV

Парсер требует следующую строку заголовка (порядок гибкий):| Colonne | Обязательно | Description |
|---------|-------------|--------------|
| `label` | Oui | Méthode applicable (cas mixte; instrument normalisé selon la norme v1 et UTS-46). |
| `suffix_id` | Oui | Le suffixe d'identification de l'identifiant (désposé ou `0x` hex). |
| `owner` | Oui | Entrez AccountId (littéral IH58 ; indice @domain facultatif) pour l'enregistrement. |
| `term_years` | Oui | Le chien `1..=255`. |
| `payment_asset_id` | Oui | Règlement actif (par exemple `xor#sora`). |
| `payment_gross` / `payment_net` | Oui | Sans cela, les éditions précédentes sont actives. |
| `settlement_tx` | Oui | JSON utilise ou utilise une plate-forme de conversion ou de hachage. |
| `payment_payer` | Oui | AccountId, plaque d'immatriculation automatique. |
| `payment_signature` | Oui | JSON ou fichier de stockage pour l'intendant ou la trésorerie. |
| `controllers` | Facultatif | Sélectionnez le contrôleur d'adresse, en utilisant `;` ou `,`. Utilisez `[owner]`. |
| `metadata` | Facultatif | JSON en ligne ou `@path/to/file.json` avec des conseils de résolution, des instructions TXT et des. d. En utilisant `{}`. |
| `governance` | Facultatif | JSON en ligne ou `@path` pour `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

La colonne de couleur peut être trouvée dans le premier jour, si elle ajoute `@` à l'heure actuelle.
Vous pouvez utiliser le fichier CSV de manière optimale.

## 2. Aidez-moi```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Principales options :

- `--require-governance` ouvre les portes hors du crochet de gouvernance (possible pour
  enchères premium ou missions réservées).
- `--default-controllers {owner,none}` résout le problème en utilisant les boutons du contrôleur
  падать обратно на propriétaire.
- `--controllers-column`, `--metadata-column` et `--governance-column` fonctionnent
  переименовать опциональные колонки при работе с amont exports.

Dans le cas du script, il y a un manifeste agrégé :

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "ih58...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"ih58...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"ih58...",
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

Si vous utilisez le `--ndjson`, alors le `RegisterNameRequestV1` doit être vérifié.
Un document JSON bien conçu qui permet d'automatiser les tâches effectuées dans
Torii :

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Ouvertures automatiques

### 3.1 Mode Torii REST

Sélectionnez `--submit-torii-url` et `--submit-token`, puis `--submit-token-file`,
Comment ouvrir le formulaire de manifeste dans Torii :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Aide à retarder l'arrivée d'Odin `POST /v1/sns/registrations` et à l'aider à le faire.
  est activé par HTTP. Les réponses sont ajoutées au journal des messages de NDJSON.
- `--poll-status` est automatiquement installé `/v1/sns/registrations/{selector}` après
  каждой отправки (до `--poll-attempts`, by умолчанию 5), чтобы подтвердить
  видимость записи. Recherchez `--suffix-map` (mapping JSON `suffix_id` dans la zone
  "suffixe"), l'instrument que vous utilisez est `{label}.{suffix}` pour l'interrogation.
- Noms : `--submit-timeout`, `--poll-attempts` et `--poll-interval`.

### 3.2 Exécuter la CLI d'IrohaPour programmer l'affichage du manifeste à partir de la CLI, placez-le dans le binaire :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

-Les contrôleurs doivent créer des entrées `Account` (`controller_type.kind = "Account"`),
  La CLI peut alors prendre en charge uniquement les contrôleurs basés sur les comptes.
- Les blobs de métadonnées et de gouvernance sont disponibles dans les délais pour chaque projet et
  передаются в `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr et les codes CLI sont enregistrés ; Aucun code n'est disponible.

Il est possible d'ouvrir le programme d'ouverture de session (Torii et CLI) pour effectuer des vérifications immédiates.
déploiements de registraires ou répétition de solutions de secours.

### 3.3 Ouvertures des visites

Le script `--submission-log <path>` ajoute NDJSON :

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Les réponses spécifiques Torii correspondent à la structure de `NameRecordV1` ou
`RegisterNameResponseV1` (par exemple `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`), les tâches à bord et les opérations de mise en œuvre peuvent être effectuées sans problème
texte. Prenez ce journal du registraire pour les billets du manifeste pour
воспроизводимого доказательства.

## 4. Portail de sécurité automatique

CI et portail emplois вызывают `docs/portal/scripts/sns_bulk_release.sh`, который
Il s'agit d'un assistant et d'un artéfact de sauvegarde selon le module `artifacts/sns/releases/<timestamp>/` :

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

Scénario :1. Téléchargez `registrations.manifest.json`, `registrations.ndjson` et copiez
   Il s'agit d'un CSV dans le directeur de la publication.
2. Ouvrir le manifeste à partir de Torii et/ou de la CLI (sur le site), ouvrez
   `submissions.log` est destiné à votre cuisine.
3. Formez `summary.json` avec la réponse détaillée (en utilisant l'URL Torii, en utilisant la CLI,
   timestamp), le portail automatique peut télécharger le bundle dans la bibliothèque
   artéfacts.
4. Générer `metrics.prom` (remplacer par `--metrics`) avec Prometheus-совместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   et les résultats отправки. Résumé JSON s'applique à cela.

Workflows est le directeur de l'archivage qui réalise les œuvres d'art les plus récentes
все необходимое для аудита.

## 5. Télémétrie et bord

Voici la mesure du générateur `sns_bulk_release.sh` qui correspond à la série suivante :

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Remplacez `metrics.prom` par le side-car Prometheus (pour Promtail ou batch
importateur), les registraires, les intendants et les pairs de gouvernance видели согласованный
прогресс vrac процессов. Tableau de bord Grafana
`dashboards/grafana/sns_bulk_release.json` vous permet de visualiser votre date :
по суффиксам, объем платежей и соотношение успешных/неуспешных отправок. Dashbord
Le filtre `release` permet aux auditeurs d'accéder au programme CSV.

## 6. Validation et procédures d'achat- **Canonisation de l'étiquette :** входы нормализуются Python IDNA + minuscule et
  фильтрами символов Norme v1. Les metskis ne sont pas à la hauteur de certains de vos projets.
- **Garde-corps numériques :** identifiants de suffixe, années de mandat et conseils sur les prix.
  sont antérieurs à `u16` et `u8`. La plaque est destinée à être utilisée ou hexadécimale.
  `i64::MAX`.
- **Analyse des métadonnées/gouvernance :** analyse JSON en ligne ; ссылки на файлы
  разрешаются относительно CSV. Les métadonnées ne sont pas autorisées à être validées.
- **Contrôleurs :** пустые ячейки соблюдают `--default-controllers`. Указывайте
  явные списки contrôleurs (par exemple `ih58...;ih58...`) при делегировании не-propriétaire.

Les appareils sont liés au contexte des nombres de coups (par exemple
`error: row 12 term_years must be between 1 and 255`). Le script correspond au code `1`
Lors de la validation et de `2`, vous ne pouvez pas envoyer un fichier CSV.

## 7. Tests et procédures

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` покрывает CSV парсинг,
  Dans NDJSON, la gouvernance d'application et les mises en service à partir de CLI ou Torii.
- Assistant pour le système Python (sans supplément) et robot
  Maintenant, j'ai téléchargé `python3`. Les commits d'histoire se rapportent à la CLI dans
  Il s'agit de dépôts appropriés pour vos besoins.

Pour le produit, vous devez utiliser le manifeste générique et le bundle NDJSON.
registraire тикету, чтобы stewards могли воспроизвести точные charges utiles, отправленные
à Torii.