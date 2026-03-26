---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/bulk_onboarding_toolkit.md` afin que les
operadores externos voient la meme guide SN-3b sans clonar le depot.
:::

# Kit de ferramentas de integração do maciço SNS (SN-3b)

**Roteiro de referência:** SN-3b "Ferramentas de integração em massa"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Os grandes registradores preparam o recebimento das centenas de registros `.sora` ou
`.nexus` com memes de aprovação de governo e trilhos de liquidação.
Crie payloads JSON para o principal ou relance a CLI sem escala, doc SN-3b
livre um construtor determinista CSV versão Norito que prepara estruturas
`RegisterNameRequestV1` para Torii ou CLI. L'helper valide cada linha em
amont, emet a la fois un manifeste agrege et du JSON delimitate par nouvelles
linhas opcionais, e você pode adicionar cargas úteis automaticamente a todos
registrador de estruturas de recusa para auditorias.

## 1. Esquema CSV

O analisador exige a seguinte linha de texto (a ordem é flexível):

| Coluna | Requisitos | Descrição |
|--------|--------|-------------|
| `label` | Oui | Libelle demande (casse mixte acceptee; l'outil normalize de acordo com Norm v1 e UTS-46). |
| `suffix_id` | Oui | Identificador numérico do sufixo (decimal ou `0x` hex). |
| `owner` | Oui | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Oui | Nível `1..=255`. |
| `payment_asset_id` | Oui | Ato de liquidação (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Oui | Entiers non signes representant des unites nativos do ativo. |
| `settlement_tx` | Oui | Valor JSON ou cadeia literária descritiva da transação de pagamento ou hash. |
| `payment_payer` | Oui | AccountId permite autorizar o pagamento. |
| `payment_signature` | Oui | JSON ou cadeia literária contém a pré-assinatura do administrador ou da tesouraria. |
| `controllers` | Opcional | Lista separada por ponto-virgule ou virgule des endereços de controlador de conta. Por padrão `[owner]` se omitido. |
| `metadata` | Opcional | JSON embutido ou `@path/to/file.json` fornece dicas de resolução, registros TXT, etc. Por padrão `{}`. |
| `governance` | Opcional | JSON embutido ou `@path` apontando para `GovernanceHookV1`. `--require-governance` impõe esta coluna. |

Todas as colunas podem referenciar um arquivo externo prefixando o valor da célula
par `@`. Os caminhos são resolvidos em arquivo CSV.

## 2. Executor e ajudante

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Arquivos de opções:

- `--require-governance` rejeita as linhas sem gancho de governo (utilizado para
  les encheres premium ou les afectations reservees).
- `--default-controllers {owner,none}` decidir si les cellules controllers vis
  retombent sur le compte proprietário.
- `--controllers-column`, `--metadata-column` e `--governance-column` permitidos
  de renommer les colonnes optionnelles lors d'exports amont.

Em caso de sucesso, o script escreveu um manifesto agregado:

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
```Se `--ndjson` é fornecido, cada `RegisterNameRequestV1` também é escrito como um
documento JSON em uma linha para que as automatizações possam ser transmitidas
receitas diretamente para Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Soumissions automatizados

### 3.1 Modo Torii REST

Especifique `--submit-torii-url` plus `--submit-token` ou `--submit-token-file` para
pressione cada entrada do manifesto diretamente para Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- O ajudante emet un `POST /v1/sns/names` par requete et s'arrete au premier
  erro HTTP. As respostas são adicionadas ao log como registros NDJSON.
- `--poll-status` re-interroge `/v1/sns/names/{namespace}/{literal}` após cada
  soumissão (apenas `--poll-attempts`, padrão 5) para confirmar que
  o registro está visível. Fournissez `--suffix-map` (JSON de `suffix_id`
  vers des valeurs "suffix") para que a utilidade derive dos literários
  `{label}.{suffix}` para pesquisa.
- Ajustáveis: `--submit-timeout`, `--poll-attempts` e `--poll-interval`.

### 3.2 Modo iroha CLI

Para fazer com que cada entrada do manifesto seja feita pela CLI, forneça o caminho do
binário:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Os controladores devem ter entradas `Account` (`controller_type.kind = "Account"`)
  Car la CLI expõe apenas bases de controladores nas contas.
- Os metadados e a governança dos blobs são escritos nos arquivos temporários
  receba e transmita para `iroha sns register --metadata-json ... --governance-json ...`.
- O stdout e o stderr da CLI, assim como os códigos de surtida são registrados;
  os códigos diferentes de zero interromperam a execução.

Les deux modes de soumission peuvent fonctionner ensemble (Torii et CLI) pour
cruzar as implantações do registrador ou repetir os substitutos.

### 3.3 Recus de soumission

Quando `--submission-log <path>` é fornecido, o script adicionado às entradas NDJSON
capturante:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

As respostas Torii reussies incluem extratos de estruturas de campeões de
`NameRecordV1` ou `RegisterNameResponseV1` (por exemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para que os painéis e os
relatórios de governo podem analisar o log sem inspecionar o texto livre.
Acesse este registrador de tickets com o manifesto para uma evidência
reproduzível.

## 4. Automatização de lançamento do portal

Les jobs CI e portal recorrente `docs/portal/scripts/sns_bulk_release.sh`, aqui
encapsular o ajudante e armazenar os artefatos sous
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

O script:1. Construa `registrations.manifest.json`, `registrations.ndjson` e copie-o
   CSV original no repertório de lançamento.
2. Envie o manifesto via Torii e/ou CLI (quando configurar), e crie-o
   `submissions.log` com estruturas recus ci-dessus.
3. Emet `summary.json` descritivo da liberação (caminhos, URL Torii, caminho CLI,
   timestamp) após a automação do portal poder fazer o upload do pacote versão
   le stockage d'artefacts.
4. Produto `metrics.prom` (substituir via `--metrics`) contendo os computadores
   no formato Prometheus para o total de solicitações, distribuição de sufixos,
   os totais de ativos e os resultados de submissão. Le JSON resume pointe vers
   este arquivo.

Os fluxos de trabalho arquivam simplesmente o repertório de lançamento como um único artefato,
qui contém desormais tout ce dont la gouvernance a besoin pour l'audit.

## 5. Telemetria e painéis

O arquivo de métricas gerado por `sns_bulk_release.sh` expõe as séries
seguintes:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injete `metrics.prom` em seu sidecar Prometheus (por exemplo, via Promtail ou
um lote de importação) para registradores de alinhadores, administradores e pares de governo em
avanço em massa. O quadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualize os memes dados com des
panneaux pour les comptes par suffixe, le volume de paiement et les ratios de
reussite/echec des soumissions. O filtro da tabela par `release` para que
os auditores podem se concentrar em um único CSV de execução.

## 6. Validação e modos de verificação

- **Normalização de rótulos:** as entradas são normalizadas com Python IDNA plus
  minúsculas e filtros de caracteres Norm v1. Les rótulos invalidos ecoam vite
  avant tout appel reseau.
- **Números importantes:** ids de sufixo, anos de mandato e dicas de preços fornecidas
  rester nos bornes `u16` e `u8`. Os campos de pagamento aceitam des
  inteiros decimais ou hexadecimais apenas `i64::MAX`.
- **Análise de metadados ou governança:** o JSON inline é analisado diretamente; os
  referências a arquivos são resoluções relativas à localização do CSV.
  Os metadados não objetos produzem um erro de validação.
- **Controladores:** as células vêem o respeito `--default-controllers`. Fournissez
  des listes explícitas (por exemplo `soraカタカナ...;soraカタカナ...`) quando você delega a des
  atores não proprietários.

Les echecs são sinalizados com números de linha contextuais (por exemplo
`error: row 12 term_years must be between 1 and 255`). O script classifica com ele
código `1` em erros de validação e `2` ao clicar no caminho CSV manualmente.

## 7. Testes e proveniência

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` permite a análise de CSV,
  a emissão NDJSON, a governança de aplicação e os caminhos de emissão CLI ou Torii.
- O ajudante é o Python pur (nenhuma adição de dependência) e parte do caminho
  ou `python3` está disponível. L'historique des commits est suivi aux cotes de la
  CLI no depósito principal para reprodução.Para as execuções de produção, junte-se ao gene manifesto e ao pacote NDJSON au
ticket du registrador para que os administradores possam refazer as cargas exatas
sou um Torii.