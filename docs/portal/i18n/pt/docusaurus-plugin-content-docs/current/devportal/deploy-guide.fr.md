---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Vista do conjunto

Este manual converte os itens do roteiro **DOCS-7** (publicação SoraFS) e **DOCS-8**
(automatização do pino CI/CD) em um procedimento acionável para o desenvolvimento do portal.
Ele cobre a fase build/lint, a embalagem SoraFS, a assinatura de manifestos com Sigstore,
a promoção de alias, a verificação e os exercícios de reversão para cada visualização e lançamento
portanto, reproduzível e auditável.

O fluxo supõe que você tenha o binário `sorafs_cli` (compile com `--features cli`), acesse um
um endpoint Torii com permissões de registro de pinos e credenciais OIDC para Sigstore.
Armazene os segredos de longa duração (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) em
seu cofre CI; Os locais de execução podem carregar o carregador por meio das exportações do shell.

## Pré-requisito

- Nó 18.18+ com `npm` ou `pnpm`.
-`sorafs_cli` via `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii que expõe `/v1/sorafs/*` mais uma conta/cle privee d'autorite que pode ser solicitada
  des manifestes et des alias.
- Emmeteur OIDC (GitHub Actions, GitLab, identidade de carga de trabalho, etc.) para emitir um `SIGSTORE_ID_TOKEN`.
Opcional: `examples/sorafs_cli_quickstart.sh` para funcionamento a seco e
  `docs/source/sorafs_ci_templates.md` para scaffolding de fluxos de trabalho GitHub/GitLab.
- Configure as variáveis ​​OAuth Try it (`DOCS_OAUTH_*`) e execute-as
  [lista de verificação de reforço de segurança](./security-hardening.md) antes de promover uma compilação
  fora do laboratório. A construção do portal echoue é mantida quando essas variáveis são mantidas
  ou quando os botões TTL/polling classificam as janelas impostas; exportar
  `DOCS_OAUTH_ALLOW_INSECURE=1` exclusivo para pré-visualizações de locais de jato. Joignez
  as prévias do pen-test no ticket de lançamento.

## Etapa 0 - Capture um pacote de proxy Experimente

Antes de promover uma visualização do Netlify ou do gateway, veja as fontes do proxy Try it
e o resumo do manifesto OpenAPI é assinado em um pacote determinado:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copia os auxiliares proxy/sonda/rollback, verifica a assinatura
OpenAPI e escrito `release.json` mais `checksums.sha256`. Junte-se a este pacote no ticket de
promoção Netlify/SoraFS gateway para que os revisores possam recuperar as fontes exatas
e as dicas do alvo Torii sem reconstrução. Le bundle registrado também se os portadores
fornecido pelo cliente etaient actives (`allow_client_auth`) para gerenciar o plano de implementação
e as regras CSP em fase.

## Etapa 1 - Construa e faça o portal

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

- `build/checksums.sha256` - manifesto SHA256 para `sha256sum -c`.
- `build/release.json` - metadados (`tag`, `generated_at`, `source`) corrigidos em cada CAR/manifeste.

Arquive esses arquivos com o currículo CAR para que os revisores possam comparar os artefatos
visualização sem reconstrução.

## Etapa 2 - Empacotar estatísticas de ativos

Execute o packer CAR com o repertório de surtida Docusaurus. L'exemplo ci-dessous ecrit
os artefatos sob `artifacts/devportal/`.

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

O currículo JSON captura as contas de pedaços, os resumos e as dicas de planejamento de prova
que `manifest build` e os painéis CI são reutilizados mais tarde.

## Etapa 2b - Empaque os companheiros OpenAPI e SBOM

DOCS-7 exige que o site do portal seja publicado, o snapshot OpenAPI e as cargas úteis SBOM
como os manifestos distintos para que os gateways possam gravar os cabeçalhos
`Sora-Proof`/`Sora-Content-CID` para cada artefato. Le helper de lançamento
(`scripts/sorafs-pin-release.sh`) empaquete deixa o repertório OpenAPI
(`static/openapi/`) e os SBOMs são emitidos via `syft` nos CARs separados
`openapi.*`/`*-sbom.*` e registre metadonnes em
`artifacts/sorafs/portal.additional_assets.json`. Em fluxo Manuel,
repita as etapas 2-4 para cada carga útil com seus próprios prefixos e rótulos de metadados
(por exemplo `--car-out "$OUT"/openapi.car` mais
`--metadata alias_label=docs.sora.link/openapi`). Registrar cada par de manifesto/alias
em Torii (site, OpenAPI, SBOM portail, SBOM OpenAPI) antes de bascular o DNS para que
la gateway serve des preuves agrafees para todos os artefatos públicos.

## Etapa 3 - Construindo o manifesto

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

Ajuste os sinalizadores de pin-policy de acordo com sua janela de lançamento (por exemplo, `--pin-storage-class
quente para os canários). A variante JSON é a opção mais prática para revisão de código.

## Etapa 4 - Signatário com Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```O pacote registra o resumo do manifesto, os resumos dos pedaços e um hash BLAKE3 do token
OIDC sem persistir o JWT. Guarde o pacote e a assinatura separada; as promoções de
a produção pode reutilizar os artefatos dos memes em vez de re-assinar. Locais de execução
pode substituir os sinalizadores do provedor par `--identity-token-env` (ou definir `SIGSTORE_ID_TOKEN`
) quando um auxiliar OIDC externo emite o token.

## Etape 5 - Registro Soumettre au pin

Insira o manifesto assinado (e o plano de pedaços) em Torii. Exija sempre um currículo para
que a entrada/alias resulta tão auditável.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority soraカタカナ... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Ao lançar a visualização ou o canary (`docs-preview.sora`), repita o lançamento
com um alias exclusivo para que o controle de qualidade possa verificar o conteúdo antes da produção da promoção.

A ligação do alias requer três títulos: `--alias-namespace`, `--alias-name` e `--alias-proof`.
A governança produz o pacote de prova (base64 ou bytes Norito) quando a demanda de alias é
aprovado; armazene-o nos segredos CI e exponha-o como arquivo antes de invocar
`manifest submit`. Deixe as bandeiras do apelido de vídeo quando você quiser apenas pinner
le manifeste sans toucher au DNS.

## Etapa 5b - Gerar uma proposta de governança

Cada manifesto doit voyager com uma proposta feita ao Parlamento para que todos os cidadãos
Sora puisse introduzir a mudança sem a identificação de privilégios de credenciais. Depois deles
etapes enviar/assinar, lancez:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura a instrução canônica `RegisterPinManifest`,
o resumo dos pedaços, a política e o índice de alias. Joignez-le au ticket de governança ou au
portal do Parlamento para que os delegados possam comparar a carga útil sem reconstrução.
Como o comando ne touche jamais la cle d'autorite Torii, tout citoyen pode rediger la
localização da proposição.

## Etapa 6 - Verificando as provas e a telemetria

Após o pino, execute as etapas de verificação determinadas:

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

- Vigilância `torii_sorafs_gateway_refusals_total` e
  `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalias.
- Execute `npm run probe:portal` para exercer o proxy Try-It e as garantias registradas
  contre le contentu fraichement pinne.
- Capture as evidências de monitoramento descritas em
  [Publicação e monitoramento](./publishing-monitoring.md) para a porta de observação
  DOCS-3c foi satisfeito com as etapas de publicação. Le helper aceita manutenção
  entradas adicionais `bindings` (site, OpenAPI, portal SBOM, SBOM OpenAPI) e aplicar
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` no hotel através do guarda opcional
  `hostname`. A invocação ci-dessous ecrit um currículo JSON exclusivo e o pacote de evidências
  (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) no repertório
  de lançamento:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Etapa 6a - Planeje os certificados do gateway

Derivar o plano SAN/desafio TLS antes de criar pacotes GAR para equipar gateway e
Os aprovadores do DNS examinaram as evidências do meme. O novo ajudante reflete as entradas DG-3
enumerando os hosts curinga canônicos, os SANs pretty-host, os rótulos DNS-01 e
Os desafios que a ACME recomenda:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Comprometa o JSON com o pacote de lançamento (ou carregue-o com o ticket de alteração) para
que os operadores podem coletar os valores SAN na configuração
`torii.sorafs_gateway.acme` de Torii e que os revisores GAR possam confirmar os
mapeamentos canônicos/pretty sem reexecutar as derivações de hotéis. Adicionar argumentos
`--name` suplementos para cada sufixo promovido no lançamento do meme.

## Etapa 6b - Derivando os mapeamentos dos hotéis canônicos

Antes de modelar as cargas úteis GAR, registre o mapeamento do ponto determinado para cada alias.
`cargo xtask soradns-hosts` hashe cada `--name` e seu rótulo canonique
(`<base32>.gw.sora.id`), emita o requisito curinga (`*.gw.sora.id`) e derive o host bonito
(`<alias>.gw.sora.name`). Persista a surtida nos artefatos de liberação para que eles
revisores DG-3 podem comparar o mapeamento com o soumission GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>` para ecoar quando um GAR ou um JSON de
gateway de ligação preenche um dos requisitos de hosts. Le helpe aceita arquivos adicionais de verificação,
isso facilita o fiapo do modelo GAR e o gráfico `portal.gateway.binding.json` em
a invocação do meme:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Use o JSON de currículo e o log de verificação do ticket de alteração DNS/gateway para isso
os auditores podem confirmar os hosts canônicos, curinga e muito sem reexecutar os
roteiros. Execute novamente o comando quando os novos alias forem adicionados ao pacote para que eles
atualiza GAR heritente da evidência do meme.

## Etapa 7 - Gerando o descritor de substituição de DNS

A produção de cortes requer um pacote de mudança auditável. Depois de uma entrega
reussie (binding d'alias), le helper emet
`artifacts/sorafs/portal.dns-cutover.json`, capturante:

- metadados de ligação de alias (namespace/nome/prova, resumo do manifesto, URL Torii,
  época soumis, autoridade);
- contexto de lançamento (tag, alias label, chemins manifeste/CAR, plan de chunks, bundle Sigstore);
- ponteiros de verificação (sonda de comando, alias + endpoint Torii); et
- opções de campeões de controle de alterações (ticket de identificação, corte de fenetre, operações de contato,
  produção de nome de host/zona);
- metadados de promoção de rota derivados do cabeçalho `Sora-Route-Binding`
  (host canônico/CID, caminhos de cabeçalho + ligação, comandos de verificação), garantindo que la
  promoção GAR e os exercícios de fallback referenciando a evidência do meme;
- os artefatos de planos de rota genéricos (`gateway.route_plan.json`,
  modelos de cabeçalhos e opções de reversão de cabeçalhos) para que os tickets de alteração e
  ganchos de lint CI podem verificar se cada pacote DG-3 faz referência aos planos de
  promoção/reversão canônicas antes da aprovação;
- opções de metadados para invalidação de cache (endpoint de purga, autenticação variável, carga útil JSON,
  e exemplo de comando `curl`); et
- dicas de rollback apontadas para o descritor precedente (tag de lançamento e resumo do manifesto)
  para que os tickets capturem um caminho de fallback determinado.

Quando a liberação requer limpeza de cache, gere um plano canônico com o descritor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Use o pacote `portal.cache_plan.json` DG-3 para que os operadores estejam direcionados para hosts/caminhos
 deterministas (e as dicas correspondentes de autenticação) quando emitem as solicitações `PURGE`.
As opções de cache de seção do descritor podem referenciar este arquivo diretamente, observar
os revisores alinhados nos endpoints são exatamente expurgados durante uma transição.

Cada pacote DG-3 também requer uma promoção de lista de verificação + reversão. Generez-le via
`cargo xtask soradns-route-plan` para que os revisores possam rastrear etapas exatas
comprovação, corte e reversão por alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Le `gateway.route_plan.json` emis capture hosts canonices/pretty, rappels de health-check
por etapa, atualizações de ligação GAR, limpeza de cache e ações de reversão. Joignez-le aux
artefatos GAR/encadernação/recorte antes de receber o ticket de troca para que Ops puisse
repetir e validar scripts de etapas de memes.

`scripts/generate-dns-cutover-plan.mjs` alimente este descritor e execute-o automaticamente a partir de então
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

Recupere as opções de metadados por meio das variáveis de ambiente antes de executar o auxiliar de pin:

| Variável | Mas |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID do ticket armazenado no descritor. |
| `DNS_CUTOVER_WINDOW` | Janela de corte ISO8601 (ex: `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Produção de nome de host + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias ​​on-call ou contato d'escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint purge cache registrado no descritor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contém a eliminação do token (padrão: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Chemin ver o descritor precedente para reversão de metadados. |

Execute o JSON na revisão da alteração do DNS para que os aprovadores possam verificar os resumos
manifestos, ligações de alias e comandos investigam sem sujar os logs CI. As sinalizações CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` e `--previous-dns-plan` fornecem substituições de memes
quando o ajudante está fora do CI.

## Etapa 8 - Emita o esquema do arquivo de zona do resolvedor (opcional)Quando a janela de transição da produção é iniciada, o script de lançamento pode ser lançado
o esqueleto do arquivo de zona SNS e o resolvedor de trechos automaticamente. Passe-os
registra desejos e metadados de DNS por meio de variáveis de ambiente ou opções CLI; o ajudante
appelle `scripts/sns_zonefile_skeleton.py` imediatamente após a geração do descritor.
Forneça pelo menos um valor A/AAAA/CNAME e o resumo GAR (BLAKE3-256 da carga útil GAR).
Se a zona/nome do host for conhecida e `--dns-zonefile-out` for omitida, o ajudante será escrito em
`artifacts/sns/zonefiles/<zone>/<hostname>.json` e remplit
`ops/soradns/static_zones.<hostname>.json` resolvedor de trecho de código.

| Variável/sinalizador | Mas |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | O caminho do arquivo de zona zoneado é gerado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Caminho do resolvedor de snippet (padrão: `ops/soradns/static_zones.<hostname>.json` se omitido). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL applique aux records generes (padrão: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Endereços IPv4 (env separado por virgules ou flag CLI repetível). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Endereços IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Opção CNAME de destino. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinos SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas adicionais TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Substituir o rótulo da versão do cálculo do arquivo de zona. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Force o carimbo de data/hora `effective_at` (RFC3339) no lugar da estreia da janela de corte. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Substitua a prova literal registrada nos metadados. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Substituir o CID registrado nos metadados. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Etat de freeze guardião (suave, duro, descongelamento, monitoramento, emergência). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Bilhete de referência guardião/conselho para congelar. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 para descongelamento. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas congelam adições (env separados por virgules ou flag repetable). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) do sinal GAR da carga útil. Requisito quando as ligações do gateway são apresentadas. |

O fluxo de trabalho GitHub Actions contém esses valores devido aos segredos do repositório para cada produção de pinos
emet automaticamente os artefatos zonefile. Configure os segredos seguintes (os valores peuvent
contenir des listes separaes par virgules pour les champs multivalues):

| Segredo | Mas |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | A produção de nome de host/zona passa para au helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​​​on-call stocke no descritor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registra IPv4/IPv6 como editor. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Opção CNAME de destino. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pinos SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionais. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | O congelamento de metadados é registrado no esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 em hexadecimal do sinal GAR da carga útil. |

Ao desativar `.github/workflows/docs-portal-sorafs-pin.yml`, forneça as entradas
`dns_change_ticket` e `dns_cutover_window` para que o descritor/zonefile herte do
boa janela. Deixe-os ver apenas para os ensaios a seco.

Tipo de invocação (online com o proprietário do runbook SN-7):

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
  ...autres flags...
```

Le helper recupera automaticamente o ticket de mudança como entrada TXT e herança de estreia de
a janela é cortada como timestamp `effective_at` ou override. Para concluir o fluxo de trabalho, veja
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre a delegação DNS pública

O esqueleto do arquivo de zona não define os registros autorizados do
zona. Você deve ainda configurar a delegação NS/DS da zona parente chez
seu registrador ou fornecedor de DNS para que a Internet pública encontre servidores
de nomes.

- Para as transferências para o Apex/TLD, use ALIAS/ANAME (de acordo com o fornecedor) ou
  publique registros A/AAAA apontando para o IP anycast do gateway.
- Para os sub-domínios, publique um CNAME para o host bonito derivado
  (`<fqdn>.gw.sora.name`).
- O hotel canônico (`<hash>.gw.sora.id`) fica no domínio do gateway e
  não é público em sua zona pública.

### Modelo de gateway de cabeçalhos

O ajudante de implantação também é `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, dois artefatos que satisfazem a exigência DG-3
para ligação de conteúdo de gateway:- `portal.gateway.headers.txt` contém o bloco completo de cabeçalhos HTTP (incluindo
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS e seu descritor
  `Sora-Route-Binding`) que os gateways edge devem registrar cada resposta.
- `portal.gateway.binding.json` registra as informações do meme em formato de máquina lisível
  para que os tickets de troca e automação possam comparar
  ligações host/cid sem scraper la sortie shell.

Eles são gerados automaticamente via
`cargo xtask soradns-binding-template`
e capturou o alias, o resumo do manifesto e o gateway do nome do host fornecido a
`sorafs-pin-release.sh`. Para regenerar ou personalizar o bloco de cabeçalhos, lancez:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Passe `--csp-template`, `--permissions-template`, ou `--hsts-template` para substituir
os modelos de cabeçalho padrão quando uma implantação sob diretivas
suplementares; combine-os com os switches `--no-*` existentes para suprimir
uma conclusão de cabeçalho.

Acesse o snippet de cabeçalho para a solicitação de alteração CDN e alimente o documento JSON
au pipeline de gateway de automação para que a promoção de hotel corresponda a
a l'evidence de release.

O script de liberação executa automaticamente o auxiliar de verificação para que eles
os ingressos DG-3 incluem hoje uma evidência recente. Execute novamente o manual si
modifique o JSON da ligação principal:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

O comando decode le payload `Sora-Proof` agrafe, verifique se os metadados
`Sora-Route-Binding` corresponde ao CID do manifesto + nome do host e ecoa vite
se um cabeçalho derivar. Arquive o console de sortie com outros artefatos de
implantação quando você lança o comando fora do CI para que os revisores DG-3
certifique-se de que a ligação seja válida antes da transferência.

> **Integração do descritor DNS:** `portal.dns-cutover.json` embarque maintenant
> uma seção `gateway_binding` aponta vários artefatos (chemins, conteúdo CID,
> status da prova e modelo de cabeçalhos literais) **e** uma estrofe `route_plan`
> aqui referência `gateway.route_plan.json` assim como os modelos de cabeçalhos
> principal e reversão. Incluir esses blocos em cada ticket DG-3 para que eles
> revisores podem comparar os valores exatos `Sora-Name/Sora-Proof/CSP` et
> confirme que os planos de promoção/reversão correspondem ao pacote de evidências
> sem abrir o arquivo build.

## Etapa 9 - Executar monitores de publicação

O item roadmap **DOCS-3c** requer uma evidência para continuar o portal, o proxy Try it et
as ligações do gateway foram interrompidas após um lançamento. Execute o monitor consolidado
apenas apres les Etapes 7-8 et branchez-le a vos probes programas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` carrega o arquivo de configuração (veja
  `docs/portal/docs/devportal/publishing-monitoring.md` para o esquema) e
  executar três verificações: probes de paths portail + validação CSP/Permissions-Policy,
  probes proxy Try it (opção via filho endpoint `/metrics`), e o verificador
  de gateway de ligação (`cargo xtask soradns-verify-binding`) que exige manutenção la
  presença e o valor de atendimento de Sora-Content-CID com verificações de alias/manifeste.
- O comando retorna diferente de zero quando um teste echoue para CI, cron jobs, ou
  Operadores de runbook podem cancelar a liberação antes de promover o alias.
- Passe `--json-out` ecrite um currículo JSON exclusivo com status par cível; `--evidence-dir`
  emet `summary.json`, `portal.json`, `tryit.json`, `binding.json`, e `checksums.sha256`
  para que os revisores de governança possam comparar os resultados sem relançar os monitores.
  Arquive este repertório sob `artifacts/sorafs/<tag>/monitoring/` com o pacote Sigstore
  e o descritor DNS.
- Incluir a saída do monitor, exportar Grafana (`dashboards/grafana/docs_portal.json`),
  e o ID do Drill Alertmanager na liberação do ticket para que o SLO DOCS-3c soit
  auditável mais tardio. O manual de monitoramento da publicação é dedicado a você
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Os testes do portal exigem HTTPS e rejeitam os URLs base `http://`, exceto se
`allowInsecureHttp` é definido no monitor de configuração; produção/encenação gardez les cibles
em TLS e não ative a substituição para pré-visualizações de localidades.

Automatize o monitor via `npm run monitor:publishing` no Buildkite/cron uma vez
o portal online. O meme comandou, apontou para a produção de URLs, alimente as verificações
sante continus entre lançamentos.

## Automatização com `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula as etapas 2-6. Ou:1. arquivo `build/` em um tarball determinado,
2. execute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   e `proof verify`,
3. execute a opção `manifest submit` (incluindo a ligação do alias) quando
   as credenciais Torii são apresentadas, e
4. ecrit `artifacts/sorafs/portal.pin.report.json`, a opção
  `portal.pin.proposal.json`, o descritor DNS (após envio),
  e o pacote de gateway de ligação (`portal.gateway.binding.json` mais o bloco de cabeçalhos)
  para que as equipes de governança, redes e operações possam comparar as evidências
  sem sujar os logs CI.

Definições `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, et (opcional)
`PIN_ALIAS_PROOF_PATH` antes de invocar o script. Use `--skip-submit` para secar
corre; o fluxo de trabalho GitHub ci-dessous bascule ceci por meio da entrada `perform_submit`.

## Etapa 8 - Publique as especificações OpenAPI e os pacotes SBOM

DOCS-7 exige que a construção do portal, a especificação OpenAPI e os artefatos SBOM sejam atravessados
o meme pipeline determina. Os ajudantes existentes cobrem as três:

1. **Regenerar e assinar as especificações.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Obtenha um lançamento de rótulo via `--version=<label>` e você deseja preservar um instantâneo
   histórico (por exemplo `2025-q3`). O ajudante escreve o snapshot em
   `static/openapi/versions/<label>/torii.json`, o espelho em
   `versions/current`, e registre os metadados (SHA-256, status do manifesto, carimbo de data/hora
   um dia) em `static/openapi/versions.json`. Le portail developmentpeur lit cet index
   para que os painéis Swagger/RapiDoc apresentem um seletor de versão e mostrem-no
   digest/la assinatura associada inline. Omettre `--version` conserva os rótulos do lançamento
   precedente e não rafraichit que os ponteiros `current` + `latest`.

   O manifesto captura os resumos SHA-256/BLAKE3 para que o gateway possa agrafar des
   cabeçalhos `Sora-Proof` para `/reference/torii-swagger`.

2. **Emita os SBOMs CycloneDX.** O lançamento do pipeline atende aos SBOMs syft
   selecione `docs/source/sorafs_release_pipeline_plan.md`. Gardez la sortie
   Pres des artefatos de construção:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaque toda carga útil no CAR.**

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

   Siga os memes etapes `manifest build` / `manifest sign` que o site principal,
   ajustando os aliases do artefato (por exemplo, `docs-openapi.sora` para a especificação e
   `docs-sbom.sora` para o pacote SBOM assinado). Manutenção de aliases distintos
   SoraDNS, GARs e limites de reversão de tickets em payload exato.

4. **Enviar e vincular.** Reutilize a autoridade existente e o pacote Sigstore, mais registrado
   a tupla de alias no lançamento da lista de verificação para que os auditores possam seguir o exemplo
   nom Sora mappe vers quel digest manifeste.

Arquivar os manifestos spec/SBOM com o build portail garante que cada ticket seja liberado
contém o conjunto completo de artefatos sem reexecutar o empacotador.

### Auxiliar de automação (script de CI/pacote)

`./ci/package_docs_portal_sorafs.sh` codifique as Etapas 1-8 para o roteiro do item
**DOCS-7** é executável com um único comando. O ajudante:

- executar o requisito de preparação do portal (`npm ci`, sincronizar OpenAPI/norito, testar widgets);
- enviar os CARs e pares manifestos do portal, OpenAPI e SBOM via `sorafs_cli`;
- execute a opção `sorafs_cli proof verify` (`--proof`) e a assinatura Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- depositar todos os artefatos sob `artifacts/devportal/sorafs/<timestamp>/` et
  ecrit `package_summary.json` para que CI/outillage release possa inserir o pacote; et
- rafraichit `artifacts/devportal/sorafs/latest` para ponteiro para a última execução.

Exemplo (pipeline completo com Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Utilitários de sinalizadores:- `--out <dir>` - substitui a raiz dos artefatos (padrão preserva o carimbo de data e hora dos dossiês).
- `--skip-build` - reutiliza um `docs/portal/build` existente (utile quand CI ne peut pas
  reconstruir uma causa de espelhos offline).
- `--skip-sync-openapi` - ignorar `npm run sync-openapi` quando `cargo xtask openapi`
  não pode ser joindre crates.io.
- `--skip-sbom` - evite chamar `syft` quando o binário não for instalado (o log de script
  un avertissement a la place).
- `--proof` - execute `sorafs_cli proof verify` para cada par CAR/manifesto. As cargas úteis
  vários arquivos exigem mais suporte ao plano de blocos na CLI, mas deixe esta bandeira
  desative se você encontrar erros `plan chunk count` e verifique manualmente
  uma foi a porta upstream livre.
- `--sign` - invoque `sorafs_cli manifest sign`. Fornecer um token via
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou solte o CLI do recuperador via
  `--sigstore-provider/--sigstore-audience`.

Para a produção de artefatos, utilize `docs/portal/scripts/sorafs-pin-release.sh`.
O pacote mantém o portal, OpenAPI e SBOM, assinando cada manifesto e registrando-o
os ativos de metadados complementares em `portal.additional_assets.json`. O ajudante
compre os memes knobs opcionais que o empacotador CI e os novos switches
`--openapi-*`, `--portal-sbom-*` e `--openapi-sbom-*` para atribuir tuplas de alias
por artefato, substitua a fonte SBOM via `--openapi-sbom-source`, reflita certas
cargas úteis (`--skip-openapi`/`--skip-sbom`), e um ponteiro para um binário `syft` não padrão
com `--syft-bin`.

O script exibe cada comando executado; copie o log no ticket release
com `package_summary.json` para que os revisores possam comparar os resumos do CAR,
os metadados do plano e os hashes do pacote Sigstore sem fornecer um shell de sortie ad hoc.

## Etape 9 - Gateway de verificação + SoraDNS

Antes de anunciar uma mudança, prove que o novo alias resolve via SoraDNS e que os gateways
agraffent des proves frais:

1. **Executar a sonda do portão.** `ci/check_sorafs_gateway_probe.sh` exerce
   `cargo xtask sorafs-gateway-probe` com demonstração de luminárias em
   `fixtures/sorafs_gateway/probe_demo/`. Para as implementações dos rolos, aponte a sonda
   ver o nome do host disponível:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   A sonda decodifica `Sora-Name`, `Sora-Proof` e `Sora-Proof-Status` separadamente
   `docs/source/sorafs_alias_policy.md` e echoue quando o resumo do manifesto,
   os TTLs ou as ligações derivadas do GAR.

   Para verificações pontuais leves (por exemplo, quando apenas o pacote de ligação
   alterado), execute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   O auxiliar valida o pacote de ligação capturado e é útil para liberação
   tickets que precisam apenas de confirmação de vinculação em vez de um exercício de sondagem completo.

2. **Capture a evidência de perfuração.** Para operar a broca ou executar o PagerDuty,
   envolva a sonda com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   lançamento do devportal -- ...`. Le wrapper armazena cabeçalhos/logs sous
   `artifacts/sorafs_gateway_probe/<stamp>/`, com um dia `ops/drill-log.md`, et
   (opcionalmente) desativa o rollback de hooks ou payloads PagerDuty. Definissez
   `--host docs.sora` para validar o caminho SoraDNS em vez de um código rígido de IP.

3. **Verifique as ligações DNS.** Quando a governança publicar o alias de prova, registre-o
   le fichier GAR reference par le probe (`--gar`) e juignez-le a l'evidence release.
   Os proprietários podem resolver a entrada do meme via `tools/soradns-resolver` para
   certifique-se de que as entradas em cache honram o novo manifesto. Antes de juntar o JSON,
   executez
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   para validar offline o mapeamento do host determinado, o manifesto de metadados e os rótulos
   telemetria. O ajudante pode emitir um currículo `--json-out` com o sinal GAR para que eles
   os revisores apresentam uma evidência verificável sem abrir o binário.
  Quando você redigiz um novo GAR, prefira
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (receba um `--manifest-cid <cid>` somente quando o arquivo se manifestar não for mais válido
  disponível). O ajudante deriva a manutenção do CID ** e ** o resumo do diretório BLAKE3
  a partir do manifesto JSON, cupe os espaços, desduplicar as bandeiras `--telemetry-label`,
  teste os rótulos e emita os modelos CSP/HSTS/Permissions-Policy padrão antes
  Crie o JSON para que a carga útil seja determinada pelo meme se os operadores
  captura rótulos a partir de conchas diferentes.

4. **Alias do observador de métricas.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   et `torii_sorafs_gateway_refusals_total{profile="docs"}` na tela pendente da sonda;
   ces series são gráficos em `dashboards/grafana/docs_portal.json`.

## Etapa 10 - Monitoramento e pacote de evidências- **Painéis.** Exportar `dashboards/grafana/docs_portal.json` (portal de SLOs),
  `dashboards/grafana/sorafs_gateway_observability.json` (gateway de latência +
  prova de sante), et `dashboards/grafana/sorafs_fetch_observability.json`
  (sante orquestrador) para cada lançamento. Execute as exportações JSON no lançamento do ticket
  para que os revisores possam enviar os pedidos Prometheus.
- **Investigação de arquivos.** Conservar `artifacts/sorafs_gateway_probe/<stamp>/` no git-annex
  ou seu balde de evidências. Inclui o teste de currículo, os cabeçalhos e a carga útil do PagerDuty
  capturar telemetria por script.
- **Bundle de release.** Armazene os currículos CAR do portail/SBOM/OpenAPI, os pacotes manifestos,
  assinaturas Sigstore, `portal.pin.report.json`, sonda de logs Try-It, e relatórios de verificação de link
  sob um carimbo de data/hora do dossiê (por exemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Registro de perfuração.** Quando as sondas font partie d'un drill, laissez
  `scripts/telemetry/run_sorafs_gateway_probe.sh` adiciona entradas a `ops/drill-log.md`
  para que a evidência do meme satisfaça o requisito do caos SNNet-5.
- **Liens de ticket.** Consulte os IDs do painel Grafana ou os exports PNG anexados no arquivo
  bilhete de mudança, com o caminho da sonda de rapport, para que os revisores possam
  recuperar SLOs sem acesso ao shell.

## Etapa 11 - Painel de pontuação de busca de múltiplas fontes e evidências

O publicador em SoraFS requer manutenção de uma busca de evidências de múltiplas fontes (DOCS-7/SF-6)
com os testes DNS/gateway ci-dessus. Depois do pino do manifesto:

1. **Lancer `sorafs_fetch` com o manifesto ao vivo.** Utilize os artefatos planejados/manifestados
   produtos nas etapas 2-3 com credenciais de gateway emis para cada provedor.
   Persista em todas as saídas para que os auditores possam recuperar o traço de decisão
   orquestrador:

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

   - Recupere as referências dos provedores de anúncios pelo manifesto (por exemplo
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     e passe-os via `--provider-advert name=path` para que o placar avalie os
     Fenetres de capacite de facon determinista. Utilizar
     `--allow-implicit-provider-metadata` **único** quando você rejouez de fixtures em CI;
     A produção das brocas deve citar os anúncios que acompanham o pino.
   - Quando o manifesto fizer referência às regiões adicionais, repita o comando com eles
     provedores de tuplas correspondentes para cada cache/alias em um artefato fetch associado.

2. **Arquive as saídas.** Guarde `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, e `chunk_receipts.ndjson` no dossiê de evidência
   du lançamento. Esses arquivos capturam os pares de ponderação, o orçamento de nova tentativa, a latência EWMA
   e as receitas por parte do pacote de governança serão retidas para SF-7.

3. **Entre em contato com a telemetria.** Importe as saídas fetch no painel
   **SoraFS Observabilidade de busca** (`dashboards/grafana/sorafs_fetch_observability.json`),
   monitore `torii_sorafs_fetch_duration_ms`/`_failures_total` e os painéis do provedor de faixa
   para as anomalias. Liez les snapshots Grafana au ticket release with the chemin scoreboard.

4. **Teste as regras de alerta.** Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   para validar o pacote de alertas Prometheus antes de encerrar o lançamento. Joignez la sortie
   promotool au ticket para que os revisores DOCS-7 confirmem que os alertas de travamento et
   exércitos restentes de provedores lentos.

5. **Brancher en CI.** O fluxo de trabalho de pin portal guarda uma etapa `sorafs_fetch` traseira
   a entrada `perform_fetch_probe`; activez-le para as execuções de teste/produção até que
   A busca de evidências é produzida com o pacote manifesto sem intervenção manual.
   Les drills locais podem reutilizar o script meme e exportar tokens gateway et
   e definido `PIN_FETCH_PROVIDERS` na lista de provedores separados por virgules.

## Promoção, observação e reversão

1. **Promoção:** gardez des alias distintos para preparação e produção. Promovez en
   reexecutor `manifest submit` com o manifesto/pacote do meme, e alterado
   `--alias-namespace/--alias-name` para ponteiro para o alias de produção. Cela Evite
   reconstruir ou assinar novamente quando o controle de qualidade aprovar o teste de pin.
2. **Monitoramento:** importe o registro de pinos do painel
   (`docs/source/grafana_sorafs_pin_registry.json`) além das especificações do portal de sondas
   (veja `docs/portal/docs/devportal/observability.md`). Alertar sobre desvio de somas de verificação,
   sondas ecoam ou fotos de prova de nova tentativa.
3. **Reversão:** para recuperar o atraso, retome o manifesto precedente (ou retire
   o alias atual) com `sorafs_cli manifest submit --alias ... --retire`.
   Gardez toujours le dernier bundle connu comme bon et le resume CAR pour que les
   as provas de reversão podem ser recriadas se os logs do torneio CI.

## Modelo de fluxo de trabalho CI

No mínimo, seu pipeline deve:1. Build + lint (`npm ci`, `npm run build`, geração de checksums).
2. Empaque (`car pack`) e calcule os manifestos.
3. Assine através do token OIDC do trabalho (`manifest sign`).
4. Carregue os artefatos (CAR, manifesto, pacote, plano, currículos) para auditoria.
5. Registro Soumettre au pin:
   - Solicitações pull -> `docs-preview.sora`.
   - Tags/ramos protegidos -> produção de alias de promoção.
6. Execute as sondas + portas de prova de verificação antes do término.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todas essas etapas para os lançamentos manuais.
O fluxo de trabalho:

- construir/testar o portal,
- empaque o build via `scripts/sorafs-pin-release.sh`,
- assinar/verificar o manifesto do pacote via GitHub OIDC,
- carregue o CAR/manifeste/bundle/plan/resumes como artefatos, etc.
- (opcionalmente) soumet le manifeste + alias binding quando os segredos são apresentados.

Configure os seguintes segredos/variáveis antes de desativar o trabalho:

| Nome | Mas |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expõe `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrado com os envios. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autorite de assinatura para o manifesto de submissão. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | O alias da tupla está no manifesto quando `perform_submit` é `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Codificação de alias à prova de pacote em base64 (opcional; omettre pour sauter alias binding). |
| `DOCS_ANALYTICS_*` | Os endpoints existentes de análise/sondagem são reutilizados por outros fluxos de trabalho. |

Desative o fluxo de trabalho por meio das ações da interface do usuário:

1. Fornece `alias_label` (ex: `docs.sora.link`), uma opção `proposal_alias`,
   e uma opção de substituição `release_tag`.
2. Deixe `perform_submit` desmarcado para gerar artefatos sem tocar Torii
   (útil para simulações) ou ative-o para publicar diretamente no alias configure.

`docs/source/sorafs_ci_templates.md` documenta mais os auxiliares CI genéricos para
os projetos fora do repositório, mas o portal de fluxo de trabalho deve ser a sua preferência
para lançamentos diários.

## Lista de verificação

- [ ] `npm run build`, `npm run test:*`, e `npm run check:links` são verdes.
- [] `build/checksums.sha256` e `build/release.json` captura nos artefatos.
- [ ] CAR, planejar, manifestar e retomar gêneros sob `artifacts/`.
- [] Pacote Sigstore + assinatura destacada armazenada com registros.
- [ ] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json`
      captura quando as submissões são enviadas.
- [] `portal.pin.report.json` (e opcional `portal.pin.proposal.json`)
      arquivo com os artefatos CAR/manifeste.
- [ ] Logs dos arquivos `proof verify` e `manifest verify-signature`.
- [] Painéis Grafana mis a jour + sondas Try-It reussis.
- [] Reversão de notas (ID manifesto precedente + alias de resumo) jointes au
      liberação de ingressos.