---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b229520dcf8c5d2f45248dd9f382a8c61638de561bdef770e10730efc263003
source_last_modified: "2026-01-22T15:57:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: deploy-guide
lang: pt
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

## Visao geral

Este playbook converte os itens do roadmap **DOCS-7** (publicacao de SoraFS) e **DOCS-8**
(automacao de pin de CI/CD) em um procedimento acionavel para o portal de desenvolvedores.
Cobre a fase de build/lint, o empacotamento SoraFS, a assinatura de manifestos com Sigstore,
a promocao de alias, a verificacao e os drills de rollback para que cada preview e release
seja reproduzivel e auditavel.

O fluxo assume que voce tem o binario `sorafs_cli` (construido com `--features cli`), acesso a
um endpoint Torii com permissoes de pin-registry, e credenciais OIDC para Sigstore. Guarde segredos
de longa duracao (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) em seu vault de CI; as
execucoes locais podem carrega-los a partir de exports do shell.

## Pre-requisitos

- Node 18.18+ com `npm` ou `pnpm`.
- `sorafs_cli` a partir de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que expoe `/v1/sorafs/*` mais uma conta/chave privada de autoridade que possa enviar manifestos e alias.
- Emissor OIDC (GitHub Actions, GitLab, workload identity, etc.) para emitir um `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para dre runs e `docs/source/sorafs_ci_templates.md` para scaffolding de workflows de GitHub/GitLab.
- Configura as variables OAuth de Tre it (`DOCS_OAUTH_*`) e execute a
 [security-hardening checklist](./security-hardening.md) antes de promover um build
 fuera del lab. O build del portal ahora falla quando estas variables faltan
 o quando os knobs de TTL/polling quedan fuera de as ventanas aplicadas; exporta
 `DOCS_OAUTH_ALLOW_INSECURE=1` apenas para previews locales desechables. Anexe a
 evidencia de pen-test al ticket de release.

## Etapa 0 - Captura um paquete del proxe Tre it

Antes de promover um preview a Netlife o al gateway, sella as fuentes del proxe Tre it e o
digest del manifesto OpenAPI firmado em um paquete determinista:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copia os helpers de proxy/probe/rollback, verifique a assinatura
OpenAPI e escribe `release.json` mas `checksums.sha256`. Anexe este pacote al ticket de
promocao de Netlify/SoraFS gatewae para que os revisores possam reproducir as fuentes exactas
del proxe e as pistas del target Torii sem reconstruir. O paquete tambem registra si os
bearers provistos por o cliente estaban habilitados (`allow_client_auth`) para manter o plan
de rollout e as reglas CSP em sincronizacion.

## Etapa 1 - Build e lint del portal

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

`npm run build` execute automaticamente `scripts/write-checksums.mjs`, produciendo:

- `build/checksums.sha256` - manifesto SHA256 adecuado para `sha256sum -c`.
- `build/release.json` - metadatos (`tag`, `generated_at`, `source`) fijados em cada CAR/manifesto.

Arquive ambos arquivos junto al resumo CAR para que os revisores possam comparar artefactos de
preview sem reconstruir.

## Etapa 2 - Empaqueta os assets estaticos

Execute o empacotamentor CAR contra o directorio de salida de Docusaurus. O exemplo de abajo
escribe todos os artefactos bajo `artifacts/devportal/`.

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

O resumo JSON captura conteos de chunks, digests e pistas de planeacion de proof que
`manifest build` e os dashboards de CI reutilizan depois.

## Etapa 2b - Empaqueta companions OpenAPI e SBOM

DOCS-7 requiere publicar o sitio del portal, o snapshot OpenAPI e os payloads SBOM
como manifestos distintos para que os gateways possam engrapar os headers
`Sora-Proof`/`Sora-Content-CID` para cada artefacto. O helper de release
(`scripts/sorafs-pin-release.sh`) ya empaqueta o directorio OpenAPI
(`static/openapi/`) e os SBOMs emitidos via `syft` em CARs separados
`openapi.*`/`*-sbom.*` e registra os metadatos em
`artifacts/sorafs/portal.additional_assets.json`. Al executar o fluxo manual,
repite os Etapas 2-4 para cada payload com sus propios prefijos e etiquetas de metadatos
(por exemplo `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Registra cada par manifesto/alias
em Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) antes de cambiar DNS para que
o gatewae possa servir provas engrapadas para todos os artefactos publicados.

## Etapa 3 - Construye o manifesto

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

Ajusta os flags de politica de pin segun tu ventana de release (por exemplo, `--pin-storage-class
hot` para canaries). A variante JSON es opcional pero conveniente para code review.

## Etapa 4 - Assinatura com Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```

O bundle registra o digest del manifesto, os digests de chunks e um hash BLAKE3 del token
OIDC sem persistir o JWT. Guarda tanto o bundle como a assinatura separada; as promociones de
produccion podem reutilizar os mesmos artefactos em lugar de re-assinar. As execucoes locais
podem reemplazar os flags del provider com `--identity-token-env` (o establecer
`SIGSTORE_ID_TOKEN` em o entorno) quando um helper OIDC externo emite o token.

## Etapa 5 - Envie al pin registry

Envie o manifesto firmado (e o plan de chunks) a Torii. Solicita siempre um resumo para que
a entrada/alias resultante seja auditavel.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite soraカタカナ... \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

Quando se despliega um alias de preview o canare (`docs-preview.sora`), repite o
envio com um alias unico para que QA possa verificar o contenido antes de a promocao a
produccion.

O binding de alias requiere tres campos: `--alias-namespace`, `--alias-name` e `--alias-proof`.
Governance produce o bundle de proof (base64 o bytes Norito) quando se aprueba a solicitud
del alias; guardalo em segredos de CI e exponlo como archivo antes de invocar `manifest submit`.
Deja os flags de alias sem establecer quando apenas piensas fijar o manifesto sem tocar DNS.

## Etapa 5b - Genera uma propuesta de governance

Cada manifesto debe viajar com uma propuesta lista para Parliament para que cualquier ciudadano
Sora possa introducir o cambio sem pedir credenciais privilegiadas. Depois de os pasos
submit/sign, execute:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura a instruccion canonica `RegisterPinManifest`,
o digest de chunks, a politica e a pista de alias. Adjuntalo al ticket de governance o al
portal Parliament para que os delegados possam comparar o payload sem reconstruir os artefactos.
Como o comando nunca toca a clave de autoridad de Torii, cualquier ciudadano pode redactar a
propuesta localmente.

## Etapa 6 - Verifique provas e telemetria

Depois de fijar, execute os etapas de verificacao determinista:

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
- Execute `npm run probe:portal` para ejercitar o proxe Try-It e os links registrados
 contra o contenido recien fijado.
- Captura a evidencia de monitoramento descrita em
 [Publishing & Monitoring](./publishing-monitoring.md) para que a gate de observabilidad DOCS-3c
 se satisfaga junto a os etapas de publicacao. O helper ahora acepta multiples entradas
 `bindings` (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) e aplica `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 em o host objetivo via o guard opcional `hostname`. A invocacao de abajo escribe tanto um
 resumo JSON unico como o bundle de evidencia (`portal.json`, `tryit.json`, `binding.json` e
 `checksums.sha256`) bajo o directorio de release:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a - Planifica certificados del gateway

Deriva o plan de SAN/challenge TLS antes de crear paquetes GAR para que o equipe de gatewae e os
aprobadores de DNS revisen a mesma evidencia. O novo helper refleja os insumos de automacao
DG-3 enumerando hosts wildcard canonicos, pretty-host SANs, etiquetas DNS-01 e desafios ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```

Commitea o JSON junto al bundle de release (o subelo com o ticket de cambio) para que os operadores
possam pegar os valores SAN em a configuracion `torii.sorafs_gateway.acme` de Torii e os revisores
de GAR possam confirmar os mapeos canonico/prette sem re-executar derivaciones de host. Agrega
argumentos `--name` adicionales para cada sufijo promocionado em o mesmo release.

## Etapa 6b - Deriva mapeos de host canonicos

Antes de templar payloads GAR, registra o mapeo de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` em su etiqueta canonica
(`<base32>.gw.sora.id`), emite o wildcard requerido (`*.gw.sora.id`) e deriva o prette host
(`<alias>.gw.sora.name`). Persiste a salida em os artefactos de release para que os revisores
DG-3 possam comparar o mapeo junto al envio GAR:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Usa `--verify-host-patterns <file>` para fallar rapido quando um JSON de GAR o binding de gateway
omite uno de os hosts requeridos. O helper acepta multiples archivos de verificacao, haciendo
facil lint de tanto a plantilla GAR como o `portal.gateway.binding.json` engrapado em a mesma invocacion:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Anexe o resumo JSON e o log de verificacao al ticket de cambio de DNS/gatewae para que
auditores possam confirmar os hosts canonicos, wildcard e prette sem re-executar os scripts.
Re-execute o comando quando se agreguen novos alias al bundle para que as actualizaciones GAR
hereden a mesma evidencia.

## Etapa 7 - Genera o descriptor de cutover DNS

Os cutovers de produccion requieren um paquete de cambio auditavel. Depois de um submit exitoso
(binding de alias), o helper emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:

- metadatos de binding de alias (namespace/name/proof, digest del manifesto, URL Torii,
 epoch enviado, autoridad);
- contexto de release (tag, alias label, rutas de manifesto/CAR, plan de chunks, bundle Sigstore);
- punteros de verificacao (comando probe, alias + endpoint Torii); e
- campos opcionales de change-control (id de ticket, ventana de cutover, contacto ops,
 hostname/zone de produccion);
- metadatos de promocao de rutas derivados del header `Sora-Route-Binding`
 (host canonico/CID, rutas de header + binding, comandos de verificacao), asegurando que a promocao
 GAR e os drills de fallback referencien a mesma evidencia;
- os artefactos de route-plan generados (`gateway.route_plan.json`,
 templates de headers e headers de rollback opcionales) para que os tickets de cambio e hooks de
 lint de CI possam verificar que cada paquete DG-3 referencia os planes de promocao/rollback
 canonicos antes de a aprobacion;
- metadatos opcionales de invalidacion de cache (endpoint de purge, variable de auth, payload JSON,
 e exemplo de comando `curl`); e
- hints de rollback apuntando al descriptor previo (tag de release e digest del manifesto)
 para que os tickets capturen uma ruta de fallback determinista.

Quando o release requiere purgas de cache, genera um plan canonico junto al descriptor de cutover:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```

Anexe o `portal.cache_plan.json` resultante al paquete DG-3 para que os operadores
tengan hosts/paths deterministas (e os hints de auth que coinciden) al emitir requests `PURGE`.
A secao opcional de cache del descriptor pode referenciar este archivo directamente,
manteniendo alineados a os revisores de change-control sobre exactamente que endpoints se limpian
durante um cutover.

Cada paquete DG-3 tambem necesita um checklist de promocao + rollback. Generala via
`cargo xtask soradns-route-plan` para que os revisores de change-control possam seguir os pasos
exactos de preflight, cutover e rollback por alias:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

O `gateway.route_plan.json` emitido captura hosts canonicos/pretty, recordatorios de health-check
por etapas, actualizaciones de binding GAR, purgas de cache e acciones de rollback. Incluyelo com
os artefactos GAR/binding/cutover antes de enviar o ticket de cambio para que Ops possa ensayar e
aprobar os mesmos etapas com guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa este descriptor e se execute automaticamente desde
`sorafs-pin-release.sh`. Para regenerarlo o personalizarlo manualmente:

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

Rellena os metadatos opcionales via variables de entorno antes de executar o helper de pin:

| Variable | Proposito |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID de ticket almacenado em o descriptor. |
| `DNS_CUTOVER_WINDOW` | Ventana de cutover ISO8601 (por exemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Hostname de produccion + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias on-call o contacto de escalamiento. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de purge de cache registrado em o descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contiene o token de purge (default: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Ruta al descriptor de cutover previo para metadatos de rollback. |

Anexe o JSON al review de cambio DNS para que os aprobadores possam verificar digests de manifesto,
bindings de alias e comandos probe sem tener que revisar logs de CI. Os flags de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, e `--previous-dns-plan` proveen os mesmos overrides
quando se execute o helper fuera de CI.

## Etapa 8 - Emite o esqueleto de zonefile del resolver (opcional)

Quando a ventana de cutover de produccion es conocida, o script de release pode emitir o
esqueleto de zonefile SNS e o snippet de resolver automaticamente. Pasa os registros DNS deseados
e metadatos via variables de entorno o opciones CLI; o helper llamara a
`scripts/sns_zonefile_skeleton.py` inmediatamente depois de generar o descriptor de cutover.
Provee al menos um valor A/AAAA/CNAME e o digest GAR (BLAKE3-256 del payload GAR firmado). Si a
zona/hostname son conocidos e `--dns-zonefile-out` se omite, o helper escribe em
`artifacts/sns/zonefiles/<zone>/<hostname>.json` e llena
`ops/soradns/static_zones.<hostname>.json` como o snippet de resolver.

| Variable / flag | Proposito |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Ruta para o esqueleto de zonefile generado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Ruta del snippet de resolver (default: `ops/soradns/static_zones.<hostname>.json` quando se omite). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL aplicado a os registros generados (default: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direcciones IPv4 (env separado por comas o flag CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direcciones IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Target CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pines SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionales (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override del label de version de zonefile computado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Fuerza o timestamp `effective_at` (RFC3339) em lugar del inicio de a ventana de cutover. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override del literal proof registrado em os metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override del CID registrado em os metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de freeze de guardian (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia de ticket de guardian/council para freezes. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 para thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de freeze adicionales (env separado por comas o flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) del payload GAR firmado. Requerido quando hae bindings de gateway. |

O workflow de GitHub Actions lee estos valores a partir de secrets del repositorio para que cada pin de
produccion emita os artefactos de zonefile automaticamente. Configura os seguintes secrets
(strings podem contener listas separadas por comas para campos multivalor):

| Secret | Proposito |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Hostname/zona de produccion pasados al helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias on-call almacenado em o descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registros IPv4/IPv6 a publicar. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Target CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pines SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionales. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadatos de freeze registrados em o esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 em hex del payload GAR firmado. |

Al disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proporciona os inputs
`dns_change_ticket` e `dns_cutover_window` para que o descriptor/zonefile hereden a ventana
correcta. Dejarlos em blanco apenas quando ejecutes dre runs.

Invocacion tipica (coincide com o runbook del owner SN-7):

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

O helper automaticamente arrastra o ticket de cambio como entrada TXT e hereda o inicio de a
ventana de cutover como timestamp `effective_at` a menos que se haga override. Para o fluxo
operativo completo, ver `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Plantilla de headers del gateway

O helper de deploe tambem emite `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, dos artefactos que satisfacen o requisito DG-3 de
gateway-content-binding:

- `portal.gateway.headers.txt` contiene o bloque completo de headers HTTP (incluyendo
 `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, e o descriptor
 `Sora-Route-Binding`) que os gateways de borde deben engrapar em cada respuesta.
- `portal.gateway.binding.json` registra a mesma informacion em forma legible por maquinas
 para que os tickets de cambio e a automacao possam comparar bindings host/cid sem
 raspar a salida de shell.

Se generan automaticamente via
`cargo xtask soradns-binding-template`
e capturan o alias, o digest del manifesto e o hostname de gatewae que se
pasaron a `sorafs-pin-release.sh`. Para regenerar o personalizar o bloque de headers,
execute:

```bash
cargo xtask soradns-binding-template \
 --manifest artifacts/sorafs/portal.manifest.json \
 --alias docs.sora \
 --hostname docs.sora.link \
 --route-label production \
 --json-out artifacts/sorafs/portal.gateway.binding.json \
 --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pasa `--csp-template`, `--permissions-template`, o `--hsts-template` para override
de os templates de headers por default quando um despliegue necesita directivas adicionales;
combinalos com os switches `--no-*` existentes para eliminar um header por completo.

Anexe o snippet de headers al request de cambio de CDN e alimenta o documento JSON
al pipeline de automacao de gatewae para que a promocao real de host coincida com a
evidencia de release.

O script de release execute automaticamente o helper de verificacao para que os
tickets DG-3 siempre incluyan evidencia reciente. Vuelve a ejecutarlo manualmente
quando edites o binding JSON a mano:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

O comando decodifica o payload `Sora-Proof` engrapado, asegura que o metadata `Sora-Route-Binding`
coincida com o CID del manifesto + hostname, e falla rapido si algun header se desvia.
Arquive a salida de consola junto a os otros artefactos de despliegue siempre que ejecutes
o comando fuera de CI para que os revisores DG-3 tengan prova de que o binding fue validado
antes del cutover.

> **Integracion del descriptor DNS:** `portal.dns-cutover.json` ahora incrusta uma seccion
> `gateway_binding` apuntando a estos artefactos (rutas, content CID, estado de proof e o
> template literal de headers) **e** uma estrofa `route_plan` referenciando
> `gateway.route_plan.json` mas os templates de headers principal e rollback. Incluye esos
> bloques em cada ticket de cambio DG-3 para que os revisores possam comparar os valores
> exactos de `Sora-Name/Sora-Proof/CSP` e confirmar que os planes de promocao/rollback
> coinciden com o bundle de evidencia sem abrir o archivo de build.

## Etapa 9 - Execute monitores de publicacao

O item del roadmap **DOCS-3c** requiere evidencia continua de que o portal, o proxe Tre it e os
bindings del gatewae se mantienen saludables depois de um release. Execute o monitor consolidado
inmediatamente depois de os Etapas 7-8 e conectalo a tus probes programados:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` carga o archivo de config (ver
 `docs/portal/docs/devportal/publishing-monitoring.md` para o esquema) e
 execute tres checks: probes de paths del portal + validacion de CSP/Permissions-Policy,
 probes del proxe Tre it (opcionalmente golpeando su endpoint `/metrics`), e o verificador
 de binding de gatewae (`cargo xtask soradns-verify-binding`) que ahora exige a presencia e
 o valor esperado de Sora-Content-CID junto com as verificaciones de alias/manifesto.
- O comando termina com non-zero quando falla algun probe para que CI, cron jobs, o operadores
 de runbook possam detener um release antes de promover alias.
- Pasar `--json-out` escribe um resumo JSON unico com estado por target; `--evidence-dir`
 emite `summary.json`, `portal.json`, `tryit.json`, `binding.json` e `checksums.sha256` para que
 os revisores de governance possam comparar resultados sem re-executar os monitores. Arquive
 este directorio bajo `artifacts/sorafs/<tag>/monitoring/` junto al bundle Sigstore e o
 descriptor de cutover DNS.
- Incluye a salida del monitor, o export de Grafana (`dashboards/grafana/docs_portal.json`),
 e o ID del drill de Alertmanager em o ticket de release para que o SLO DOCS-3c possa ser
 auditado luego. O playbook dedicado de monitor de publishing vive em
 `docs/portal/docs/devportal/publishing-monitoring.md`.

Os probes del portal requieren HTTPS e rechazan URLs base `http://` a menos que `allowInsecureHttp`
este configurado em a config del monitor; mantenha targets de produccion/staging em TLS e apenas
habilita o override para previews locales.

Automatiza o monitor via `npm run monitor:publishing` em Buildkite/cron uma vez que o portal
este em vivo. O mesmo comando, apuntado a URLs de produccion, alimenta os checks de salud
continuos que SRE/Docs usan entre releases.

## Automacao com `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula os Etapas 2-6. Este:

1. arquive `build/` em um tarball determinista,
2. execute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
 e `proof verify`,
3. opcionalmente execute `manifest submit` (incluyendo alias binding) quando hae credenciais
 Torii, e
4. escribe `artifacts/sorafs/portal.pin.report.json`, o opcional
 `portal.pin.proposal.json`, o descriptor de cutover DNS (depois de submissions),
 e o bundle de binding de gatewae (`portal.gateway.binding.json` mas o bloque de headers)
 para que os equipos de governance, networking e ops possam comparar o bundle de evidencia
 sem revisar logs de CI.

Configura `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, e (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar o script. Usa `--skip-submit` para dry
runs; o workflow de GitHub descrito abajo alterna esto via o input `perform_submit`.

## Etapa 8 - Publica especificaciones OpenAPI e paquetes SBOM

DOCS-7 requiere que o build del portal, a especificacion OpenAPI e os artefactos SBOM viajen
por o mesmo pipeline determinista. Os helpers existentes cubren os tres:

1. **Regenera e assinatura a especificacion.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 Pasa uma etiqueta de release via `--version=<label>` quando quieras preservar um snapshot
 historico (por exemplo `2025-q3`). O helper escribe o snapshot em
 `static/openapi/versions/<label>/torii.json`, lo espeja em
 `versions/current`, e registra os metadatos (SHA-256, estado del manifesto, e
 timestamp actualizado) em `static/openapi/versions.json`. O portal de desenvolvedores
 lee ese indice para que os paneles Swagger/RapiDoc possam presentar um selector de version
 e mostrar o digest/assinatura asociado em linea. Omitir `--version` mantem as etiquetas del
 release previo intactas e apenas refresca os punteros `current` + `latest`.

 O manifesto captura digests SHA-256/BLAKE3 para que o gatewae possa engrapar
 headers `Sora-Proof` para `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** O pipeline de release ya espera SBOMs basados em syft
 segun `docs/source/sorafs_release_pipeline_plan.md`. Mantenha a salida
 junto a os artefactos de build:

 ```bash
 syft dir:build -o json > "$OUT"/portal.sbom.json
 syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
 ```

3. **Empaqueta cada payload em um CAR.**

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

 Sigue os mesmos etapas de `manifest build` / `manifest sign` que o sitio principal,
 ajustando alias por artefacto (por exemplo, `docs-openapi.sora` para a especificacion e
 `docs-sbom.sora` para o bundle SBOM firmado). Manter alias distintos mantem SoraDNS,
 GARs e tickets de rollback acotados al payload exacto.

4. **Submit e bind.** Reutiliza a autoridad existente e o bundle Sigstore, pero registra
 o tuple de alias em o checklist de release para que os auditores rastreen que nombre Sora
 mapea a que digest de manifesto.

Arquivar os manifestos de spec/SBOM junto al build del portal asegura que cada ticket de
release contiene o set completo de artefactos sem re-executar o packer.

### Helper de automacao (CI/package script)

`./ci/package_docs_portal_sorafs.sh` codifica os Etapas 1-8 para que o item del roadmap
**DOCS-7** possa ejercitarse com um apenas comando. O helper:

- execute a preparacion requerida del portal (`npm ci`, sync OpenAPI/norito, tests de widgets);
- emite os CARs e pares de manifesto del portal, OpenAPI e SBOM via `sorafs_cli`;
- opcionalmente execute `sorafs_cli proof verify` (`--proof`) e a assinatura Sigstore
 (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deja todos os artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` e
 escribe `package_summary.json` para que CI/tooling de release possa ingerir o bundle; e
- refresca `artifacts/devportal/sorafs/latest` para apuntar a a execucao mas reciente.

Exemplo (pipeline completo com Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

Flags a conocer:

- `--out <dir>` - override del root de artefactos (default mantem carpetas com timestamp).
- `--skip-build` - reutiliza um `docs/portal/build` existente (util quando CI no pode
 reconstruir por mirrors offline).
- `--skip-sync-openapi` - omite `npm run sync-openapi` quando `cargo xtask openapi`
 no pode llegar a crates.io.
- `--skip-sbom` - evita llamar a `syft` quando o binario no esta instalado (o script imprime
 uma advertencia em su lugar).
- `--proof` - execute `sorafs_cli proof verify` para cada par CAR/manifesto. Os payloads de
 multiples archivos aun requieren soporte de chunk-plan em o CLI, asi que deja este flag
 sem establecer si encuentras errores de `plan chunk count` e verifique manualmente quando
 llegue o gate upstream.
- `--sign` - invoca `sorafs_cli manifest sign`. Provee um token com
 `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) o deja que o CLI lo obtenga usando
 `--sigstore-provider/--sigstore-audience`.

Quando envies artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Ahora empaqueta o portal, OpenAPI e payloads SBOM, assinatura cada manifesto, e registra metadatos
extra de assets em `portal.additional_assets.json`. O helper entiende os mesmos knobs opcionales
usados por o packager de CI mas os novos switches `--openapi-*`, `--portal-sbom-*`, e
`--openapi-sbom-*` para asignar tuples de alias por artefacto, override de a fuente SBOM via
`--openapi-sbom-source`, omitir ciertos payloads (`--skip-openapi`/`--skip-sbom`),
e apuntar a um binario `syft` no default com `--syft-bin`.

O script expoe cada comando que execute; copia o log em o ticket de release
junto a `package_summary.json` para que os revisores possam comparar digests de CAR, metadatos de
plan, e hashes del bundle Sigstore sem revisar salida de shell ad hoc.

## Etapa 9 - Verificacao de gatewae + SoraDNS

Antes de anunciar um cutover, prova que o alias novo resuelve via SoraDNS e que os gateways
engrapan provas frescas:

1. **Execute o gate de probe.** `ci/check_sorafs_gateway_probe.sh` ejercita
 `cargo xtask sorafs-gateway-probe` contra os fixtures demo em
 `fixtures/sorafs_gateway/probe_demo/`. Para despliegues reales, apunta o probe al hostname objetivo:

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 O probe decodifica `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status` segun
 `docs/source/sorafs_alias_policy.md` e falla quando o digest del manifesto,
 os TTLs o os bindings GAR se desvian.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **Captura evidencia de drill.** Para drills de operador o simulacros de PagerDuty, envuelve
 o probe com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
 devportal-rollout -- ...`. O wrapper guarda headers/logs bajo
 `artifacts/sorafs_gateway_probe/<stamp>/`, actualiza `ops/drill-log.md`, e
 (opcionalmente) dispara hooks de rollback o payloads PagerDuty. Establece
 `--host docs.sora` para validar a ruta SoraDNS em lugar de hard-codear uma IP.

3. **Verifique bindings DNS.** Quando governance publica o proof del alias, registra o archivo GAR
 referenciado por o probe (`--gar`) e adjuntalo a a evidencia de release. Os owners de resolver
 podem reflejar o mesmo input via `tools/soradns-resolver` para asegurar que as entradas em cache
 respeten o manifesto novo. Antes de anexar o JSON, execute
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 para que o mapeo determinista de host, os metadatos del manifesto e as etiquetas de telemetre se
 validen offline. O helper pode emitir um resumo `--json-out` junto al GAR firmado para que os
 revisores tengan evidencia verificable sem abrir o binario.
 Quando redactes um GAR novo, prefiere
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (vuelve a `--manifest-cid <cid>` apenas quando o archivo de manifesto no este disponible). O helper
 ahora deriva o CID **e** o digest BLAKE3 directamente del manifest JSON, recorta espacios em blanco,
 deduplica flags `--telemetry-label` repetidos, ordena as etiquetas, e emite os templates por default
 de CSP/HSTS/Permissions-Police antes de escribir o JSON para que o payload se mantenga determinista
 incluso quando os operadores capturan etiquetas a partir de shells distintas.

4. **Observe metricas de alias.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 e `torii_sorafs_gateway_refusals_total{profile="docs"}` em pantalla enquanto o probe
 corre; ambas series estan graficadas em `dashboards/grafana/docs_portal.json`.

## Etapa 10 - Monitoramento e bundle de evidencia

- **Dashboards.** Exporta `dashboards/grafana/docs_portal.json` (SLOs del portal),
 `dashboards/grafana/sorafs_gateway_observability.json` (latencia del gatewae +
 salud de proof), e `dashboards/grafana/sorafs_fetch_observability.json`
 (salud del orchestrator) para cada release. Anexe os exports JSON al ticket de
 release para que os revisores possam reproducir os queries de Prometheus.
- **Archivos de probe.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` em git-annex
 o tu bucket de evidencia. Incluye o resumo del probe, headers, e payload PagerDuty
 capturado por o script de telemetry.
- **Bundle de release.** Guarda os resumos de CAR del portal/SBOM/OpenAPI, bundles de manifesto,
 firmas Sigstore, `portal.pin.report.json`, logs de probe Try-It, e reportes de link-check
 bajo uma carpeta com timestamp (por exemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quando os probes son parte de um drill, deja que
 `scripts/telemetry/run_sorafs_gateway_probe.sh` agregue entradas a `ops/drill-log.md`
 para que a mesma evidencia satisfaga o requisito de caos SNNet-5.
- **Links de ticket.** Referencia os IDs de panel de Grafana o exports PNG adjuntos em o ticket
 de cambio, junto com a ruta del reporte de probe, para que os revisores possam cruzar os SLOs
 sem acesso a shell.

## Etapa 11 - Drill de fetch multi-source e evidencia de scoreboard

Publicar em SoraFS ahora requiere evidencia de fetch multi-source (DOCS-7/SF-6)
junto a as provas de DNS/gatewae anteriores. Depois de fijar o manifesto:

1. **Execute `sorafs_fetch` contra o manifesto live.** Usa os mesmos artefactos de plan/manifesto
 producidos em os Etapas 2-3 mas as credenciais de gatewae emitidas para cada proveedor. Persiste
 toda a salida para que os auditores possam reproducir o rastro de decision del orchestrator:

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

 - Busca primero os adverts de proveedores referenciados por o manifesto (por exemplo
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 e pasalos via `--provider-advert name=path` para que o scoreboard possa evaluar ventanas
 de capacidad de forma determinista. Usa
 `--allow-implicit-provider-metadata` **apenas** quando reproduces fixtures em CI; os drills
 de produccion deben citar os adverts firmados que llegaron com o pin.
 - Quando o manifesto referencie regiones adicionales, repite o comando com os tuples de
 proveedores correspondientes para que cada cache/alias tenga um artefacto de fetch asociado.

2. **Arquive as salidas.** Guarda `scoreboard.json`,
 `providers.ndjson`, `fetch.json`, e `chunk_receipts.ndjson` bajo a carpeta de evidencia del
 release. Estos archivos capturan o weighting de peers, o retre budget, a latencia EWMA, e os
 receipts por chunk que o paquete de governance debe retener para SF-7.

3. **Actualiza telemetry.** Importa as salidas de fetch em o dashboard **SoraFS Fetch
 Observability** (`dashboards/grafana/sorafs_fetch_observability.json`),
 observando `torii_sorafs_fetch_duration_ms`/`_failures_total` e os paneles de rango por proveedor
 para anomalias. Enlaza os snapshots de panel Grafana al ticket de release junto com a ruta del
 scoreboard.

4. **Smokea as reglas de alerta.** Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh`
 para validar o bundle de alertas Prometheus antes de cerrar o release. Anexe a salida de
 promtool al ticket para que os revisores DOCS-7 confirmen que as alertas de stall e
 slow-provider siguen armadas.

5. **Cablea em CI.** O workflow de pin del portal mantem um etapa `sorafs_fetch` detras del input
 `perform_fetch_probe`; habilitalo para execucoes de staging/produccion para que a evidencia de
 fetch se produzca junto al bundle de manifesto sem intervencion manual. Os drills locales podem
 reutilizar o mesmo script exportando os tokens de gatewae e estableciendo `PIN_FETCH_PROVIDERS`
 a a lista de proveedores separada por comas.

## Promocao, observabilidad e rollback

1. **Promocao:** mantenha alias separados para staging e produccion. Promociona re-executando
 `manifest submit` com o mesmo manifesto/bundle, cambiando
 `--alias-namespace/--alias-name` para apuntar al alias de produccion. Esto evita
 reconstruir o re-assinar uma vez que QA aprueba o pin de staging.
2. **Monitoramento:** importa o dashboard de pin-registry
 (`docs/source/grafana_sorafs_pin_registry.json`) mas os probes especificos del portal
 (ver `docs/portal/docs/devportal/observability.md`). Alerta em drift de checksums,
 probes fallidos o picos de retre de proof.
3. **Rollback:** para revertir, reenvia o manifesto previo (o retira o alias actual) usando
 `sorafs_cli manifest submit --alias ... --retire`. Mantenha siempre o ultimo bundle
 conocido como bueno e o resumo CAR para que as provas de rollback possam recrearse si
 os logs de CI se rotan.

## Plantilla de workflow CI

Como minimo, tu pipeline debe:

1. Build + lint (`npm ci`, `npm run build`, generacion de checksums).
2. Empacotar (`car pack`) e computar manifestos.
3. Assinar usando o token OIDC del job (`manifest sign`).
4. Subir artefactos (CAR, manifesto, bundle, plan, summaries) para auditoria.
5. Enviar al pin registry:
 - Pull requests -> `docs-preview.sora`.
 - Tags / branches protegidas -> promocao de alias de produccion.
6. Executar probes + gates de verificacao de proof antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todos estos etapas para releases manuales.
O workflow:

- construye/testea o portal,
- empaqueta o build via `scripts/sorafs-pin-release.sh`,
- assinatura/verifique o bundle de manifesto usando GitHub OIDC,
- sube o CAR/manifesto/bundle/plan/resumos como artifacts, e
- (opcionalmente) envie o manifesto + alias binding quando hae secrets.

Configura os seguintes repositore secrets/variables antes de disparar o job:

| Name | Proposito |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expoe `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de epoch registrado com submissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de assinatura para o submit del manifesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple de alias enlazado al manifesto quando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle de proof de alias codificado em base64 (opcional; omite para saltar alias binding). |
| `DOCS_ANALYTICS_*` | Endpoints de analytics/probe existentes reutilizados por otros workflows. |

Dispara o workflow via a UI de Actions:

1. Provee `alias_label` (por exemplo, `docs.sora.link`), `proposal_alias` opcional,
 e um override opcional de `release_tag`.
2. Deja `perform_submit` sem marcar para generar artefactos sem tocar Torii
 (util para dre runs) o habilitalo para publicar directamente al alias configurado.

`docs/source/sorafs_ci_templates.md` aun documenta os helpers de CI genericos para
proyectos fuera de este repo, pero o workflow del portal debe ser a opcion preferida
para releases del dia a dia.

## Checklist

- [ ] `npm run build`, `npm run test:*`, e `npm run check:links` estan em verde.
- [ ] `build/checksums.sha256` e `build/release.json` capturados em artefactos.
- [ ] CAR, plan, manifesto, e resumo generados bajo `artifacts/`.
- [ ] Bundle Sigstore + assinatura separada almacenados com logs.
- [ ] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json`
 capturados quando hae submissions.
- [ ] `portal.pin.report.json` (e opcional `portal.pin.proposal.json`)
 archivado junto a os artefactos CAR/manifesto.
- [ ] Logs de `proof verify` e `manifest verify-signature` archivados.
- [ ] Dashboards de Grafana actualizados + probes Try-It exitosos.
- [ ] Notas de rollback (ID de manifesto previo + digest de alias) adjuntas al
 ticket de release.
