---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

##Obzor

Isso é feito para fornecer pontos de armazenamento **DOCS-7** (Publicação SoraFS) e **DOCS-8** (pino automático em CI/CD) em Um fornecedor prático para o portal de distribuição. Ao abrir o build/lint, instale SoraFS, verifique a configuração do Sigstore, prodвижение алиасов, проверку и тренировки отката, чтобы каждый preview and release были воспроизводимыми и аудируемыми.

Em seguida, verifique se você está usando o binário `sorafs_cli` (com `--features cli`), fornecido para o endpoint Torii правами pin-registry e OIDC учетные данные para Sigstore. Os segredos de segurança (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) são armazenados no CI vault; O software local pode ser exportado para shell.

## Uso antecipado

- Nó 18.18+ com `npm` ou `pnpm`.
- `sorafs_cli` ou `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii, который открывает `/v2/sorafs/*`, плюс учетная запись/приватный ключ, способные отправлять манифесты e алиасы.
- Emissor OIDC (GitHub Actions, GitLab, identidade de carga de trabalho, etc.) para usar `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para simulação e `docs/source/sorafs_ci_templates.md` para fluxos de trabalho GitHub/GitLab.
- Abra o teste OAuth Try it (`DOCS_OAUTH_*`) e use-o
  [lista de verificação de reforço de segurança](./security-hardening.md) foi fornecida pelo laboratório. Теперь сборка портала падает, если переменные отсутствуют ou TTL/пуллинг параметры выходят за допустимые окна; `DOCS_OAUTH_ALLOW_INSECURE=1` é usado para pré-visualização local. Прикрепляйте доказательства pen-test к релиз-таску.

## Шаг 0 - Зафиксировать bundle прокси Experimente

Para fornecer uma visualização no Netlify ou gateway, verifique isso Experimente proxy e resumo do OpenAPI манифеста в Pacote de determinação:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copia proxy/sonda/reversão de ajuda, verifique o OpenAPI e selecione `release.json` mais `checksums.sha256`. Прикрепите этот bundle к Netlify/SoraFS gateway ticket, чтобы ревьюеры могли воспроизвести точные исходники прокси и O alvo Torii não é compatível. O pacote também possui uma configuração, um cliente de token de portador (`allow_client_auth`), um plano de implementação e uma instalação CSP aprovada синхронизированными.

## Passo 1 - Build e lint portal

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` é um interruptor automático `scripts/write-checksums.mjs`, definido:

- `build/checksums.sha256` - SHA256 Manifesto para `sha256sum -c`.
-`build/release.json` - metadaнные (`tag`, `generated_at`, `source`) para carro/carro.

Архивируйте оба файла вместе с CAR summary, чтобы ревьюеры могли сравнивать preview артефакты без пересборки.

## Passo 2 - Упаковать статические ассеты

Compre o empacotador CAR no seu catálogo Docusaurus. Experimente uma nova imagem em seus artefatos em `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Resumo JSON фиксирует число чанков, дайджесты и подсказки для планирования prova, которые затем используют `manifest build` e CI painéis.

## Шаг 2b - Упаковать OpenAPI e companheiros SBOM

DOCS-7 требует публиковать сайт портала, snapshot OpenAPI e cargas úteis SBOM как отдельные манифесты, чтобы gateways могли Instale cabeçalhos `Sora-Proof`/`Sora-Content-CID` para o artefato. Release helper (`scripts/sorafs-pin-release.sh`) no pacote OpenAPI como (`static/openapi/`) e SBOM de `syft` em carros antigos `openapi.*`/`*-sbom.*` e metadados em `artifacts/sorafs/portal.additional_assets.json`. Em primeiro lugar, você pode usar 2-4 para cada carga útil com sua configuração e metadados (por exemplo, `--car-out "$OUT"/openapi.car` mais `--metadata alias_label=docs.sora.link/openapi`). Зарегистрируйте каждую пару manifest/alias em Torii (сайт, OpenAPI, portal SBOM, OpenAPI SBOM) para a configuração de DNS, Este gateway pode fornecer provas grampeadas para seus artefatos operacionais.

## Capítulo 3 - Manifesto de Tarefa

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Insira a bandeira de política de pins em uma nova versão (por exemplo, `--pin-storage-class hot` para canários). A variante JSON é opcional, não usada para revisão.

## Passo 4 - Подписать через Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Bundle фиксирует дайджест манифеста, дайджесты чанков и BLAKE3 хэш OIDC токена, не сохраняя JWT. Храните pacote e assinatura separada; A produção de produtos pode ser realizada por meio de uma verificação de seus artefatos em sua nova página. Локальные запуски могут заменить provedor флаги на `--identity-token-env` (ou задать `SIGSTORE_ID_TOKEN` em окружении), когда O auxiliar OIDC usa o token.

## Passo 5 - Abrir o registro de pinosAbra o manifesto (e o plano de blocos) em Torii. Para ver o resumo, o registro/alias é o nome do registro/alias que você ouviu.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Para visualização de lançamento ou alias canário (`docs-preview.sora`), você pode usar o alias exclusivo, o controle de qualidade pode fornecer conteúdo durante a promoção de produção.

Ligação de alias требует трех полей: `--alias-namespace`, `--alias-name` e `--alias-proof`. Governança fornece pacote de provas (base64 ou Norito bytes) после утверждения запроса alias; explique-o nos segredos do CI e verifique como ocorreu o `manifest submit`. Selecione alias флаги пустыми, если хотите только закрепить манифест без изменения DNS.

## Capítulo 5b - Proposta de governança do Сгенерировать

Каждый манифест должен сопровождаться proposta parlamentar, чтобы любой гражданин Sora мог внести изменение Não seja fornecido com um aplicativo privado. Para enviar/assinar você precisa:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` фиксирует каноническую инструкцию `RegisterPinManifest`, resumo чанков, política e dica de alias. Прикрепите его к tíquete de governança ou portal do Parlamento, чтобы делегаты могли сравнивать payload без пересборки. O comando não está usando a chave Torii, pois isso pode ser uma proposta local.

## Passo 6 - Prover provas e telemetria

O pino mais importante pode ser usado para determinar:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- Verifique `torii_sorafs_gateway_refusals_total` e `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Abra `npm run probe:portal`, verifique o proxy Try-It e verifique o conteúdo do seu site.
- Monitorar evidências de monitoramento de
  [Publicação e monitoramento](./publishing-monitoring.md), чтобы соблюсти portão de observabilidade DOCS-3c. Helper теперь принимает несколько `bindings` (сайт, OpenAPI, portal SBOM, OpenAPI SBOM) e требует `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` na parte traseira da proteção `hostname`. Você está procurando um conjunto de resumo JSON e pacote de evidências (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) na versão do catálogo:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Passo 6a - Gateway de autenticação de nuvem

Organize um plano TLS SAN/desafio para pacotes GAR, gateways de comando e aprovadores de DNS funcionando com segurança e evidências. Novos auxiliares incluem entradas DG-3, hosts curinga canônicos, SANs de host bonito, rótulos DNS-01 e desafios ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Comprometa o JSON com o pacote de lançamento (ou administre o ticket de alteração), o operador pode instalar o SAN na configuração `torii.sorafs_gateway.acme`, um revisor GAR pode fornecer mapeamentos canônicos/pretty sem alterações. Use o `--name` para obter um número suficiente em uma versão adequada.

## Passo 6b - Mapeamentos de host canônicos polivalentes

Antes de criar cargas úteis GAR, determine o mapeamento de host para o alias. `cargo xtask soradns-hosts` хеширует каждый `--name` em rótulo canônico (`<base32>.gw.sora.id`), эмитит нужный curinga (`*.gw.sora.id`) e выводит pretty host (`<alias>.gw.sora.name`). Сохраняйте вывод в artefatos de lançamento, чтобы DG-3 revisores podem mapear рядом с submissão GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>`, este é o padrão que GAR ou JSON de ligação de gateway não é compatível com isso. Ajudante configurou nenhuma configuração, usando o modelo GAR e o modelo GAR, e `portal.gateway.binding.json` no comando anterior:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Escreva o resumo JSON e o log de verificação para o ticket de alteração de DNS/gateway, para que os auditores possam fornecer hosts canônicos/wildcard/pretty que não sejam usados. Перезапускайте команду при добавлении новых alias, as atualizações do GAR contêm evidências.

## Passo 7 - Descritor de substituição de DNS do gerenciador

Cortes de produção требуют аудируемого pacote de mudança. После успешной auxiliar de submissão (ligação de alias) генерирует `artifacts/sorafs/portal.dns-cutover.json`, фиксируя:- ligação de alias de metadados (namespace/nome/prova, resumo do manifesto, URL Torii, época submetida, autoridade);
- contexto de liberação (tag, rótulo de alias, manifesto/CAR пути, plano de bloco, pacote Sigstore);
- ponteiros de verificação (comando de teste, alias + endpoint Torii); e
- política de controle de alterações opcional (id do ticket, janela de transição, contato de operações, nome do host/zona de produção);
- promoção de rota de metadados no cabeçalho `Sora-Route-Binding` (host/CID canônico, cabeçalho пути + ligação, comandos проверки), promoção GAR чтобы e exercícios de fallback ссылались на одну evidências;
- Artefatos de plano de rota de gerenciamento (`gateway.route_plan.json`, modelos de cabeçalho e cabeçalhos de reversão opcionais), tíquetes de alteração de чтобы e ganchos de fiapos CI могли проверить, что каждый pacote DG-3 ссылается em planos canônicos de promoção/reversão;
- metadados opcionais para invalidação de cache (endpoint de limpeza, variável de autenticação, carga útil JSON e exemplo `curl`);
- dicas de reversão no descritor pré-definido (tag de lançamento e resumo do manifesto), que alteram tickets фиксировали детерминированный fallback.

Ou seja, liberar a limpeza de cache, сгенерируйте канонический plano вместе с descritor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Прикрепите `portal.cache_plan.json` no pacote DG-3, este operador irá determinar hosts/caminhos (e fornecer dicas de autenticação) por `PURGE` é desbloqueado. O descritor de seção de cache opcional pode ser selecionado neste arquivo, seus revisores de controle de alterações são exibidos, como pontos de extremidade são usados ​​​​por transição.

O pacote Каждый DG-3 também inclui promoção + lista de verificação de reversão. Сгенерируйте его через `cargo xtask soradns-route-plan`, чтобы revisores de controle de alterações podem usar o preflight/cutover/rollback шаги для каждого alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` фиксирует hosts canônicos/bonitos, lembretes de verificação de integridade encenados, atualizações de ligação GAR, limpezas de cache e ações de reversão. Прикладывайте его вместе с GAR/binding/cutover артефактами до подачи change ticket, чтобы Ops могли репетировать те же шаги.

Descritor de formato `scripts/generate-dns-cutover-plan.mjs` e identificado por `sorafs-pin-release.sh`. Para a geração de energia:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Selecione os metadados opcionais para definir o pin helper:

| Período | Atualizado |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID do ticket, código de identificação no descritor. |
| `DNS_CUTOVER_WINDOW` | Corte padrão ISO8601 (por exemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nome do host de produção + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias ​​de plantão ou contato de escalonamento. |
| `DNS_CACHE_PURGE_ENDPOINT` | Purge endpoint, descrito no descritor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var com token de limpeza (padrão: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Coloque o descritor de transição para metadados de reversão. |

Escreva JSON para revisão de alterações de DNS, aprovadores de arquivos podem fornecer resumo de manifesto, ligações de alias e comandos de sonda para processar logs de CI. CLI tags `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`, `--cache-purge-auth-env` e `--previous-dns-plan` fornece uma substituição para o auxiliar do CI.

## Passo 8 - Esqueleto do arquivo de zona do resolvedor de configuração (опционально)

Para abrir a janela de transição de produção, o script de liberação pode ser gerado automaticamente pelo esqueleto do arquivo de zona SNS e pelo snippet do resolvedor. Transfira novas descrições de DNS e metadados para env ou CLI; helper usa `scripts/sns_zonefile_skeleton.py` para gerar o descritor de transição. Use o mínimo de valor A/AAAA/CNAME e GAR digest (BLAKE3-256 contém carga útil GAR). A zona/hostname é definida e `--dns-zonefile-out` é proposta, a opção auxiliar está em `artifacts/sns/zonefiles/<zone>/<hostname>.json` e fornece `ops/soradns/static_zones.<hostname>.json` como trecho de resolução.| Переменная / bandeira | Atualizado |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Coloque o esqueleto do arquivo de zona para gerar o esqueleto. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Coloque o snippet do resolvedor (por `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL para atualização (padrão: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Endereço IPv4 (env separado por vírgula ou sinalizador повторяемый). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Endereço IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Destino CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinos SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Adicione uma mensagem TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Substituir o rótulo da versão do arquivo de zona computado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Use o carimbo de data/hora `effective_at` (RFC3339) para abrir a janela de transição. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Substituir o literal de prova nos metadados. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Substituir o CID pelos metadados. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelamento do Guardian (suave, duro, descongelamento, monitoramento, emergência). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referência do bilhete do guardião/conselho para congelar. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Carimbo de data e hora RFC3339 para descongelamento. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Дополнительные congelar notas (env separado por vírgula ou повторяемый флаг). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 digest (hex) contém carga útil GAR. Verifique as ligações de gateway. |

Fluxo de trabalho GitHub Actions é definido como um repositório de segredos que contém artefatos de zonefile de produção. Настройте следующие segredos (строки могут содержать listas separadas por vírgula para pólos de vários valores):

| Segredo | Atualizado |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nome do host/zona de produção para o helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de plantão no descritor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Identificações IPv4/IPv6 para publicação. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Destino CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pinos SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные TXT записи. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Congelar metadados no esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex BLAKE3 digest подписанного carga útil GAR. |

Ao usar `.github/workflows/docs-portal-sorafs-pin.yml`, você precisa das entradas `dns_change_ticket` e `dns_cutover_window`, os descritores/arquivo de zona não estão corretos nos metadados ok. Coloque-o no lugar para o funcionamento a seco.

Tipo de comando (do proprietário do runbook SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...other flags...
```

Helper автоматически переносит change ticket em TXT запись и наследует начало cutover window как `effective_at`, если не указано иное. Fluxo de trabalho operacional Полный см. em `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Limpeza de DNS-делегации

O arquivo de zona de zoneamento é usado para registrar zonas automaticamente. Delegacia NS/DS
родительской зоны нужно настроить у регистратора ou DNS-Provaйдера, чтобы
Você pode usar a Internet para acessar seu servidor de nomes.

- Para a transferência para apex/TLD use ALIAS/ANAME (usado pelo provedor) ou
  publique as letras A/AAAA, use o gateway anycast-IP.
- Para poder publicar CNAME em um host bonito derivado
  (`<fqdn>.gw.sora.name`).
- Канонический хост (`<hash>.gw.sora.id`) colocado no gateway doméstico e não
  publique em sua zona pública.

### Gateway Shablon заголовков

Deploy helper como генерирует `portal.gateway.headers.txt` e `portal.gateway.binding.json` - sua arte, удовлетворяющие DG-3 требованиям gateway-content-binding:

- `portal.gateway.headers.txt` contém um bloco de cabeçalhos HTTP (como `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS e `Sora-Route-Binding`), O Edge Gateway deve ser configurado para ser aberto.
- `portal.gateway.binding.json` фиксирует ту же информацию в машиночитаемом виде, чтобы change tickets и автоматизация могли сравнивать As ligações host/cid não estão disponíveis no shell de análise.

Ele é gerado por `cargo xtask soradns-binding-template` e alias de фиксируют, resumo de manifesto e nome de host do gateway, que foi transferido para `sorafs-pin-release.sh`. Para regenerar ou converter cabeçalhos de bloco, você precisará:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Передайте `--csp-template`, `--permissions-template` ou `--hsts-template`, чтобы переопределить шаблоны заголовков; use-o no cabeçalho `--no-*` para o cabeçalho de atualização.

Ao inserir o snippet na solicitação de alteração do CDN e transferir o JSON para o pipeline de automação do gateway, você obterá evidências reais.

Liberar script автоматически запускает auxiliar de verificação, чтобы DG-3 tickets всегда содержали свежую evidências. Execute-o corretamente, mas certifique-se de vincular JSON diretamente:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```O comando decodificar `Sora-Proof` grampeado, fornecerá `Sora-Route-Binding` com manifesto CID + nome do host e definirá os cabeçalhos dos dados. Архивируйте вывод вместе с другими release artefatos при запуске вне CI, чтобы DG-3 revisores имели доказательство проверки.

> **Интеграция DNS descriptor:** `portal.dns-cutover.json` теперь включает секцию `gateway_binding`, указывающую на эти артефакты (пути, content CID, prova de status e modelo de cabeçalho literal) **e** seção `route_plan`, seleção de `gateway.route_plan.json` e modelo de recuperação/reversão. Включайте эти блоки в каждый DG-3 change ticket, чтобы revisores могли сравнить точные значения `Sora-Name/Sora-Proof/CSP` и подтвердить соответствие планов pacote de evidências de promoção/reversão без открытия build архива.

## Capítulo 9 - Monitores de publicação atualizados

Item de roteiro **DOCS-3c** требует постоянных доказательств, что портал, Try it proxy e gateway bindings остаются здоровыми после релиза. Abra o monitor consolidado nos dias 7 a 8 e faça suas sondagens agendadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` configura a configuração (como `docs/portal/docs/devportal/publishing-monitoring.md` para o esquema) e usa três testes: probes путей портала + CSP/validação de política de permissões, experimente proxy probes (опционально `/metrics`), e verificador de ligação de gateway (`cargo xtask soradns-verify-binding`), который теперь требует наличие e ожидаемое значение Sora-Content-CID вместе с alias/manifest проверками.
- Команда завершается с diferente de zero, когда любой probe падает, чтобы CI/cron/runbook операторы могли остановить релиз до alias de produção.
- Флаг `--json-out` пишет единый summary JSON по целям; `--evidence-dir` пишет `summary.json`, `portal.json`, `tryit.json`, `binding.json` e `checksums.sha256`, quais revisores de governança podem ser encontrados Os resultados obtidos não serão monitorados automaticamente. Arquive este catálogo para `artifacts/sorafs/<tag>/monitoring/` com o pacote Sigstore e o descritor de transição de DNS.
- Включайте вывод мониторинга, Grafana export (`dashboards/grafana/docs_portal.json`) e Alertmanager drill ID no ticket de liberação, чтобы DOCS-3c SLO можно было аудировать por favor. Manual de monitorização publicado em `docs/portal/docs/devportal/publishing-monitoring.md`.

Portal probes требуют HTTPS и отклоняют `http://` базовые URL, если не задан `allowInsecureHttp` в конфиге monitor; Selecione produção/preparação para TLS e use override para visualização local.

O monitor automático é `npm run monitor:publishing` no Buildkite/cron para abrir o portal. Para isso, acesse URLs de produção, verifique se as verificações de integridade estão disponíveis.

## Автоматизация через `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` инкапсулирует шаги 2-6. Sobre:

1. Instale `build/` no tarball determinado,
2. Selecione `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` e `proof verify`,
3. Use `manifest submit` (ligação de alias) usando credenciais Torii, e
4. Escolha `artifacts/sorafs/portal.pin.report.json`, опциональный `portal.pin.proposal.json`, descritor de transição de DNS (submissão) e pacote de ligação de gateway (`portal.gateway.binding.json` mais bloco de cabeçalho), parâmetros governança/rede/ops Os comandos podem fornecer evidências sem os logotipos do CI.

Antes de usar, instale `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` e (опционально) `PIN_ALIAS_PROOF_PATH`. Use `--skip-submit` para funcionamento a seco; O fluxo de trabalho do GitHub não é compatível com esta entrada `perform_submit`.

## Passo 8 - Publicação de especificações OpenAPI e pacotes SBOM

O documento DOCS-7, este portal, a especificação OpenAPI e os artefatos SBOM fornecem o mesmo e determinam o pipeline. Ajudantes de ajuda ajudam você a fazer isso:

1. **Especificações de especificação e especificação.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Use `--version=<label>` para obter um instantâneo histórico (por exemplo, `2025-q3`). O helper captura snapshot em `static/openapi/versions/<label>/torii.json`, registra em `versions/current` e armazena metadados (SHA-256, status de manifesto, carimbo de data/hora atualizado) em `static/openapi/versions.json`. O portal usa esse índice para sua versão e abre o resumo/assinatura. O `--version` não é compatível, basta usar rótulos e rótulos `current` + `latest`.

   Manifest фиксирует SHA-256/BLAKE3 digests, чтобы gateway мог приклеивать `Sora-Proof` para `/reference/torii-swagger`.

2. **Configurar SBOMs CycloneDX.** Pipeline de lançamento para criar SBOMs baseados em syft como `docs/source/sorafs_release_pipeline_plan.md`. Você pode criar artefatos de construção:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Transfira a carga útil para o CAR.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```Следуйте тем же шагам `manifest build`/`manifest sign`, что и для основного сайта, задавая отдельные alias на artefato (por exemplo, `docs-openapi.sora` para especificações e `docs-sbom.sora` para suporte SBOM). O alias alternativo fornece SoraDNS, GAR e tickets de reversão para fornecer carga útil conjunta.

4. **Enviar e vincular.** Selecione a autoridade da autoridade e o pacote Sigstore, sem definir a tupla de alias na lista de verificação de liberação, чтобы аудиторы знали, какой Nome Sora соответствует какому resumo.

Архивация spec/SBOM манифестов вместе с build портала гарантирует, что каждый release ticket содержит полный набор артефактов Não é um empacotador de embalagem confiável.

### Auxiliar de automação (script de CI/pacote)

`./ci/package_docs_portal_sorafs.sh` código шаги 1-8, item do roteiro чтобы **DOCS-7** можно было выполнить одной коmanдой. Ajudante:

- выполняет подготовку портала (`npm ci`, OpenAPI/Norito sincronização, testes de widget);
- formar CARs e pares de manifestos para portal, OpenAPI e SBOM через `sorafs_cli`;
- опционально выполняет `sorafs_cli proof verify` (`--proof`) e assinatura Sigstore (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- classifique seus artefatos em `artifacts/devportal/sorafs/<timestamp>/` e selecione `package_summary.json` para ferramentas de CI/liberação; e
- обновляет `artifacts/devportal/sorafs/latest` на последний запуск.

Exemplo (pipeline com Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Bandeiras polonesas:

- `--out <dir>` - substituir artefactos de cor (por meio de catálogo com carimbo de data/hora).
- `--skip-build` - использовать существующий `docs/portal/build` (espelhos offline usados).
- `--skip-sync-openapi` - пропустить `npm run sync-openapi`, que `cargo xtask openapi` não pode ser fornecido para crates.io.
- `--skip-sbom` - não вызывать `syft`, если бинарь не установлен (скрипт печатает предупреждение).
- `--proof` - выполнить `sorafs_cli proof verify` para o carro/manifesto. Cargas úteis multifacetadas oferecem suporte ao plano de bloco na CLI, para que você possa usar esta opção por meio de download `plan chunk count` e verifique o portão de entrada.
- `--sign` - вызвать `sorafs_cli manifest sign`. Use o token `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) para usar o CLI получить e o `--sigstore-provider/--sigstore-audience`.

Para os artefactos de produção são utilizados `docs/portal/scripts/sorafs-pin-release.sh`. No pacote do portal, OpenAPI e SBOM, insira o manifesto do arquivo e insira os metadados no `portal.additional_assets.json`. Ajudante понимает те же botões, что e CI packager, mais novos `--openapi-*`, `--portal-sbom-*` e `--openapi-sbom-*` para настройки alias tuplas, substituir fonte SBOM через `--openapi-sbom-source`, fornece cargas úteis adicionais (`--skip-openapi`/`--skip-sbom`) e указания нестандартного `syft` через `--syft-bin`.

Скрипт выводит все команды; скопируйте лог в release ticket вместе с `package_summary.json`, чтобы ревьюеры могли сверять CAR digests, plan metadata e Sigstore bundle hashes bez Prosmotra случайного shell вывода.

## Passo 9 - Gateway de teste + SoraDNS

Antes de usar o cutover убедитесь, este novo alias резолвится через SoraDNS e gateways fornece suas provas:

1. **Iniciar porta da sonda.** `ci/check_sorafs_gateway_probe.sh` usa `cargo xtask sorafs-gateway-probe` em luminárias de demonstração em `fixtures/sorafs_gateway/probe_demo/`. Para realmente instalar o nome do host de destino:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Probe декодирует `Sora-Name`, `Sora-Proof` e `Sora-Proof-Status` para `docs/source/sorafs_alias_policy.md` e падает при расхождении resumo манифеста, TTL ou Ligações GAR.

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **Evidências para perfuração.** Para exercícios de operador ou ensaios a seco do PagerDuty são fornecidos `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. O wrapper armazena cabeçalhos/logs em `artifacts/sorafs_gateway_probe/<stamp>/`, usa `ops/drill-log.md` e usa potencialmente ganchos de rollback ou cargas úteis do PagerDuty. Use `--host docs.sora`, isso prova o SoraDNS sem IP.3. **Verifique as ligações DNS.** A governança pública publica a prova de alias, solicita o arquivo GAR, verifica o teste (`--gar`) e o registra para liberar evidências. Proprietários de resolvedores podem programar o que você precisa para `tools/soradns-resolver`, isso é necessário, o que é necessário para obter informações importantes nova manifestação. Ao usar JSON como `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`, você determina mapeamento de host, manipulação de metadados e rótulos de telemetria. offline. Helper может сформировать `--json-out` resumo рядом с подписанным GAR.
  Para obter o novo GAR, use `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (configurado para `--manifest-cid <cid>` por meio do manifesto aberto fala). Helper теперь выводит CID **и** BLAKE3 digest прямо из manifest JSON, удаляет пробелы, дедуплицирует повторяющиеся `--telemetry-label`, сортирует rótulos и Escolha os modelos CSP/HSTS/Permissions-Policy padrão antes de gerar JSON, para que a carga útil seja determinada.

4. **Apelido de Следить за метриками.** Definido na tela `torii_sorafs_alias_cache_refresh_duration_ms` e `torii_sorafs_gateway_refusals_total{profile="docs"}`; A série está em `dashboards/grafana/docs_portal.json`.

## Capítulo 10 - Monitoramento e agrupamento de evidências

- **Dashboards.** Exiba `dashboards/grafana/docs_portal.json` (portal SLO), `dashboards/grafana/sorafs_gateway_observability.json` (gateway de latência + prova de integridade) e `dashboards/grafana/sorafs_fetch_observability.json` (integridade do orquestrador) para obter segurança. Selecione o transporte JSON para liberar ticket.
- **Arquivos de teste.** Coloque `artifacts/sorafs_gateway_probe/<stamp>/` no git-annex ou no balde de evidências. Veja o resumo do probe, cabeçalhos e carga útil do PagerDuty.
- **Liberar pacote.** Сохраняйте CAR resumo portal/SBOM/OpenAPI, pacotes de manifesto, assinaturas Sigstore, `portal.pin.report.json`, logs de sonda Try-It e relatórios de verificação de link no pacote adicional (por exemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Registro de perfuração.** Sondas de teste - часть broca, `scripts/telemetry/run_sorafs_gateway_probe.sh` добавляет запись в `ops/drill-log.md`, чтобы это покрывало SNNet-5 caos requisito.
- **Links de ingressos.** Pesquisar IDs de painel Grafana ou configurar exportações PNG em alteração de ticket com o relatório de sondagem, чтобы ревьюеры могли SLOs seguros não são fornecidos pelo shell.

## Capítulo 11 - Exercício de busca de múltiplas fontes e evidência de placar

Publicado em SoraFS, você deve usar evidências de busca de várias fontes (DOCS-7/SF-6) com provas de DNS/gateway. После pin манифеста:

1. **Instale `sorafs_fetch` na manifestação ao vivo.** Insira seus artefatos de plano/manifesto nas etapas 2-3 e credenciais de gateway para o caso provador. Сохраняйте все выходные данные, чтобы аудиторы могли повторить решение orquestrador:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Selecione anúncios de provedores de pesquisa, envie-os para o manifesto (por exemplo, `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) e atualize-os para `--provider-advert name=path`, чтобы placar оценивал окна возможностей детерминированно. `--allow-implicit-provider-metadata` é usado **только** para CI фикстур; brocas de produção должны ссылаться на подписанные anúncios из pin.
   - При наличии дополнительных регионов повторите команду соответствующими provedor tuplas, чтобы каждый cache/alias имел связанный fetch артефакт.

2. **Reserve seus dados.** Solicite `scoreboard.json`, `providers.ndjson`, `fetch.json` e `chunk_receipts.ndjson` no catálogo de evidências релиза. Ele fornece ponderação de pares, orçamento de novas tentativas, latência EWMA e recebimentos de pedaços para SF-7.

3. **Ativar telemetria.** Importar saídas de busca no painel **SoraFS Observabilidade de busca** (`dashboards/grafana/sorafs_fetch_observability.json`), localizado em `torii_sorafs_fetch_duration_ms`/`_failures_total` e painéis para intervalos de provedores. Прикладывайте Grafana snapshots к release ticket вместе с путем к scoreboard.

4. **Verifique as regras de alerta.** Abra `scripts/telemetry/test_sorafs_fetch_alerts.sh`, verifique o pacote de alerta Prometheus para obter a confiança. Selecione a saída do promtool, os revisores do DOCS-7 podem ser ativados, que ativam alertas de provedor lento/parado.

5. **Configurar no CI.** Fluxo de trabalho do pino do portal definido `sorafs_fetch` para entrada `perform_fetch_probe`; Quando você está interessado em preparação/produção, você busca evidências para obter o pacote de manifesto. Os exercícios locais podem ser usados ​​para o script, tokens de gateway de transferência e o nome `PIN_FETCH_PROVIDERS` (código de teste separado por vírgula).

## Promoção, observabilidade e reversão1. **Promoção:** держите отдельные pseudônimo de preparação e produção. Produza, usando `manifest submit` como manifesto/pacote, chamado `--alias-namespace/--alias-name` como alias de produção. Este é um teste de qualidade / controle de qualidade para aprovação do controle de qualidade.
2. **Monitoramento:** use o registro de pinos do painel (`docs/source/grafana_sorafs_pin_registry.json`) e sondas de portal (`docs/portal/docs/devportal/observability.md`). Verifique o desvio da soma de verificação, testes com falha ou picos de prova de nova tentativa.
3. **Reversão:** para remover o manifesto anterior (ou aposentar o alias do текущий) com `sorafs_cli manifest submit --alias ... --retire`. Para obter um pacote em bom estado e um resumo do CAR, essas provas de reversão podem ser usadas para gerar logs de CI.

## Fluxo de trabalho do Shablon CI

Pipeline mínimo usado:

1. Build + lint (`npm ci`, `npm run build`, somas de verificação de geração).
2. Упаковка (`car pack`) e manifesto вычисление.
3. Selecione o token OIDC com escopo de trabalho (`manifest sign`).
4. Загрузка артефактов (CAR, manifesto, pacote, plano, resumos) para auditoria.
5. Abrir o registro de pinos:
   - Solicitações pull -> `docs-preview.sora`.
   - Tags/ramos protegidos -> alias de produção de promoção.
6. Sondas de teste + portões de verificação de prova são usados.

`.github/workflows/docs-portal-sorafs-pin.yml` соединяет эти шаги para liberações manuais. Fluxo de trabalho:

- construir/testar portal,
- упаковка build через `scripts/sorafs-pin-release.sh`,
- подпись/проверка manifest bundle через GitHub OIDC,
- загрузка CAR/manifesto/pacote/plano/resumos de prova como artefatos, и
- (опционально) отправка manifest + alias binding при наличии segredos.

Verifique os segredos/variáveis do repositório antes de executar o trabalho:

| Nome | Atualizado |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii com `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época, записанный при submissão. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Подписывающая autoridade para submissão манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias, привязываемый при `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Pacote de prova de alias Base64 (опционально). |
| `DOCS_ANALYTICS_*` | Verifique endpoints de análise/sonda. |

Запускайте fluxo de trabalho da UI de ações:

1. Selecione `alias_label` (por exemplo, `docs.sora.link`), `proposal_alias` opcional e `release_tag` opcional.
2. Instale o `perform_submit` para a geração de artefatos usando o Torii (funcionamento a seco) ou para o публикации на настроенный alias.

`docs/source/sorafs_ci_templates.md` описывает общие CI helpers para seus projetos, não para o fluxo de trabalho do portal de uso.

## Verifique

- [ ] `npm run build`, `npm run test:*` e `npm run check:links` são fornecidos.
- [ ] `build/checksums.sha256` e `build/release.json` são usados ​​em artefatos.
- [ ] CAR, plano, manifesto e resumo сгенерированы под `artifacts/`.
- [ ] Pacote Sigstore + assinatura desanexada сохранены с логами.
- [ ] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json` сохранены при submissão.
- [ ] `portal.pin.report.json` (e opcional `portal.pin.proposal.json`) архивированы рядом с CAR/manifest artefatos.
- [ ] Os registros `proof verify` e `manifest verify-signature` são válidos.
- [ ] Painéis Grafana обновлены + Try-It probes успешны.
- [ ] Notas de reversão (ID предыдущего манифеста + alias digest) приложены к release ticket.