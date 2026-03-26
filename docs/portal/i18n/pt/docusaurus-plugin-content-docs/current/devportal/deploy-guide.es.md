---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Panorama geral

Este playbook contém os elementos do roteiro **DOCS-7** (publicação de SoraFS) e **DOCS-8**
(automatização de pin de CI/CD) em um procedimento acionável para o portal de desenvolvimento.
Cubra a fase de construção/lint, o pacote SoraFS, a firma de manifestos com Sigstore,
a promoção de alias, a verificação e os exercícios de reversão para cada visualização e lançamento
mar reproduzível e auditável.

O fluxo pressupõe que você tenha o binário `sorafs_cli` (construído com `--features cli`), acesso a
um endpoint Torii com permissões de registro de pinos e credenciais OIDC para Sigstore. Guarda secreto
de longa vida (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) em seu cofre de CI; las
ejecuciones locales podem ser carregadas a partir de exportações do shell.

## Pré-requisitos

- Nó 18.18+ com `npm` ou `pnpm`.
- `sorafs_cli` de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que expõe `/v1/sorafs/*` mas uma conta/clave privada de autoridade que pode enviar manifestos e alias.
- Emisor OIDC (GitHub Actions, GitLab, identidade de carga de trabalho, etc.) para emitir um `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para simulações e `docs/source/sorafs_ci_templates.md` para scaffolding de fluxos de trabalho do GitHub/GitLab.
- Configure as variáveis ​​OAuth de Try it (`DOCS_OAUTH_*`) e execute-as
  [lista de verificação de reforço de segurança](./security-hardening.md) antes de promover uma compilação
  fora do laboratório. A construção do portal agora falha quando essas variáveis estão faltando
  ou quando os botões de TTL/polling são usados fora das janelas aplicadas; exportar
  `DOCS_OAUTH_ALLOW_INSECURE=1` apenas para visualizações de localidades desechables. Adjunta la
  evidência de pen-test no ticket de lançamento.

## Paso 0 - Capturar um pacote de proxy Experimente

Antes de promover uma visualização no Netlify ou no gateway, venda as fontes do proxy Try it e el
resumo do manifesto OpenAPI firmado em um pacote determinado:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copia os auxiliares de proxy/sonda/reversão, verifica a firma
OpenAPI e descreve `release.json` mas `checksums.sha256`. Adjunta este pacote ao ticket de
promoção do gateway Netlify/SoraFS para que os revisores possam reproduzir as fontes exatas
del proxy e as pistas do alvo Torii sem reconstrução. O pacote também é registrado se perdido
portadores fornecem provisões para o cliente estabelecer habilitações (`allow_client_auth`) para manter o plano
de implementação e regras CSP em sincronização.

## Paso 1 - Construir e instalar o portal

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

`npm run build` ejeta automaticamente `scripts/write-checksums.mjs`, produzindo:

- `build/checksums.sha256` - manifestado para SHA256 adequado para `sha256sum -c`.
- `build/release.json` - metadados (`tag`, `generated_at`, `source`) fixados em cada CAR/manificado.

Arquivar ambos os arquivos junto com o currículo CAR para que os revisores possam comparar artefatos de
visualizar pecado reconstruir.

## Paso 2 - Empaque os ativos estaticos

Execute o empacotador CAR contra o diretório de saída de Docusaurus. O exemplo de baixo
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

O currículo JSON captura conteúdo de pedaços, resumos e pistas de planejamento de prova que
`manifest build` e os painéis de CI reutilizados depois.

## Paso 2b - Empaqueta companheiros OpenAPI y SBOM

DOCS-7 requer a publicação do site do portal, do snapshot OpenAPI e das cargas úteis SBOM
como manifestações diferentes para que os gateways possam gravar os cabeçalhos
`Sora-Proof`/`Sora-Content-CID` para cada artefato. El ajudante de lançamento
(`scripts/sorafs-pin-release.sh`) e empaque o diretório OpenAPI
(`static/openapi/`) e os SBOMs emitidos via `syft` em CARs separados
`openapi.*`/`*-sbom.*` e registre os metadados em
`artifacts/sorafs/portal.additional_assets.json`. Ao executar o fluxo manual,
repita os passos 2-4 para cada carga útil com seus próprios prefixos e etiquetas de metadados
(por exemplo `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Registrar cada par manifestado/alias
en Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) antes de alterar DNS para que
el gateway pode servir testes engrapados para todos os artefatos publicados.

## Paso 3 - Construindo o manifesto

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

Ajusta as bandeiras de política de pin após sua janela de lançamento (por exemplo, `--pin-storage-class
quente` para canários). A variante JSON é opcional, mas conveniente para revisão de código.

## Paso 4 - Firma com Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```O pacote registra o resumo do manifesto, os resumos dos pedaços e um hash BLAKE3 do token
OIDC não persiste no JWT. Guarda tanto o pacote como a firma separada; as promoções de
a produção pode reutilizar os mesmos artefatos em vez de reafirmá-los. Locais de execução
pode substituir as bandeiras do provedor com `--identity-token-env` (ou estabelecer
`SIGSTORE_ID_TOKEN` no ambiente) quando um auxiliar OIDC externo emite o token.

## Paso 5 - Envia al pin record

Envia o manifesto firmado (e o plano de pedaços) para Torii. Solicite sempre um currículo para isso
a entrada/alias resultante é auditável.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Quando você usar um alias de preview ou canary (`docs-preview.sora`), repita o
envie com um alias único para que o controle de qualidade possa verificar o conteúdo antes da promoção a
produção.

A ligação do alias requer três campos: `--alias-namespace`, `--alias-name` e `--alias-proof`.
A governança produz o pacote de prova (base64 ou bytes Norito) quando a solicitação é solicitada
do apelido; guarde-o em segredos de CI e exponha-o como arquivo antes de invocar `manifest submit`.
Deixe os sinalizadores de alias sem estabelecer quando você só pensa em fixar o manifesto sem tocar DNS.

## Paso 5b - Gerando uma proposta de governança

Cada manifesto deve viajar com uma lista proposta para o Parlamento para qualquer cidadão
Sora pueda introduzir a mudança sem pedir credenciais privilegiadas. Depois dos passos
enviar/assinar, ejecuta:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura a instrução canônica `RegisterPinManifest`,
el digest de chunks, la politica e la pista de alias. Adjuntalo ao ticket de governança ou al
portal do Parlamento para que os delegados possam comparar a carga útil sem reconstruir os artefatos.
Como el comando nunca toca a chave de autoridade de Torii, qualquer cidadão pode redigir la
propuesta localmente.

## Paso 6 - Verificações e telemetria

Depois de fijar, execute as etapas de verificação determinista:

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

- Verifique `torii_sorafs_gateway_refusals_total` e
  `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalias.
- Execute `npm run probe:portal` para executar o proxy Try-It e os links registrados
  contra o conteúdo recebido.
- Captura a evidência do monitoramento descrita em
  [Publicação e monitoramento](./publishing-monitoring.md) para o portão de observação DOCS-3c
  se satisfaça junto com os passos de publicação. El helper agora aceita entradas múltiplas
  `bindings` (site, OpenAPI, SBOM do portal, SBOM de OpenAPI) e aplicativo `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  no host objetivo através do protetor opcional `hostname`. A invocação de baixo descreve tanto um
  resumo JSON único como o pacote de evidência (`portal.json`, `tryit.json`, `binding.json` y
  `checksums.sha256`) abaixo do diretório de lançamento:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - Plano de certificados do gateway

Deriva o plano de SAN/challenge TLS antes de criar pacotes GAR para que o equipamento de gateway e os
aprovadores de DNS revisam a mesma evidência. El nuevo helper reflete os índices de automatização
DG-3 enumerando hosts canônicos curinga, SANs de host bonito, etiquetas DNS-01 e desafios ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Commite o JSON junto com o pacote de lançamento (ou subelo com o ticket de mudança) para os operadores
você pode gravar os valores SAN na configuração `torii.sorafs_gateway.acme` de Torii e revisá-los
de GAR pode confirmar os mapas canonico/pretty sem reexecutar derivações de host. Agrega
argumentos `--name` adicionais para cada sufijo promovido no mesmo lançamento.

## Paso 6b - Deriva mapeos de host canonicos

Antes de templários payloads GAR, registre o mapa de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` em sua etiqueta canônica
(`<base32>.gw.sora.id`), emite o curinga necessário (`*.gw.sora.id`) e deriva o pretty host
(`<alias>.gw.sora.name`). Persista a saída nos artefatos de liberação para que os revisores
DG-3 pode comparar o mapa junto com o envio GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>` para cair rapidamente quando um JSON de GAR ou ligação de gateway
omita um dos hosts necessários. El helper aceita múltiplos arquivos de verificação, fazendo
é fácil usar tanto a planta GAR como o `portal.gateway.binding.json` gravado na mesma invocação:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Adicione o currículo JSON e o log de verificação ao ticket de mudança de DNS/gateway para que
os auditores podem confirmar os hosts canônicos, curinga e pretty sem reexecutar os scripts.
Execute novamente o comando quando você agregar novos alias ao pacote para que as atualizações GAR
aqui está a misma evidência.

## Paso 7 - Gera o descritor de DNS de transição

Os cortes de produção exigem um pacote de mudança auditável. Depois de um envio exitoso
(ligação de alias), el helper emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:

- metadados de vinculação de alias (namespace/nome/prova, resumo do manifesto, URL Torii,
  época enviada, autoridade);
- contexto de lançamento (tag, alias label, rotas de manifiedto/CAR, plan de chunks, bundle Sigstore);
- punteros de verificação (comando probe, alias + endpoint Torii); sim
- campos opcionais de controle de mudança (id de ticket, janela de corte, operações de contato,
  nome do host/zona de produção);
- metadados de promoção de rotas derivados do cabeçalho `Sora-Route-Binding`
  (host canonico/CID, rotas de cabeçalho + vinculação, comandos de verificação), garantindo que a promoção
  GAR e os exercícios de fallback referem-se à mesma evidência;
- os artefatos de plano de rota gerados (`gateway.route_plan.json`,
  modelos de cabeçalhos e cabeçalhos de reversão opcionais) para que os tickets de mudança e ganchos de
  lint de CI pode verificar se cada pacote DG-3 faz referência aos planos de promoção/reversão
  canônicos antes da aprovação;
- metadados opcionais de invalidação de cache (endpoint de purga, variável de autenticação, payload JSON,
  e exemplo de comando `curl`); sim
- dicas de rollback apuntando o descritor anterior (tag de release e resumo do manifesto)
  para que os tickets sejam capturados em uma rota determinista de fallback.

Quando o lançamento exige limpeza de cache, gera um plano canônico junto com o descritor de corte:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Adjunta o `portal.cache_plan.json` resultante do pacote DG-3 para os operadores
tengan hosts/paths deterministas (e as dicas de autenticação que coincidem) para emitir solicitações `PURGE`.
A seção opcional de cache do descritor pode referenciar este arquivo diretamente,
mantendo alinhados aos revisores de controle de mudança exatamente para que os endpoints sejam limpos
durante uma transição.

Cada pacote DG-3 também precisa de uma lista de verificação de promoção + reversão. Geral via
`cargo xtask soradns-route-plan` para que os revisores de controle de mudança possam seguir os passos
exatos de comprovação, corte e reversão por alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

El `gateway.route_plan.json` emitiu captura de hosts canônicos/pretty, registros de verificação de saúde
em etapas, atualizações de vinculação GAR, limpeza de cache e ações de reversão. Incluir com
os artefatos GAR/binding/cutover antes de enviar o ticket de mudança para que Ops possa ensayar e
aprobar los mismos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa este descritor e é executado automaticamente desde
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

Releia os metadados opcionais por meio de variáveis de ambiente antes de executar o auxiliar de pin:

| Variável | Proposta |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID do ticket armazenado no descritor. |
| `DNS_CUTOVER_WINDOW` | Janela de corte ISO8601 (por exemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nome do host de produção + zona autorizada. |
| `DNS_OPS_CONTACT` | Alias ​​de plantão ou contato de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de limpeza de cache registrado no descritor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contém o token de limpeza (padrão: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Rota para o descritor de corte anterior para metadados de reversão. |

Adicione o JSON à revisão de mudança de DNS para que os aprovadores possam verificar resumos de manifestação,
ligações de alias e comandos sondam sem ter que revisar logs de CI. Os sinalizadores da CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, e `--previous-dns-plan` provam as mesmas substituições
quando o ajudante é executado fora do CI.

## Paso 8 - Emite o esqueleto do arquivo de zona do resolvedor (opcional)Quando a janela de corte da produção é conhecida, o roteiro de lançamento pode divulgar o
esqueleto do zonefile SNS e o trecho de resolução automática. Passar os registros DNS desejados
e metadados via variáveis de ambiente ou opções CLI; el ajudante llamara a
`scripts/sns_zonefile_skeleton.py` imediatamente após gerar o descritor de corte.
Prove pelo menos um valor A/AAAA/CNAME e o resumo GAR (BLAKE3-256 do payload GAR firmado). Si la
zona/hostname são conhecidos e `--dns-zonefile-out` é omitido, el helper escribe en
`artifacts/sns/zonefiles/<zone>/<hostname>.json` e cheia
`ops/soradns/static_zones.<hostname>.json` como o trecho de resolução.

| Variável/sinalizador | Proposta |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Ruta para o esqueleto do zonefile gerado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Rota do trecho de resolução (padrão: `ops/soradns/static_zones.<hostname>.json` quando omitido). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL aplicado aos registros gerados (padrão: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direções IPv4 (env separado por vírgulas ou flag CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direções IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Destino CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinheiros SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionais (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Substituir o rótulo da versão do zonefile computado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Acione o carimbo de data/hora `effective_at` (RFC3339) no lugar do início da janela de corte. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Substituir a prova literal registrada nos metadados. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Substituir o CID registrado nos metadados. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelamento de guardião (suave, duro, descongelamento, monitoramento, emergência). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referência de ticket de guardião/conselho para congelamentos. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 para descongelamento. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de congelamento adicionais (ambiente separado por vírgulas ou flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) da carga útil GAR firmado. Requerido quando há ligações de gateway. |

O fluxo de trabalho do GitHub Actions contém esses valores dos segredos do repositório para cada pino de
a produção emita os artefatos do arquivo de zona automaticamente. Configurar os segredos seguintes
(strings podem conter listas separadas por vírgulas para campos multivalor):

| Segredo | Proposta |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nome do host/zona de produção passada para o helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de plantão armazenado no descritor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registros IPv4/IPv6 publicados. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Destino CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pinheiros SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionais. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadados de congelamento registrados no esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 em hexadecimal da carga útil GAR firmado. |

Ao disparar `.github/workflows/docs-portal-sorafs-pin.yml`, fornece as entradas
`dns_change_ticket` e `dns_cutover_window` para que o descritor/zonefile aqui esteja a janela
correto. Deixe-os em branco sozinho quando executar testes.

Invocação típica (coincide com o runbook do proprietário SN-7):

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

O ajudante arrastra automaticamente o ticket de mudança como entrada TXT e herda o início da
janela de corte como timestamp `effective_at` a menos que se haga override. Para o fluxo
operativo completo, versão `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegação pública de DNS

O esqueleto do arquivo de zona apenas define registros autorizados da zona. Tia
você deve configurar a delegação NS/DS da zona pai em seu registrador ou
provedor de DNS para que o público da Internet possa encontrar os servidores de nomes.

- Para cortes no apex/TLD, use ALIAS/ANAME (dependente do provedor) o
  publica registros A/AAAA que apontam para o IP anycast do gateway.
- Para subdomínios, publique um CNAME para o host bonito derivado
  (`<fqdn>.gw.sora.name`).
- O host canônico (`<hash>.gw.sora.id`) permanece abaixo do domínio do gateway
  e não se publica dentro da sua zona pública.

### Planta de cabeçalhos do gateway

O ajudante de implantação também emite `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, dos artefatos que satisfazem o requisito DG-3 de
ligação de conteúdo de gateway:- `portal.gateway.headers.txt` contém o bloco completo de cabeçalhos HTTP (incluindo
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, e o descritor
  `Sora-Route-Binding`) que os gateways de borda devem ser gravados em cada resposta.
- `portal.gateway.binding.json` registra a mesma informação em formato legível por máquinas
  para que os tickets de mudança e automatização possam comparar ligações host/cid sin
  raspar a salida da casca.

É gerado automaticamente via
`cargo xtask soradns-binding-template`
e captura o alias, o resumo do manifesto e o nome do host do gateway que se
passe para `sorafs-pin-release.sh`. Para regenerar ou personalizar o bloco de cabeçalhos,
ejetado:

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
dos modelos de cabeçalho por padrão quando você precisa de instruções adicionais;
combine-os com os switches `--no-*` existentes para remover um cabeçalho por completo.

Adiciona o snippet de cabeçalho à solicitação de mudança de CDN e alimenta o documento JSON
todo pipeline de automatização de gateway para que a promoção real de host coincida com a
evidência de lançamento.

O script de liberação executa automaticamente o auxiliar de verificação para que os
os ingressos DG-3 sempre incluem evidências recentes. Execute-o manualmente
Quando você edita a ligação JSON manualmente:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

O comando decodifica a carga útil `Sora-Proof` gravada, garante que os metadados `Sora-Route-Binding`
coincide com o CID do manifesto + nome do host, e falha rapidamente se algum cabeçalho for desviado.
Arquivar a saída de console junto com os outros artefatos de despliegue sempre que for executado
o comando fora de CI para que os revisores DG-3 tente verificar se a ligação foi validada
antes da transição.

> **Integração do descritor DNS:** `portal.dns-cutover.json` agora incrusta uma seção
> `gateway_binding` apontando para esses artefatos (rutas, conteúdo CID, estado de prova e el
> template literal de headers) **y** uma estrofa `route_plan` referenciando
> `gateway.route_plan.json` mas os modelos de cabeçalhos principais e rollback. Incluir isso
> blocos em cada ticket de mudança DG-3 para que os revisores possam comparar os valores
> exatos de `Sora-Name/Sora-Proof/CSP` e confirme que os planos de promoção/reversão
> coincidir com o pacote de evidências sem abrir o arquivo de construção.

## Paso 9 - Ejecuta monitores de publicação

O item do roteiro **DOCS-3c** requer evidência contínua de que o portal, o proxy Try it e os
As ligações do gateway são mantidas saudáveis após um lançamento. Ejetar o monitor consolidado
imediatamente após os passos 7-8 e conecte-o às suas sondas programadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` carrega o arquivo de configuração (ver
  `docs/portal/docs/devportal/publishing-monitoring.md` para o esquema) e
  executa três verificações: sondagens de caminhos do portal + validação de CSP/Política de Permissões,
  sondas do proxy Try it (opcionalmente golpeando o endpoint `/metrics`), e o selecionado
  de ligação de gateway (`cargo xtask soradns-verify-binding`) que agora exige presença e
  o valor esperado de Sora-Content-CID junto com as verificações de alias/manifiedsto.
- O comando termina com diferente de zero quando algum probe falha para CI, cron jobs ou operadores
  o runbook pode impedir um lançamento antes da promoção alias.
- Pasar `--json-out` escreve um currículo JSON único com estado por destino; `--evidence-dir`
  emitir `summary.json`, `portal.json`, `tryit.json`, `binding.json` e `checksums.sha256` para que
  os revisores de governança podem comparar resultados sem reexecutar os monitores. Arquivo
  este diretório abaixo `artifacts/sorafs/<tag>/monitoring/` junto com o pacote Sigstore e el
  descritor de DNS de transição.
- Inclui a saída do monitor, a exportação de Grafana (`dashboards/grafana/docs_portal.json`),
  e o ID do exercício do Alertmanager no ticket de liberação para que o SLO DOCS-3c possa ser
  auditado luego. O playbook dedicado ao monitor de publicação ao vivo em
  `docs/portal/docs/devportal/publishing-monitoring.md`.

As sondagens do portal exigem HTTPS e trocam URLs com base `http://` ou menos que `allowInsecureHttp`
está configurado na configuração do monitor; manter metas de produção/encenação em TLS e solo
habilita el override para visualizar localidades.

Automatiza o monitor via `npm run monitor:publishing` no Buildkite/cron uma vez que o portal
isto é ao vivo. O mesmo comando, apontado para URLs de produção, alimenta os cheques de saúde
contínuo que SRE/Docs usa entre lançamentos.

## Automatização com `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula os Passos 2-6. Este:1. arquive `build/` em um tarball determinista,
2. ejecuta `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   e `proof verify`,
3. opcionalmente execute `manifest submit` (incluindo ligação de alias) quando houver credenciais
   Torii, y
4. escreva `artifacts/sorafs/portal.pin.report.json`, o opcional
  `portal.pin.proposal.json`, o descritor de DNS de transição (após envios),
  e o pacote de ligação do gateway (`portal.gateway.binding.json` mas o bloco de cabeçalhos)
  para que os equipamentos de governança, networking e operações possam comparar o pacote de evidências
  sem revisar os logs do CI.

Configurar `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, y (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar o script. Usa `--skip-submit` para secar
corre; O fluxo de trabalho do GitHub descrito abaixo é alternado por meio da entrada `perform_submit`.

## Paso 8 - Especificações públicas OpenAPI e pacotes SBOM

DOCS-7 exige que a construção do portal, a especificação OpenAPI e os artefatos SBOM viajem
pelo mesmo pipeline determinista. Os ajudantes existentes cobrem os três:

1. **Regenera e firma a especificação.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Coloque uma etiqueta de liberação via `--version=<label>` quando quiser preservar um instantâneo
   histórico (por exemplo `2025-q3`). O ajudante descreve o instantâneo
   `static/openapi/versions/<label>/torii.json`, o espelho em
   `versions/current`, e registra os metadados (SHA-256, estado do manifesto, e
   timestamp atualizado) em `static/openapi/versions.json`. O portal de desenvolvimento
   Veja esses índices para que os painéis Swagger/RapiDoc possam apresentar um seletor de versão
   e mostre o resumo/firma associado on-line. Omitir `--version` mantenha as etiquetas do
   release anterior intacto e apenas refresca os punteros `current` + `latest`.

   A captura manifestada digere SHA-256/BLAKE3 para que o gateway possa gravá-lo
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

   Siga os mesmos passos de `manifest build` / `manifest sign` que o site principal,
   ajustando alias por artefato (por exemplo, `docs-openapi.sora` para a especificação e
   `docs-sbom.sora` para o pacote SBOM firmado). Mantener também conhecido como diferentes mantiene SoraDNS,
   GARs e tickets de rollback ativados para carga útil exata.

4. **Enviar e vincular.** Reutilize a autoridade existente e o pacote Sigstore, mas registre-se
   a tupla de alias na lista de verificação de liberação para que os auditores rastreiem o nome de Sora
   mapea a que digest de manifesto.

Arquivar os manifestos de especificação/SBOM junto com a construção do portal garante que cada ticket de
release contém o conjunto completo de artefatos sem reexecutar o empacotador.

### Auxiliar de automação (script CI/pacote)

`./ci/package_docs_portal_sorafs.sh` codifica os passos 1-8 para que o item do roteiro
**DOCS-7** pode ser executado com um único comando. El ajudante:

- executa a preparação necessária do portal (`npm ci`, sincronização OpenAPI/norito, testes de widgets);
- emitir os CARs e pares de manifestação do portal, OpenAPI e SBOM via `sorafs_cli`;
- opcionalmente execute `sorafs_cli proof verify` (`--proof`) e a firma Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deixe todos os artefatos abaixo de `artifacts/devportal/sorafs/<timestamp>/` e
  escreva `package_summary.json` para que CI/tooling de release possa inserir o pacote; sim
- refresca `artifacts/devportal/sorafs/latest` para executar a execução mais recente.

Exemplo (pipeline completo com Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Sinaliza um conocer:- `--out <dir>` - substitui a raiz dos artefatos (padrão mantém pastas com carimbo de data/hora).
- `--skip-build` - reutilizar um `docs/portal/build` existente (utilizando CI não pode
  reconstruir por espelhos offline).
- `--skip-sync-openapi` - omitir `npm run sync-openapi` quando `cargo xtask openapi`
  não é possível baixar crates.io.
- `--skip-sbom` - evita chamar `syft` quando o binário não está instalado (o script imprime
  uma advertência em seu lugar).
- `--proof` - executa `sorafs_cli proof verify` para cada par CAR/manifica. As cargas úteis de
  múltiplos arquivos que requerem suporte de chunk-plan na CLI, assim que você deixar este sinalizador
  não estabeleça se houver erros de `plan chunk count` e verifique manualmente quando
  llegue el gate rio acima.
- `--sign` - invoque `sorafs_cli manifest sign`. Prove um token con
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou deixe que a CLI seja obtida usando
  `--sigstore-provider/--sigstore-audience`.

Cuando inveja dos artefatos de produção usa `docs/portal/scripts/sorafs-pin-release.sh`.
Agora empaque o portal, OpenAPI e payloads SBOM, firme cada manifestação e registre metadados
ativos extras em `portal.additional_assets.json`. O ajudante entende os mesmos botões opcionais
usados ​​pelo empacotador de CI, mas os novos switches `--openapi-*`, `--portal-sbom-*`, y
`--openapi-sbom-*` para atribuir tuplas de alias por artefato, substituir a fonte SBOM via
`--openapi-sbom-source`, omitir determinadas cargas úteis (`--skip-openapi`/`--skip-sbom`),
e aponte para um binário `syft` sem padrão com `--syft-bin`.

O script expõe cada comando que é executado; copie o log e o ticket de lançamento
junto a `package_summary.json` para que os revisores possam comparar resumos de CAR, metadados de
plano, e hashes do pacote Sigstore sem revisar a saída do shell ad hoc.

## Paso 9 - Verificação de gateway + SoraDNS

Antes de anunciar uma mudança, tente que o alias seja novamente resolvido via SoraDNS e que os gateways
engrapan pruebas frescas:

1. **Ejeta a porta da sonda.** `ci/check_sorafs_gateway_probe.sh` ejeta
   `cargo xtask sorafs-gateway-probe` contra a demonstração de fixtures em
   `fixtures/sorafs_gateway/probe_demo/`. Para explicar a verdade, teste o objetivo do nome do host:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   A sonda decodifica `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status` segun
   `docs/source/sorafs_alias_policy.md` e falhou quando o resumo do manifesto,
   os TTLs ou as ligações GAR são desviadas.

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **Captura de evidência de exercício.** Para exercícios de operador ou simulacros de PagerDuty, consulte
   o probe com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   lançamento do devportal -- ...`. O wrapper guarda cabeçalhos/logs abaixo
   `artifacts/sorafs_gateway_probe/<stamp>/`, atualiza `ops/drill-log.md`, y
   (opcionalmente) vários ganchos de reversão ou payloads PagerDuty. Establece
   `--host docs.sora` para validar a rota SoraDNS em vez de codificar um IP.

3. **Verifique o DNS das ligações.** Quando a governança publica a prova do alias, registre o arquivo GAR
   referenciado pela sonda (`--gar`) e adjuntalo à evidência de liberação. Os proprietários do resolvedor
   você pode refletir a mesma entrada via `tools/soradns-resolver` para garantir que as entradas no cache
   respeten el manifestato nuevo. Antes de adicionar o JSON, execute
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   para que o mapa determinista de host, os metadados do manifesto e as etiquetas de telemetria sejam
   válido offline. O ajudante pode emitir um currículo `--json-out` junto com o GAR firmado para que eles
   Os revisores têm evidências verificáveis sem abrir o binário.
  Quando você redige um GAR novo, prefira
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (Vuelve para `--manifest-cid <cid>` somente quando o arquivo de manifestação não estiver disponível). El ajudante
  agora deriva o CID **y** o resumo BLAKE3 diretamente do manifesto JSON, registrando espaços em branco,
  desduplicar sinalizadores `--telemetry-label` repetidos, ordenar as etiquetas e emitir os modelos por padrão
  de CSP/HSTS/Permissions-Policy antes de escrever o JSON para que a carga útil seja mantida determinista
  mesmo quando os operadores capturam etiquetas de conchas distintas.

4. **Observe as métricas do alias.** Manten `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` na tela enquanto a sonda
   corre; ambas as séries foram gráficadas em `dashboards/grafana/docs_portal.json`.

## Paso 10 - Monitoramento e pacote de evidências- **Painéis.** Exportação `dashboards/grafana/docs_portal.json` (SLOs do portal),
  `dashboards/grafana/sorafs_gateway_observability.json` (latência do gateway +
  saúde de prova), e `dashboards/grafana/sorafs_fetch_observability.json`
  (saúde do orquestrador) para cada lançamento. Adiciona as exportações JSON ao ticket de
  release para que os revisores possam reproduzir as consultas de Prometheus.
- **Arquivos de teste.** Mantenha `artifacts/sorafs_gateway_probe/<stamp>/` no anexo git
  o seu balde de evidências. Inclui o currículo do probe, cabeçalhos e carga útil do PagerDuty
  capturado pelo script de telemetria.
- **Bundle de release.** Guarda os currículos do CAR do portal/SBOM/OpenAPI, bundles de manifestação,
  firmas Sigstore, `portal.pin.report.json`, registra a sonda Try-It e relata a verificação de link
  abaixo uma pasta com carimbo de data/hora (por exemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quando as sondas são parte de uma broca, deixe isso
  `scripts/telemetry/run_sorafs_gateway_probe.sh` concordo com entradas para `ops/drill-log.md`
  para que a mesma evidência satisfaça o requisito de caos SNNet-5.
- **Links de ticket.** Referência aos IDs do painel de Grafana ou exporta PNG adjuntos no ticket
  de mudança, junto com a rota do relatório de sondagem, para que os revisores possam cruzar os SLOs
  sem acessar uma concha.

## Paso 11 - Exercício de busca multifonte e evidência de placar

Publicar em SoraFS agora requer evidência de fetch multi-source (DOCS-7/SF-6)
junto com as testes de DNS/gateway anteriores. Depois de estabelecer o manifesto:

1. **Ejecuta `sorafs_fetch` contra o manifesto ao vivo.** Use os mesmos artefatos de plano/manifesto
   produzidos em los Pasos 2-3 mas as credenciais de gateway emitidas para cada provedor. Persistir
   toda a saída para que os auditores possam reproduzir o rastro de decisão do orquestrador:

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

   - Procure primeiro os anúncios de fornecedores referenciados pelo manifesto (por exemplo
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     e passe via `--provider-advert name=path` para que o placar possa avaliar as janelas
     de capacidade de forma determinista. EUA
     `--allow-implicit-provider-metadata` **solo** quando reproduz fixtures em CI; os treinos
     de produção deve citar os anúncios firmados que foram lançados com o pino.
   - Quando o manifesto faz referência a regiões adicionais, repita o comando com as tuplas de
     provedores correspondentes para que cada cache/alias tenha um artefato de busca associado.

2. **Arquivar as saídas.** Guarda `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, e `chunk_receipts.ndjson` abaixo da pasta de evidências do
   lançamento. Esses arquivos capturam a ponderação de pares, o orçamento de nova tentativa, a latência EWMA e os
   receitas por parte que o pacote de governança deve ser retido para SF-7.

3. **Atualiza a telemetria.** Importe as saídas de busca no painel **SoraFS Fetch
   Observabilidade** (`dashboards/grafana/sorafs_fetch_observability.json`),
   observando `torii_sorafs_fetch_duration_ms`/`_failures_total` e os painéis de rango por provedor
   para anomalias. Coloque os instantâneos do painel Grafana no ticket de lançamento junto com a rota do
   placar.

4. **Faça as regras de alerta.** Execução `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   para validar o pacote de alertas Prometheus antes de encerrar o lançamento. Adjunta la salida de
   promotool al ticket para que os revisores DOCS-7 confirmem que os alertas de travamento e
   provedor lento segue armadas.

5. **Cabe em CI.** O fluxo de trabalho de pin do portal mantém um passo `sorafs_fetch` atrás da entrada
   `perform_fetch_probe`; habilitalo para execuções de encenação/produção para que a evidência de
   buscar se produz junto com o pacote de manifestação sem manual de intervenção. Los treinos locais pueden
   reutilizar o mesmo script exportando tokens de gateway e estabelecendo `PIN_FETCH_PROVIDERS`
   na lista de fornecedores separados por vírgulas.

## Promoção, observação e reversão

1. **Promoção:** manter alias separados para encenação e produção. Promociona reejecutando
   `manifest submit` com o mesmo manifesto/pacote, mudando
   `--alias-namespace/--alias-name` para apontar o alias de produção. Isso evita
   reconstruir ou reafirmar uma vez que o QA verifique o pino de teste.
2. **Monitoramento:** importa o painel do pin-registry
   (`docs/source/grafana_sorafs_pin_registry.json`) mas as sondas específicas do portal
   (ver `docs/portal/docs/devportal/observability.md`). Alerta de desvio de somas de verificação,
   sondas falhadas ou picos de nova tentativa de prova.
3. **Rollback:** para reverter, reenviar o manifesto anterior (ou retirar o alias atual) usando
   `sorafs_cli manifest submit --alias ... --retire`. Mantenha sempre o último pacote
   conhecido como bom e o currículo CAR para que as tentativas de reversão possam recriar si
   os logs de CI são girados.## Planta de fluxo de trabalho CI

Como mínimo, seu pipeline deve:

1. Build + lint (`npm ci`, `npm run build`, geração de somas de verificação).
2. Empaquetar (`car pack`) e manifestos computacionais.
3. Firme usando o token OIDC do trabalho (`manifest sign`).
4. Subir artefatos (CAR, manifesto, pacote, plano, resumos) para auditórios.
5. Envie para o registro do PIN:
   - Solicitações pull -> `docs-preview.sora`.
   - Tags/filiais protegidas -> promoção de alias de produção.
6. Execução de sondas + portões de verificação de prova antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todos esses passos para lançamentos manuais.
O fluxo de trabalho:

- construir/testar o portal,
- empaque o build via `scripts/sorafs-pin-release.sh`,
- firmar/verificar o pacote de manifestação usando GitHub OIDC,
- sube el CAR/manificato/bundle/plan/resumenes como artefatos, y
- (opcionalmente) envia o manifesto + alias vinculativo quando há segredos.

Configure os seguintes segredos/variáveis do repositório antes de disparar o trabalho:

| Nome | Proposta |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expõe `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrada com submissões. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridade de firma para o envio do manifesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias inserida na manifestação quando `perform_submit` é `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Pacote de prova de alias codificado em base64 (opcional; omitir para saltar a ligação de alias). |
| `DOCS_ANALYTICS_*` | Endpoints de análise/sonda existentes reutilizados por outros fluxos de trabalho. |

Dispara o fluxo de trabalho por meio da UI de Ações:

1. Prove `alias_label` (por exemplo, `docs.sora.link`), `proposal_alias` opcional,
   e uma substituição opcional de `release_tag`.
2. Deixe `perform_submit` sem marcar para gerar artefatos sem tocar Torii
   (util para testes) ou habilite-o para publicar diretamente no alias configurado.

`docs/source/sorafs_ci_templates.md` na documentação dos auxiliares de CI genéricos para
projetos fora deste repositório, mas o fluxo de trabalho do portal deve ser a opção preferida
para lançamentos del dia a dia.

## Lista de verificação

- [ ] `npm run build`, `npm run test:*`, e `npm run check:links` estão em verde.
- [ ] `build/checksums.sha256` e `build/release.json` capturados em artefatos.
- [ ] CAR, planejar, manifestar e retomar gerados abaixo de `artifacts/`.
- [ ] Bundle Sigstore + firma separada armazenada com logs.
- [ ] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json`
      capturados quando há envios.
- [ ] `portal.pin.report.json` (e opcional `portal.pin.proposal.json`)
      arquivado junto com os artefatos CAR/manifiedsto.
- [ ] Logs de `proof verify` e `manifest verify-signature` arquivados.
- [ ] Dashboards de Grafana atualizados + probes Try-It exitosos.
- [ ] Notas de reversão (ID de manifestação anterior + resumo de alias) adjuntas al
      bilhete de lançamento.