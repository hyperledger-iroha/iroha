---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Visão geral

Este playbook converte os itens do roadmap **DOCS-7** (publicação de SoraFS) e **DOCS-8**
(automática de pin de CI/CD) em um procedimento acionavel para o portal de desenvolvedores.
Cobre a fase de build/lint, o empacotamento SoraFS, a assinatura de manifestos com Sigstore,
a promoção de alias, a verificação e os exercícios de rollback para que cada preview e release
seja reproduzível e auditável.

O fluxo assume que você tem o binário `sorafs_cli` (construído com `--features cli`), acesso a
um endpoint Torii com permissões de registro de pin, e credenciais OIDC para Sigstore. Guarde segredos
de longa duração (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) em seu vault de CI; como
execuções locais podem carregar-los a partir de exportações do shell.

## Pré-requisitos

- Nó 18.18+ com `npm` ou `pnpm`.
- `sorafs_cli` a partir de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que expõe `/v1/sorafs/*` mais uma conta/chave privada de autoridade que pode enviar manifestos e alias.
- Emissor OIDC (GitHub Actions, GitLab, identidade de carga de trabalho, etc.) para emitir um `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para dre runs e `docs/source/sorafs_ci_templates.md` para scaffolding de fluxos de trabalho do GitHub/GitLab.
- Configure as variáveis OAuth de Tre it (`DOCS_OAUTH_*`) e execute um
 [lista de verificação de reforço de segurança](./security-hardening.md) antes de promover um build
 fora do laboratório. O build do portal agora falha quando essas variáveis faltam
 o quando os botões de TTL/polling quedan fuera de as ventanas aplicadas; exportar
 `DOCS_OAUTH_ALLOW_INSECURE=1` apenas para visualizações de locais desechables. Anexo a
 evidência de pen-test no ticket de lançamento.

## Etapa 0 - Captura de um pacote de proximidade Tre it

Antes de promover uma prévia na Netlife ou no gateway, venda as fontes do próximo Tre it e o
resumo do manifesto OpenAPI firmado em um pacote determinista:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copie os auxiliares de proxy/sonda/reversão, verifique a assinatura
OpenAPI e escrevo `release.json` mas `checksums.sha256`. Anexo este pacote ao ticket de
promoção de Netlify/SoraFS gateway para que os revisores possam reproduzir as fontes exatas
del proxe e as pistas do alvo Torii sem reconstruir. O pacote também registra si os
bearers provistos para o cliente estaban habilitados (`allow_client_auth`) para manter o plano
de rollout e as regras CSP em sincronização.

## Etapa 1 - Construir e criar o portal

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

`npm run build` executa automaticamente `scripts/write-checksums.mjs`, produzindo:

- `build/checksums.sha256` - manifesto SHA256 adequado para `sha256sum -c`.
- `build/release.json` - metadados (`tag`, `generated_at`, `source`) fixados em cada CAR/manifesto.

Arquive ambos os arquivos junto com o resumo CAR para que os revisores possam comparar artefatos de
preview sem reconstruir.

## Etapa 2 - Empaque os ativos estaticos

Execute o empacotamento CAR contra o diretório de saída de Docusaurus. O exemplo de baixo
escreva todos os artefatos abaixo de `artifacts/devportal/`.

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

O resumo JSON captura conteúdo de chunks, digests e pistas de planejamento de prova que
`manifest build` e os dashboards de CI reutilizados depois.

## Etapa 2b - Empaqueta acompanhantes OpenAPI e SBOM

DOCS-7 requer a publicação do site do portal, do snapshot OpenAPI e dos payloads SBOM
como manifestos diferentes para que os gateways possam gravar os cabeçalhos
`Sora-Proof`/`Sora-Content-CID` para cada artefato. Ó ajudante de liberação
(`scripts/sorafs-pin-release.sh`) você embala o diretório OpenAPI
(`static/openapi/`) e os SBOMs emitidos via `syft` em CARs separados
`openapi.*`/`*-sbom.*` e registra os metadados em
`artifacts/sorafs/portal.additional_assets.json`. Ao executar o fluxo manual,
repita as etapas 2-4 para cada carga útil com seus próprios prefixos e etiquetas de metadados
(por exemplo `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Cadastre-se por manifesto/alias
em Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) antes de alterar DNS para que
o gatewae pode experimentar engrapadas para servir todos os artefatos publicados.

## Etapa 3 - Construção do manifesto

```bash
sorafs_cli manifest build \
 --summare "$OUT"/portal.car.json \
 --manifest-out "$OUT"/portal.manifest.to \
 --manifest-json-out "$OUT"/portal.manifest.json \
 --pin-min-replicas 5 \
 --pin-storage-class warm \
 --pin-retention-epoch 14 \
 --metadata alias_label=docs.sora.link
```

Ajusta as bandeiras de política de pin segun tu ventana de release (por exemplo, `--pin-storage-class
quente` para canários). Uma variante JSON é opcional, mas conveniente para revisão de código.

## Etapa 4 - Assinatura com Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```O bundle registra o resumo do manifesto, os resumos de chunks e um hash BLAKE3 do token
OIDC sem persistir no JWT. Guarda tanto o pacote como a assinatura separada; como promoções de
a produção pode reutilizar os mesmos artefatos em vez de reassinar. As execuções locais
podem substituir as bandeiras do provedor com `--identity-token-env` (ou estabelecer
`SIGSTORE_ID_TOKEN` em o ambiente) quando um helper OIDC externo emite o token.

## Etapa 5 - Envie o registro pin

Envie o manifesto firmado (e o plano de pedaços) para Torii. Solicite sempre um resumo para que
a entrada/alias resultante seja auditável.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite <i105-account-id> \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

Quando se despliega um alias de preview o canare (`docs-preview.sora`), repita o
envio com um alias único para que o QA possa verificar o conteúdo antes de uma promoção a
produção.

A ligação do alias requer três campos: `--alias-namespace`, `--alias-name` e `--alias-proof`.
A governança produz o pacote de prova (base64 ou bytes Norito) quando se solicita uma solicitação
do apelido; guarde-o em segredos de CI e exponha-o como arquivo antes de invocar `manifest submit`.
Deixe os sinalizadores de alias sem serem estabelecidos quando apenas piensas firmarem o manifesto sem tocar DNS.

## Etapa 5b - Gerando uma proposta de governança

Cada manifesto deve viajar com uma lista proposta para o Parlamento para que qualquer cidadão
Sora possa introduzir a mudança sem pedir credenciais privilegiadas. Depois dos passos
enviar/assinar, executar:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura uma instrução canônica `RegisterPinManifest`,
o digest de chunks, a politica e a pista de alias. Adjuntalo ao ticket de governança ou al
portal Parlamento para que os delegados possam comparar o payload sem reconstruir os artefatos.
Como o comando nunca toca a chave de autoridade de Torii, qualquer cidadão pode redigir a
propuesta localmente.

## Etapa 6 - Verificação de provas e telemetria

Depois de fijar, execute as etapas de verificação determinista:

```bash
sorafs_cli proof verife \
 --manifest "$OUT"/portal.manifest.to \
 --car "$OUT"/portal.car \
 --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
 --manifest "$OUT"/portal.manifest.to \
 --bundle "$OUT"/portal.manifest.bundle.json \
 --chunk-plan "$OUT"/portal.plan.json
```

- Verifique `torii_sorafs_gateway_refusals_total` e
 `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalias.
- Execute `npm run probe:portal` para ejetar o proxy Try-It e os links registrados
 contra o conteúdo recebido.
- Captura a evidência de monitoramento descrito em
 [Publicação e monitoramento](./publishing-monitoring.md) para que a porta de observação DOCS-3c
 se satisfaga junto com as etapas de publicação. O helper agora aceita entradas múltiplas
 `bindings` (site, OpenAPI, SBOM do portal, SBOM de OpenAPI) e aplicativo `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 em o host objetivo via o guarda opcional `hostname`. A invocação de baixo descreve tanto um
 resumo JSON único como o pacote de evidências (`portal.json`, `tryit.json`, `binding.json` e
 `checksums.sha256`) abaixo do diretório de lançamento:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a - Planejando certificados do gateway

Deriva o plano de SAN/desafio TLS antes de criar pacotes GAR para que a equipe de gateway e sistema operacional
aprovadores de DNS revisam a mesma evidência. O novo helper reflete os insumos de automação
DG-3 enumerando hosts canônicos curinga, SANs de host bonito, etiquetas DNS-01 e desafios ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```

Commite o JSON junto com o pacote de lançamento (o subelo com o ticket de mudança) para que os operadores
pode pegar os valores SAN na configuração `torii.sorafs_gateway.acme` de Torii e os revisores
de GAR pode confirmar os mapas canonico/prette sem reexecutar derivações de host. Agrega
argumentos `--name` adicionais para cada sufijo promovido no mesmo lançamento.

## Etapa 6b - Deriva mapas de host canonicos

Antes de templários payloads GAR, registre o mapa de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` em sua etiqueta canônica
(`<base32>.gw.sora.id`), emite o curinga necessário (`*.gw.sora.id`) e deriva o bonito host
(`<alias>.gw.sora.name`). Persiste a saída nos artefatos de lançamento para que os revisores
DG-3 pode comparar o mapa junto com o envio GAR:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>` para cair rapidamente quando um JSON de GAR ou ligação de gateway
omita um dos hosts necessários. O helper aceita múltiplos arquivos de verificação, fazendo
facil lint de tanto a plantilla GAR como o `portal.gateway.binding.json` gravado na mesma invocação:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Anexe o resumo JSON e o log de verificação do ticket de mudança de DNS/gatewae para que
os auditores podem confirmar os hosts canônicos, curinga e prette sem reexecutar os scripts.
Execute novamente o comando quando se agregar novos alias ao pacote para que as atualizações GAR
aqui está a mesma evidência.

## Etapa 7 - Geração do descritor de DNS de corte

Os cortes de produção requerem um pacote de mudança auditável. Depois de um submit exitoso
(ligação de alias), o ajudante emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:

- metadados de vinculação de alias (namespace/nome/prova, resumo do manifesto, URL Torii,
 época enviada, autoridade);
- contexto de lançamento (tag, alias label, rutas de manifesto/CAR, plan de chunks, bundle Sigstore);
- punteros de verificação (comando probe, alias + endpoint Torii); e
- campos opcionais de controle de mudança (id de ticket, janela de corte, operações de contato,
 nome do host/zona de produção);
- metadados de promoção de rotas derivados do cabeçalho `Sora-Route-Binding`
 (host canônico/CID, rotas de cabeçalho + vinculação, comandos de verificação), garantindo que a promoção
 GAR e os exercícios de fallback referem-se à mesma evidência;
- os artefatos de plano de rota gerados (`gateway.route_plan.json`,
 templates de headers e headers de rollback opcionais) para que os tickets de mudança e ganchos de
 lint de CI pode verificar que cada pacote DG-3 refere-se aos planos de promoção/rollback
 canonicos antes de uma aprovação;
- metadados opcionais de invalidação de cache (endpoint de purga, variável de autenticação, payload JSON,
 e exemplo de comando `curl`); e
- dicas de rollback apuntando al descritor anterior (tag de release e resumo do manifesto)
 para que os tickets sejam capturados em uma rota de fallback determinista.

Quando o lançamento exigir limpeza de cache, gere um plano canônico junto com o descritor de corte:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```

Anexo o `portal.cache_plan.json` resultante do pacote DG-3 para que os operadores
tengan hosts/paths deterministas (e as dicas de autenticação que coincidem) para emitir solicitações `PURGE`.
A seção opcional de cache do descritor pode referenciar este arquivo diretamente,
mantendo alinhados aos revisores de controle de mudança exatamente para que os endpoints sejam limpos
durante uma transição.

Cada pacote DG-3 também precisa de um checklist de promoção + rollback. Geral via
`cargo xtask soradns-route-plan` para que os revisores de controle de mudança possam seguir os passos
exatos de comprovação, corte e reversão por alias:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

O `gateway.route_plan.json` emitiu captura de hosts canônicos/pretty, registros de verificação de saúde
por etapas, atualizações de vinculação GAR, limpeza de cache e ações de reversão. Incluyelo com
os artefatos GAR/binding/cutover antes de enviar o ticket de mudança para que Ops possa ensayar e
aprobar as mesmas etapas com guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa este descritor e se executa automaticamente desde
`sorafs-pin-release.sh`. Para regenerar ou personalizar manualmente:

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

Releia os metadados opcionais via variáveis de ambiente antes de executar o auxiliar de pin:

| Variável | Proposta |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID do ticket armazenado no descritor. |
| `DNS_CUTOVER_WINDOW` | Ventana de corte ISO8601 (por exemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nome do host de produção + zona autorizada. |
| `DNS_OPS_CONTACT` | Alias ​​de plantão ou contato de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de limpeza de cache registrado no descritor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contém um token de limpeza (padrão: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Rota para o descritor de corte anterior para metadados de reversão. |

Anexo ou JSON na revisão de mudança de DNS para que os aprovadores possam verificar resumos do manifesto,
ligações de alias e comandos testam sem ter que revisar logs de CI. Os sinalizadores da CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, e `--previous-dns-plan` provam as mesmas substituições
quando se executa o helper fora do CI.

## Etapa 8 - Emite o esqueleto do arquivo de zona do resolvedor (opcional)Quando a janela de corte da produção é conhecida, o roteiro de lançamento pode emitir o
esqueleto de zonefile SNS e o snippet de resolver automaticamente. Pasa os registros DNS desejados
e metadados via variáveis de ambiente ou opções CLI; o ajudante llamara a
`scripts/sns_zonefile_skeleton.py` imediatamente depois de gerar o descritor de corte.
Prove pelo menos um valor A/AAAA/CNAME e o resumo GAR (BLAKE3-256 do payload GAR firmado). Si uma
zona/hostname são conhecidos e `--dns-zonefile-out` se omite, o ajudante escreve em
`artifacts/sns/zonefiles/<zone>/<hostname>.json` e cheia
`ops/soradns/static_zones.<hostname>.json` como o trecho de resolução.

| Variável/sinalizador | Proposta |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Rota para o esqueleto do arquivo de zona gerado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Rota do snippet de resolução (padrão: `ops/soradns/static_zones.<hostname>.json` quando omitido). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL aplicado aos registros gerados (padrão: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direções IPv4 (env separado por vírgulas ou flag CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direções IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Destino CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinheiros SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionais (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Substituir o rótulo da versão do zonefile computado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Ative o carimbo de data/hora `effective_at` (RFC3339) no lugar do início de uma janela de corte. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Substituir a prova literal registrada nos metadados. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Substituir o CID registrado nos metadados. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelamento de guardião (suave, duro, descongelamento, monitoramento, emergência). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referência de ticket de guardião/conselho para congelamentos. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 para descongelamento. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de congelamento adicionais (ambiente separado por vírgulas ou flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) da carga útil GAR firmado. Requerido quando há ligações de gateway. |

O fluxo de trabalho do GitHub Actions contém esses valores dos segredos do repositório para que cada pin de
produção emita os artefatos de zonefile automaticamente. Configure os seguintes segredos
(strings podem conter listas separadas por vírgulas para campos multivalor):

| Segredo | Proposta |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nome do host/zona de produção passada para o helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​on-call armazenado no descritor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registros IPv4/IPv6 publicados. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Destino CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pinheiros SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionais. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadados de congelamento registrados no esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 em hexadecimal da carga útil GAR firmado. |

Ao disparar `.github/workflows/docs-portal-sorafs-pin.yml`, fornece as entradas
`dns_change_ticket` e `dns_cutover_window` para que o descritor/zonefile seja exibido aqui
correto. Deixe-os em branco apenas quando executar dre runs.

Invocacion tipica (coincide com o runbook do proprietário SN-7):

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
 ...otros flags...
```

O helper arrastra automaticamente o ticket de mudança como entrada TXT e hereda o inicio de a
janela de corte como timestamp `effective_at` a menos que se haga override. Para o fluxo
operativo completo, versão `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegação pública de DNS

O esqueleto do zonefile define apenas registros autorizados da zona. Ainda e
necessário configurar uma delegação NS/DS da zona pai no registrador ou provedor
DNS para que a Internet encontre os servidores de nomes.

- Para cutovers no apex/TLD, use ALIAS/ANAME (dependente do provedor) ou público
  registros A/AAAA apontando para os IPs anycast do gateway.
- Para subdomínios, publique um CNAME para o host bonito derivado
  (`<fqdn>.gw.sora.name`).
- O host canônico (`<hash>.gw.sora.id`) permanece sob o domínio do gateway e nao
  e publicado dentro da sua zona pública.

### Planta de cabeçalhos do gateway

O ajudante de implantação também emite `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, dos artefatos que atendem ao requisito DG-3 de
ligação de conteúdo de gateway:- `portal.gateway.headers.txt` contém um bloco completo de cabeçalhos HTTP (incluindo
 `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, e o descritor
 `Sora-Route-Binding`) que os gateways de borda devem ser gravados em cada resposta.
- `portal.gateway.binding.json` registra a mesma informação em formato legível por maquinas
 para que os tickets de mudança e a automação possam comparar ligações host/cid sem
 raspar a saída da casca.

É gerado automaticamente via
`cargo xtask soradns-binding-template`
e captura o alias, o resumo do manifesto e o nome do host do gateway que se
passe para `sorafs-pin-release.sh`. Para regenerar ou personalizar o bloco de cabeçalhos,
executar:

```bash
cargo xtask soradns-binding-template \
 --manifest artifacts/sorafs/portal.manifest.json \
 --alias docs.sora \
 --hostname docs.sora.link \
 --route-label production \
 --json-out artifacts/sorafs/portal.gateway.binding.json \
 --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pasa `--csp-template`, `--permissions-template`, ou `--hsts-template` para substituição
dos modelos de cabeçalho por padrão quando um despliegue precisa de instruções adicionais;
combinalos com os switches `--no-*` existentes para remover um cabeçalho por completo.

Anexo o snippet de headers para solicitação de mudança de CDN e alimentação do documento JSON
al pipeline de automação de gateway para que a promoção real de host coincida com a
evidência de lançamento.

O script de release executa automaticamente o helper de verificação para que os
os ingressos DG-3 sempre incluem evidências recentes. Execute-o manualmente
quando edita o bind JSON manualmente:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

O comando decodifica o payload `Sora-Proof` gravado, certifique-se de que os metadados `Sora-Route-Binding`
coincide com o CID do manifesto + hostname, e falha rapidamente se algum cabeçalho for desviado.
Arquive uma saída de console junto com os outros artefatos de despliegue sempre que executado
o comando fora de CI para que os revisores DG-3 tenham provado que o bind foi validado
antes da transição.

> **Integração do descritor DNS:** `portal.dns-cutover.json` agora incrusta uma seção
> `gateway_binding` apuntando a estes artefatos (rutas, conteúdo CID, estado de prova e o
> template literal de headers) **e** uma estrofa `route_plan` referenciando
> `gateway.route_plan.json` mas os templates de cabeçalhos principais e rollback. Incluir isso
> blocos em cada ticket de mudança DG-3 para que os revisores possam comparar os valores
> exatos de `Sora-Name/Sora-Proof/CSP` e confirmar que os planos de promoção/reversão
> coincide com o pacote de evidências sem abrir o arquivo de build.

## Etapa 9 - Executar monitores de publicação

O item do roadmap **DOCS-3c** requer evidência contínua de que o portal, o próximo Tre it e os
As ligações do gateway são mantidas saudáveis depois de um lançamento. Executar o monitor consolidado
Imediatamente depois das etapas 7-8 e conecte-o às suas sondas programadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` carregar o arquivo de configuração (ver
 `docs/portal/docs/devportal/publishing-monitoring.md` para o esquema) e
 execute três verificações: testes de caminhos do portal + validação de CSP/Política de Permissões,
 probes del proxe Tre it (opcionalmente golpeando seu endpoint `/metrics`), e o selecionado
 de ligação de gateway (`cargo xtask soradns-verify-binding`) que agora exige presença e
 o valor esperado de Sora-Content-CID junto com as verificações de alias/manifesto.
- O comando termina com diferente de zero quando falha algum probe para que CI, cron jobs, o operadores
 o runbook pode impedir um lançamento antes de promover alias.
- Pasar `--json-out` escribe um resumo JSON único com estado por target; `--evidence-dir`
 emite `summary.json`, `portal.json`, `tryit.json`, `binding.json` e `checksums.sha256` para que
 os revisores de governança poderão comparar resultados sem reexecutar os monitores. Arquivar
 este diretório inferior `artifacts/sorafs/<tag>/monitoring/` junto com o pacote Sigstore e o
 descritor de DNS de transição.
- Inclui saída do monitor, ou exportação de Grafana (`dashboards/grafana/docs_portal.json`),
 e o ID do drill do Alertmanager em o ticket de liberação para que o SLO DOCS-3c possa ser
 auditado luego. O playbook dedicado ao monitor de publicação vive em
 `docs/portal/docs/devportal/publishing-monitoring.md`.

As sondagens do portal requerem HTTPS e trocam URLs com base `http://` ou menos que `allowInsecureHttp`
está configurado na configuração do monitor; mantenha metas de produção/staging em TLS e apenas
habilita o override para visualizar localidades.

Automatiza o monitor via `npm run monitor:publishing` em Buildkite/cron uma vez que o portal
isto em vivo. O mesmo comando, apontado para URLs de produção, alimenta os cheques de saúde
contínuo que SRE/Docs usa entre lançamentos.

## Automação com `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula as etapas 2 a 6. Este:1. arquivo `build/` em um tarball determinista,
2. execute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
 e `proof verify`,
3. opcionalmente execute `manifest submit` (incluindo alias binding) quando tiver credenciais
 Torii, e
4. escreva `artifacts/sorafs/portal.pin.report.json`, ou opcional
 `portal.pin.proposal.json`, o descritor de DNS de corte (depois de envios),
 e o pacote de ligação de gateway (`portal.gateway.binding.json` mas o bloco de cabeçalhos)
 para que os equipamentos de governança, networking e operações possam comparar o pacote de evidências
 sem revisar logs de CI.

Configurar `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, e (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar o script. Usa `--skip-submit` para secar
corre; O fluxo de trabalho do GitHub descrito abaixo é alternado por meio da entrada `perform_submit`.

## Etapa 8 - Especificações públicas OpenAPI e pacotes SBOM

DOCS-7 requer a construção do portal, a especificação OpenAPI e os artefatos SBOM viajen
pelo mesmo pipeline determinista. Os helpers existentes cobrem os três:

1. **Regenera e assina a especificação.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 Passe uma etiqueta de liberação via `--version=<label>` quando quiser preservar um instantâneo
 histórico (por exemplo `2025-q3`). O ajudante descreve o instantâneo em
 `static/openapi/versions/<label>/torii.json`, olhe em
 `versions/current`, e registra os metadados (SHA-256, estado do manifesto, e
 timestamp atualizado) em `static/openapi/versions.json`. O portal de desenvolvedores
 veja esse índice para que os painéis Swagger/RapiDoc possam apresentar um seletor de versão
 e mostrar o resumo/assinatura associado em linha. Omitir `--version` mantem como etiquetas do
 release anterior intacto e apenas refresca os punteros `current` + `latest`.

 O manifesto captura digests SHA-256/BLAKE3 para que o gateway possa gravar
 cabeçalhos `Sora-Proof` para `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** O pipeline de lançamento espera SBOMs baseados em syft
 segundo `docs/source/sorafs_release_pipeline_plan.md`. Mantenha a saída
 junto com os artefatos de construção:

 ```bash
 syft dir:build -o json > "$OUT"/portal.sbom.json
 syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
 ```

3. **Empaque cada carga útil em um CAR.**

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
 ```

 Siga as mesmas etapas de `manifest build` / `manifest sign` que o site principal,
 ajustando alias por artefato (por exemplo, `docs-openapi.sora` para a especificação e
 `docs-sbom.sora` para o pacote SBOM firmado). Manter alias diferentes mantem SoraDNS,
 GARs e tickets de rollback ativados para carga útil exata.

4. **Envie e vincule.** Reutilize a autoridade existente e o pacote Sigstore, mas registre-se
 o tuple de alias em o checklist de release para que os auditores rastreem que nombre Sora
 mapea a que digest de manifesto.

Arquivar os manifestos de especificação/SBOM junto com a construção do portal garante que cada ticket de
release contém um conjunto completo de artefatos sem reexecutar o packer.

### Auxiliar de automação (script CI/pacote)

`./ci/package_docs_portal_sorafs.sh` codifica as etapas 1-8 para que o item do roteiro
**DOCS-7** pode ser executado com apenas um comando. Ó ajudante:

- executar uma preparação necessária do portal (`npm ci`, sincronização OpenAPI/norito, testes de widgets);
- emitir os CARs e pares de manifesto do portal, OpenAPI e SBOM via `sorafs_cli`;
- opcionalmente execute `sorafs_cli proof verify` (`--proof`) e a assinatura Sigstore
 (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deixe todos os artefatos abaixo `artifacts/devportal/sorafs/<timestamp>/` e
 escreva `package_summary.json` para que CI/tooling de liberação possa inserir o pacote; e
- refresca `artifacts/devportal/sorafs/latest` para apontar para uma execução mais recente.

Exemplo (pipeline completo com Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

Sinaliza um conocer:

- `--out <dir>` - sobrescreve a raiz dos artefatos (mantem carpetas padrão com carimbo de data/hora).
- `--skip-build` - reutiliza um `docs/portal/build` existente (util quando CI não pode
 reconstruir por espelhos offline).
- `--skip-sync-openapi` - omitir `npm run sync-openapi` quando `cargo xtask openapi`
 você não pode acessar crates.io.
- `--skip-sbom` - evita chamar `syft` quando o binário não está instalado (o script imprime
 uma advertência em seu lugar).
- `--proof` - execute `sorafs_cli proof verify` para cada par CAR/manifesto. As cargas úteis de
 múltiplos arquivos que requerem suporte de chunk-plan em uma CLI, assim que você deixar este sinalizador
 sem estabelecer se houver erros de `plan chunk count` e verifique manualmente quando
 llegue o portão rio acima.
- `--sign` - invoque `sorafs_cli manifest sign`. Prove um token com
 `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou deixe que o CLI seja obtido usando
 `--sigstore-provider/--sigstore-audience`.Quando inveja artefatos de produção usa `docs/portal/scripts/sorafs-pin-release.sh`.
Agora empaque o portal, OpenAPI e payloads SBOM, assinatura de cada manifesto, e registro de metadados
ativos extras em `portal.additional_assets.json`. O helper entiende os mesmos botões opcionais
usados por o empacotador de CI mas os novos switches `--openapi-*`, `--portal-sbom-*`, e
`--openapi-sbom-*` para atribuir tuplas de alias por artefato, substituir uma fonte SBOM via
`--openapi-sbom-source`, omitir determinadas cargas úteis (`--skip-openapi`/`--skip-sbom`),
e aponte para um binário `syft` no default com `--syft-bin`.

O script expõe cada comando que executa; copia o log em o ticket de lançamento
junto a `package_summary.json` para que os revisores possam comparar resumos de CAR, metadados de
plan, e hashes do pacote Sigstore sem revisar a saída do shell ad hoc.

## Etapa 9 - Verificação de gateway + SoraDNS

Antes de anunciar um cutover, prove que o alias novo será resolvido via SoraDNS e que os gateways
engrapan provas frescas:

1. **Execute o portão da sonda.** `ci/check_sorafs_gateway_probe.sh` ejeção
 `cargo xtask sorafs-gateway-probe` contra os fixtures demo em
 `fixtures/sorafs_gateway/probe_demo/`. Para esclarecer a verdade, tente sondar o objetivo do nome do host:

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 A sonda decodifica `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status` segun
 `docs/source/sorafs_alias_policy.md` e falhou quando o resumo do manifesto,
 os TTLs ou os binds GAR são desviados.

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **Captura de evidência de exercício.** Para exercícios de operador ou simulacros de PagerDuty, consulte
 o probe com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
 lançamento do devportal -- ...`. O wrapper guarda cabeçalhos/logs abaixo
 `artifacts/sorafs_gateway_probe/<stamp>/`, atualiza `ops/drill-log.md`, e
 (opcionalmente) vários ganchos de reversão ou payloads PagerDuty. Establece
 `--host docs.sora` para validar uma rota SoraDNS em vez de codificar um IP.

3. **Verifique ligações DNS.** Quando a governança publica a prova do alias, registre o arquivo GAR
 referenciado pela sonda (`--gar`) e adicionado à evidência de liberação. Os proprietários do resolvedor
 podemos refletir a mesma entrada via `tools/soradns-resolver` para garantir que as entradas no cache
 respeten o manifesto novo. Antes de anexar o JSON, execute
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 para que o mapa determinista de host, os metadados do manifesto e as etiquetas de telemetre se
 válido offline. O helper pode emitir um resumo `--json-out` junto com o GAR firmado para que os
 Os revisores têm evidências verificáveis sem abrir o binário.
 Quando redigir um GAR novo, prefira
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (Vuelve a `--manifest-cid <cid>` apenas quando o arquivo do manifesto não estiver disponível). Ó ajudante
 agora deriva o CID **e** o digest BLAKE3 diretamente do manifesto JSON, recorta espaços em branco,
 deduplica flags `--telemetry-label` repete, ordena as tags, e emite os templates por padrão
 de CSP/HSTS/Permissions-Police antes de escrever o JSON para que o payload seja mantido determinista
 mesmo quando os operadores capturam etiquetas a partir de conchas distintas.

4. **Observe as métricas de alias.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 e `torii_sorafs_gateway_refusals_total{profile="docs"}` em pantalla enquanto o probe
 corre; ambas as séries foram gráficadas em `dashboards/grafana/docs_portal.json`.

## Etapa 10 - Monitoramento e pacote de evidências- **Painéis.** Exportação `dashboards/grafana/docs_portal.json` (SLOs do portal),
 `dashboards/grafana/sorafs_gateway_observability.json` (latência do gateway +
 saúde de prova), e `dashboards/grafana/sorafs_fetch_observability.json`
 (saúde do orquestrador) para cada lançamento. Anexo os exporta JSON para ticket de
 release para que os revisores possam reproduzir as consultas de Prometheus.
- **Arquivos de teste.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` em git-annex
 o seu balde de evidências. Inclui resumo do probe, cabeçalhos e payload PagerDuty
 capturado pelo script de telemetria.
- **Bundle de release.** Guarda os resumos de CAR do portal/SBOM/OpenAPI, bundles de manifesto,
 firmas Sigstore, `portal.pin.report.json`, logs de sonda Try-It, e relatórios de link-check
 abaixo uma pasta com timestamp (por exemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quando as sondas fazem parte de uma broca, deixe que
 `scripts/telemetry/run_sorafs_gateway_probe.sh` concordo com entradas para `ops/drill-log.md`
 para que a mesma evidência satisfaga o requisito de caos SNNet-5.
- **Links de ticket.** Referência dos IDs do painel de Grafana ou exporta PNG adjuntos no ticket
 de mudança, junto com a rota do relatório de sondagem, para que os revisores possam cruzar os SLOs
 sem acesso a um shell.

## Etapa 11 - Drill de fetch multi-source e evidência de scoreboard

Publicar em SoraFS agora requer evidência de fetch multi-source (DOCS-7/SF-6)
junto com as provas de DNS/gatewae anteriores. Depois de firmar o manifesto:

1. **Execute `sorafs_fetch` contra o manifesto live.** Usa os mesmos artefatos de plano/manifesto
 produzidos nas Etapas 2-3, mas como credenciais de gateway emitidas para cada provedor. Persistir
 Toda a salida para que os auditores possam reproduzir o rastro de decisão do orquestrador:

 ```bash
 OUT=artifacts/sorafs/devportal
 FETCH_OUT="$OUT/fetch/$(date -u +%E%m%dT%H%M%SZ)"
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

 - Procure primeiro os anúncios de fornecedores referenciados pelo manifesto (por exemplo
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 e pasalos via `--provider-advert name=path` para que o placar possa avaliar as janelas
 de capacidade de forma determinista. EUA
 `--allow-implicit-provider-metadata` **apenas** quando reproduz fixtures em CI; exercícios de sistema operacional
 de produção deve citar os anúncios firmados que llegaron com o pin.
 - Quando o manifesto referencia regiões adicionais, repita o comando com os tuples de
 provedores correspondentes para que cada cache/alias tenha um artefato de fetch associado.

2. **Arquivar como saídas.** Guarda `scoreboard.json`,
 `providers.ndjson`, `fetch.json`, e `chunk_receipts.ndjson` abaixo da pasta de evidências do
 lançamento. Esses arquivos capturam a ponderação de pares, o orçamento de retorno, a latência EWMA, e os
 receitas por parte que o pacote de governança deve ser retido para SF-7.

3. **Atualiza telemetria.** Importa as saídas de busca no painel **SoraFS Fetch
 Observabilidade** (`dashboards/grafana/sorafs_fetch_observability.json`),
 observando `torii_sorafs_fetch_duration_ms`/`_failures_total` e os painéis de rango por provedor
 para anomalias. Coloque os snapshots do painel Grafana no ticket de lançamento junto com a rota do
 placar.

4. **Smokea conforme regras de alerta.** Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh`
 para validar o pacote de alertas Prometheus antes de encerrar ou liberar. Anexo à saída de
 promtool al ticket para que os revisores DOCS-7 confirmem que as alertas de stall e
 provedor lento segue armadas.

5. **Cabe em CI.** O fluxo de trabalho do pino do portal mantém uma etapa `sorafs_fetch` depois da entrada
 `perform_fetch_probe`; habilitalo para execuções de encenação/produção para que a evidência de
 buscar se produz junto com o pacote de manifesto sem manual de intervenção. Os locais de treino podem
 reutilizar o mesmo script exportando os tokens de gateway e estabelecendo `PIN_FETCH_PROVIDERS`
 uma lista de fornecedores separados por vírgulas.

## Promoção, observação e reversão

1. **Promoção:** mantenha alias separados para encenação e produção. Promociona reexecutando
 `manifest submit` com o mesmo manifesto/bundle, mudando
 `--alias-namespace/--alias-name` para apontar o alias de produção. Isso evita
 reconstruir ou reassinar uma vez que o QA teste o pino de teste.
2. **Monitoramento:** importa o painel de registro de pin
 (`docs/source/grafana_sorafs_pin_registry.json`) mas as sondas específicas do portal
 (ver `docs/portal/docs/devportal/observability.md`). Alerta de desvio de checksums,
 sondas falidas ou picos de retenção de prova.
3. **Rollback:** para reverter, reenviar o manifesto anterior (ou retirar o alias atual) usando
 `sorafs_cli manifest submit --alias ... --retire`. Mantenha sempre o último pacote
 conhecido como bom e o resumo CAR para que as tentativas de rollback possam recriar si
 os logs de CI são rotan.

## Planta de fluxo de trabalho CI

Como mínimo, seu pipeline deve:1. Build + lint (`npm ci`, `npm run build`, geração de somas de verificação).
2. Empacotar (`car pack`) e manifestos computacionais.
3. Assinar usando o token OIDC do trabalho (`manifest sign`).
4. Subir artefatos (CAR, manifesto, pacote, plano, resumos) para auditórios.
5. Envie para o registro do PIN:
 - Solicitações pull -> `docs-preview.sora`.
 - Tags/filiais protegidas -> promoção de alias de produção.
6. Executar probes + gates de verificação de prova antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todas essas etapas para lançamentos manuais.
Ó fluxo de trabalho:

- construir/testar o portal,
- empaqueta o build via `scripts/sorafs-pin-release.sh`,
- assinar/verificar o pacote de manifesto usando GitHub OIDC,
- sube o CAR/manifesto/bundle/plan/resumos como artefatos, e
- (opcionalmente) envie o manifesto + alias binding quando tiver segredos.

Configure os seguintes segredos/variáveis do repositório antes de disparar o trabalho:

| Nome | Proposta |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expõe `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrada com submissões. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridade de assinatura para envio do manifesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias incluída no manifesto quando `perform_submit` é `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Pacote de prova de alias codificado em base64 (opcional; omitir para saltar alias binding). |
| `DOCS_ANALYTICS_*` | Endpoints de análise/sonda existentes reutilizados por outros fluxos de trabalho. |

Dispara o fluxo de trabalho por meio de uma UI de Ações:

1. Prove `alias_label` (por exemplo, `docs.sora.link`), `proposal_alias` opcional,
 e uma substituição opcional de `release_tag`.
2. Deixe `perform_submit` sem marcar para gerar artefatos sem tocar Torii
 (util para dre runs) ou habilite-o para publicar diretamente no alias configurado.

`docs/source/sorafs_ci_templates.md` na documentação dos helpers de CI genéricos para
projetos fora deste repositório, mas o fluxo de trabalho do portal deve ser uma opção preferida
para lançamentos del dia a dia.

## Lista de verificação

- [ ] `npm run build`, `npm run test:*`, e `npm run check:links` estão em verde.
- [ ] `build/checksums.sha256` e `build/release.json` capturados em artefatos.
- [ ] CAR, plano, manifesto, e resumo gerado abaixo `artifacts/`.
- [ ] Pacote Sigstore + assinatura separada almacenados com logs.
- [ ] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json`
 capturados quando há envios.
- [ ] `portal.pin.report.json` (e opcional `portal.pin.proposal.json`)
 arquivado junto aos artefatos CAR/manifesto.
- [ ] Logs de `proof verify` e `manifest verify-signature` arquivados.
- [ ] Dashboards de Grafana atualizados + probes Try-It exitosos.
- [ ] Notas de reversão (ID do manifesto anterior + resumo do alias) adjuntas al
 bilhete de lançamento.