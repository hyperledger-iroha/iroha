---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` pour les opérateurs externes
je vais à mon orientation SN-3b pour cloner le dépôt.
:::

# Boîte à outils d'intégration dans Massa SNS (SN-3b)

**Référence de la feuille de route :** SN-3b "Outils d'intégration en masse"  
**Artefatos :** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les registraires grandes fréquemment préparent des centenas de registros `.sora` ou
`.nexus` comme mesures de gouvernance et de règlement. Montar
payloads JSON manuellement ou réexécuter une CLI jusqu'à l'échelle, puis entrer SN-3b dans l'étape
constructeur déterministe de CSV pour Norito qui prépare les structures
`RegisterNameRequestV1` pour Torii ou pour une CLI. O helper valida cada linha
avec des antécédents, émet tant un manifeste agrégé quant à JSON délimité par
quebra de linha facultative, et peut envoyer les charges utiles automatiquement en quantité
registra recibos estruturados para auditorias.

## 1. Esquema CSV

L'analyseur exige une ligne de cabecalho suivante (a ordem e flexivel) :| Colonne | Obligatoire | Description |
|--------|-------------|---------------|
| `label` | Sim | Étiquette sollicitée (cas mixte Aceita ; ferramenta normaliza conforme Norm v1 e UTS-46). |
| `suffix_id` | Sim | Identificateur numérique de suffixe (décimal ou `0x` hex). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Inteiro `1..=255`. |
| `payment_asset_id` | Sim | Ativo de règlement (par exemple `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Sim | Inteiros sem sinal representando unidades natives do ativo. |
| `settlement_tx` | Sim | Valeur JSON ou chaîne littérale écrite pour le transfert de paiement ou le hachage. |
| `payment_payer` | Sim | AccountId qui autorise le paiement. |
| `payment_signature` | Sim | JSON ou chaîne littérale prétend être la preuve de l'assistanat du steward ou du tesouraria. |
| `controllers` | Optionnel | Liste séparée par pont et virgula ou virgula de enderecos de conta contrôleur. Padrao `[owner]` lorsqu'il est omis. |
| `metadata` | Optionnel | JSON en ligne ou `@path/to/file.json` fournit des conseils de résolution, des enregistrements TXT, etc. Padrao `{}`. |
| `governance` | Optionnel | JSON en ligne ou `@path` posé pour `GovernanceHookV1`. `--require-governance` exige cette colonne. |Chaque colonne peut faire référence à un fichier externe avec le préfixe ou la valeur de la cellule avec `@`.
Les chemins sont résolus dans les relations avec l'archive CSV.

## 2. Exécuter ou aider

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Principes d'exploitation :

- `--require-governance` rejeita linhas sem um hook de gouvernance (utile pour
  leiloes premium ou atribuicoes reservadas).
- `--default-controllers {owner,none}` décide des cellules des contrôleurs vazias
  voltam para a conta propriétaire.
- Permis `--controllers-column`, `--metadata-column` et `--governance-column`
  renoear colunas opcionais ao trabalhar com exporte en amont.

En cas de réussite, le script est gravé dans un manifeste agrégé :

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
```

Se `--ndjson` fornecido, cada `RegisterNameRequestV1` tambem e escrito como
un document JSON de ligne unique pour que les demandes soient automatiquement transmises
directement à Torii :

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

### 3.1 Mode Torii REST

Spécifique `--submit-torii-url` mais `--submit-token` ou `--submit-token-file`
pour envoyer chaque entrée du manifeste directement à Torii :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- L'assistant émet un `POST /v1/sns/names` pour la demande et interrompt le premier
  erreur HTTP. En réponse à cette question, vous serez informé du journal como registros NDJSON.
- `--poll-status` reconsulta `/v1/sns/names/{namespace}/{literal}` apos cada envio
  (mangé `--poll-attempts`, par défaut 5) pour confirmer que l'enregistrement est visible.
  Forneca `--suffix-map` (JSON de `suffix_id` pour les valeurs "suffixe") pour qu'un
  ferramenta dérivent des documents `{label}.{suffix}` pour les sondages.
- Ajuste: `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### 3.2 Mode CLI iroha

Pour tourner chaque entrée du manifeste par la CLI, indiquez le chemin du binaire :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Les contrôleurs développent des entrées `Account` (`controller_type.kind = "Account"`)
  Par conséquent, une CLI est actuellement disponible pour exposer les contrôleurs en fonction du contenu.
- Blobs de métadonnées et de gouvernance gravés dans des archives temporaires sur demande
  et encaminhados para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout et stderr da CLI, mais les codes de la dite, sao registrados ; codigos nao
  zéro abortam a execucao.

Les modes de soumission peuvent être combinés (Torii et CLI) pour vérifier
les déploiements effectuent des sauvegardes d'enregistrement ou d'enregistrement.

### 3.3 Réceptions de soumission

Quand `--submission-log <path>` est fourni, le script annexe entre NDJSON que
capturer :

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Les réponses Torii bem-sucedidas incluent des champs estruturados extraidos de
`NameRecordV1` ou `RegisterNameResponseV1` (par exemple `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) pour les tableaux de bord et les rapports
de gouverner possam parsear o log sem inspecionar texto livre. Anexe ce journal
comme tickets de registrar junto com o manifeste para evidencia reproduzivel.

## 4. Portail de publication automatique

Emplois de CI et do portail chamam `docs/portal/scripts/sns_bulk_release.sh`, que
encapsula o helper e armazena artefatos sanglot
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

Ô scénario :

1. Construction `registrations.manifest.json`, `registrations.ndjson`, et copie du CSV
   original para o directoire de sortie.
2. Soumettez le manifeste en utilisant Torii et/ou une CLI (lorsque configuré), gravando
   `submissions.log` avec les recettes estruturados acima.
3. Émettez `summary.json` en décrivant la version (caminhos, URL do Torii, caminho da
   CLI, horodatage) pour que le portail automatique puisse envoyer le bundle pour ou
   stockage d'artefatos.
4. Produit `metrics.prom` (remplacement via `--metrics`) avec des compteurs compatibles
   avec Prometheus pour le total des demandes, la distribution des suffixes, le total des actifs
   et les résultats de la soumission. Le JSON du CV est déposé pour cet archivage.Les flux de travail sont simplement enregistrés ou dirigés vers un répertoire de publication comme un article unique,
qu'il y a tout ce qu'il faut pour gouverner précisément les auditoires.

## 5. Télémétrie et tableaux de bord

L'archive de mesures générée par `sns_bulk_release.sh` s'expose comme série suivante :

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` pas votre side-car de Prometheus (par exemple via Promtail ou
um importador batch) pour gérer les registraires, les stewards et les pares de gouvernance
alinhados sobre o progresso em massa. Le quadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualiser mes données avec paineis
para contagens por sufixo, volume de pagamento e ratios de sucesso/falha de
soumis. Le quadro filtre par `release` pour que les auditeurs puissent se concentrer sur eux
exécution unique de CSV.

## 6. Validation et modes de validité- **Canonisation du label :** entrées normales avec Python IDNA plus
  minuscules et filtres de caractères Norme v1. Étiquettes invalides Falham Rapido
  avant de tout chamada de rede.
- **Numéros de garde-corps :** identifiants de suffixe, années de mandat et conseils sur les prix
  entre les limites `u16` et `u8`. Champs de paiement avec nombre entier de décimales
  ous hex a mangé `i64::MAX`.
- **Analyse des métadonnées ou de la gouvernance :** JSON en ligne et analysé directement ;
  références aux archives sao résolues relatives à la localisation du CSV. Métadonnées
  Aucun objet ne produit une erreur de validation.
- **Contrôleurs :** cellules en blanc respectant `--default-controllers`. Fornéca
  listes explicites (par exemple `soraカタカナ...;soraカタカナ...`) au délégué des utilisateurs nao propriétaire.

Falhas sao reportadas com numeros de linha contextuais (par exemple
`error: row 12 term_years must be between 1 and 255`). O script sai com codigo
`1` dans les erreurs de validation et `2` lorsque le chemin CSV est activé.

## 7. Tests et procédure

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` analyse du fichier CSV,
  émis NDJSON, application de la gouvernance et chemins de soumission par CLI ou Torii.
- L'assistant et Python pur (sem dépendances supplémentaires) et le support à n'importe quel endroit
  onde `python3` estiver disponivel. L'historique des commits et rastreado junto
  une CLI n'est pas un dépôt principal pour la reproduction.Pour les opérations de production, l'annexe ou le manifeste généré et le bundle NDJSON avec le ticket
registraire pour que les stewards puissent reproduire les charges utiles exatos que foram
soumis à Torii.