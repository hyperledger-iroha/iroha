---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ba0c3ce6956a34a8522ccb0891f5f570da1efbe2f4e8247bb13479d04757192
source_last_modified: "2025-11-20T04:38:39.771757+00:00"
translation_last_reviewed: 2026-01-01
---

## Panorama general

Este playbook convierte los elementos del roadmap **DOCS-7** (publicacion de SoraFS) y **DOCS-8**
(automatizacion de pin de CI/CD) en un procedimiento accionable para el portal de desarrolladores.
Cubre la fase de build/lint, el empaquetado SoraFS, la firma de manifiestos con Sigstore,
la promocion de alias, la verificacion y los drills de rollback para que cada preview y release
sea reproducible y auditable.

El flujo asume que tienes el binario `sorafs_cli` (construido con `--features cli`), acceso a
un endpoint Torii con permisos de pin-registry, y credenciales OIDC para Sigstore. Guarda secretos
de larga vida (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) en tu vault de CI; las
ejecuciones locales pueden cargarlos desde exports del shell.

## Prerequisitos

- Node 18.18+ con `npm` o `pnpm`.
- `sorafs_cli` desde `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que expone `/v1/sorafs/*` mas una cuenta/clave privada de autoridad que pueda enviar manifiestos y alias.
- Emisor OIDC (GitHub Actions, GitLab, workload identity, etc.) para emitir un `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para dry runs y `docs/source/sorafs_ci_templates.md` para scaffolding de workflows de GitHub/GitLab.
- Configura las variables OAuth de Try it (`DOCS_OAUTH_*`) y ejecuta la
  [security-hardening checklist](./security-hardening.md) antes de promocionar un build
  fuera del lab. El build del portal ahora falla cuando estas variables faltan
  o cuando los knobs de TTL/polling quedan fuera de las ventanas aplicadas; exporta
  `DOCS_OAUTH_ALLOW_INSECURE=1` solo para previews locales desechables. Adjunta la
  evidencia de pen-test al ticket de release.

## Paso 0 - Captura un paquete del proxy Try it

Antes de promocionar un preview a Netlify o al gateway, sella las fuentes del proxy Try it y el
digest del manifiesto OpenAPI firmado en un paquete determinista:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copia los helpers de proxy/probe/rollback, verifica la firma
OpenAPI y escribe `release.json` mas `checksums.sha256`. Adjunta este paquete al ticket de
promocion de Netlify/SoraFS gateway para que los revisores puedan reproducir las fuentes exactas
del proxy y las pistas del target Torii sin reconstruir. El paquete tambien registra si los
bearers provistos por el cliente estaban habilitados (`allow_client_auth`) para mantener el plan
de rollout y las reglas CSP en sincronizacion.

## Paso 1 - Build y lint del portal

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

`npm run build` ejecuta automaticamente `scripts/write-checksums.mjs`, produciendo:

- `build/checksums.sha256` - manifiesto SHA256 adecuado para `sha256sum -c`.
- `build/release.json` - metadatos (`tag`, `generated_at`, `source`) fijados en cada CAR/manifiesto.

Archiva ambos archivos junto al resumen CAR para que los revisores puedan comparar artefactos de
preview sin reconstruir.

## Paso 2 - Empaqueta los assets estaticos

Ejecuta el empaquetador CAR contra el directorio de salida de Docusaurus. El ejemplo de abajo
escribe todos los artefactos bajo `artifacts/devportal/`.

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

El resumen JSON captura conteos de chunks, digests y pistas de planeacion de proof que
`manifest build` y los dashboards de CI reutilizan despues.

## Paso 2b - Empaqueta companions OpenAPI y SBOM

DOCS-7 requiere publicar el sitio del portal, el snapshot OpenAPI y los payloads SBOM
como manifiestos distintos para que los gateways puedan engrapar los headers
`Sora-Proof`/`Sora-Content-CID` para cada artefacto. El helper de release
(`scripts/sorafs-pin-release.sh`) ya empaqueta el directorio OpenAPI
(`static/openapi/`) y los SBOMs emitidos via `syft` en CARs separados
`openapi.*`/`*-sbom.*` y registra los metadatos en
`artifacts/sorafs/portal.additional_assets.json`. Al ejecutar el flujo manual,
repite los Pasos 2-4 para cada payload con sus propios prefijos y etiquetas de metadatos
(por ejemplo `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Registra cada par manifiesto/alias
en Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) antes de cambiar DNS para que
el gateway pueda servir pruebas engrapadas para todos los artefactos publicados.

## Paso 3 - Construye el manifiesto

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

Ajusta los flags de politica de pin segun tu ventana de release (por ejemplo, `--pin-storage-class
hot` para canaries). La variante JSON es opcional pero conveniente para code review.

## Paso 4 - Firma con Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

El bundle registra el digest del manifiesto, los digests de chunks y un hash BLAKE3 del token
OIDC sin persistir el JWT. Guarda tanto el bundle como la firma separada; las promociones de
produccion pueden reutilizar los mismos artefactos en lugar de re-firmar. Las ejecuciones locales
pueden reemplazar los flags del provider con `--identity-token-env` (o establecer
`SIGSTORE_ID_TOKEN` en el entorno) cuando un helper OIDC externo emite el token.

## Paso 5 - Envia al pin registry

Envia el manifiesto firmado (y el plan de chunks) a Torii. Solicita siempre un resumen para que
la entrada/alias resultante sea auditable.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority docs@publish \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Cuando se despliega un alias de preview o canary (`docs-preview.sora`), repite el
envio con un alias unico para que QA pueda verificar el contenido antes de la promocion a
produccion.

El binding de alias requiere tres campos: `--alias-namespace`, `--alias-name` y `--alias-proof`.
Governance produce el bundle de proof (base64 o bytes Norito) cuando se aprueba la solicitud
del alias; guardalo en secretos de CI y exponlo como archivo antes de invocar `manifest submit`.
Deja los flags de alias sin establecer cuando solo piensas fijar el manifiesto sin tocar DNS.

## Paso 5b - Genera una propuesta de governance

Cada manifiesto debe viajar con una propuesta lista para Parliament para que cualquier ciudadano
Sora pueda introducir el cambio sin pedir credenciales privilegiadas. Despues de los pasos
submit/sign, ejecuta:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura la instruccion canonica `RegisterPinManifest`,
el digest de chunks, la politica y la pista de alias. Adjuntalo al ticket de governance o al
portal Parliament para que los delegados puedan comparar el payload sin reconstruir los artefactos.
Como el comando nunca toca la clave de autoridad de Torii, cualquier ciudadano puede redactar la
propuesta localmente.

## Paso 6 - Verifica pruebas y telemetria

Despues de fijar, ejecuta los pasos de verificacion determinista:

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

- Verifica `torii_sorafs_gateway_refusals_total` y
  `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalias.
- Ejecuta `npm run probe:portal` para ejercitar el proxy Try-It y los links registrados
  contra el contenido recien fijado.
- Captura la evidencia de monitoreo descrita en
  [Publishing & Monitoring](./publishing-monitoring.md) para que la gate de observabilidad DOCS-3c
  se satisfaga junto a los pasos de publicacion. El helper ahora acepta multiples entradas
  `bindings` (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) y aplica `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  en el host objetivo via el guard opcional `hostname`. La invocacion de abajo escribe tanto un
  resumen JSON unico como el bundle de evidencia (`portal.json`, `tryit.json`, `binding.json` y
  `checksums.sha256`) bajo el directorio de release:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - Planifica certificados del gateway

Deriva el plan de SAN/challenge TLS antes de crear paquetes GAR para que el equipo de gateway y los
aprobadores de DNS revisen la misma evidencia. El nuevo helper refleja los insumos de automatizacion
DG-3 enumerando hosts wildcard canonicos, pretty-host SANs, etiquetas DNS-01 y desafios ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Commitea el JSON junto al bundle de release (o subelo con el ticket de cambio) para que los operadores
puedan pegar los valores SAN en la configuracion `torii.sorafs_gateway.acme` de Torii y los revisores
de GAR puedan confirmar los mapeos canonico/pretty sin re-ejecutar derivaciones de host. Agrega
argumentos `--name` adicionales para cada sufijo promocionado en el mismo release.

## Paso 6b - Deriva mapeos de host canonicos

Antes de templar payloads GAR, registra el mapeo de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` en su etiqueta canonica
(`<base32>.gw.sora.id`), emite el wildcard requerido (`*.gw.sora.id`) y deriva el pretty host
(`<alias>.gw.sora.name`). Persiste la salida en los artefactos de release para que los revisores
DG-3 puedan comparar el mapeo junto al envio GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Usa `--verify-host-patterns <file>` para fallar rapido cuando un JSON de GAR o binding de gateway
omite uno de los hosts requeridos. El helper acepta multiples archivos de verificacion, haciendo
facil lint de tanto la plantilla GAR como el `portal.gateway.binding.json` engrapado en la misma invocacion:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Adjunta el resumen JSON y el log de verificacion al ticket de cambio de DNS/gateway para que
auditores puedan confirmar los hosts canonicos, wildcard y pretty sin re-ejecutar los scripts.
Re-ejecuta el comando cuando se agreguen nuevos alias al bundle para que las actualizaciones GAR
hereden la misma evidencia.

## Paso 7 - Genera el descriptor de cutover DNS

Los cutovers de produccion requieren un paquete de cambio auditable. Despues de un submit exitoso
(binding de alias), el helper emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:

- metadatos de binding de alias (namespace/name/proof, digest del manifiesto, URL Torii,
  epoch enviado, autoridad);
- contexto de release (tag, alias label, rutas de manifiesto/CAR, plan de chunks, bundle Sigstore);
- punteros de verificacion (comando probe, alias + endpoint Torii); y
- campos opcionales de change-control (id de ticket, ventana de cutover, contacto ops,
  hostname/zone de produccion);
- metadatos de promocion de rutas derivados del header `Sora-Route-Binding`
  (host canonico/CID, rutas de header + binding, comandos de verificacion), asegurando que la promocion
  GAR y los drills de fallback referencien la misma evidencia;
- los artefactos de route-plan generados (`gateway.route_plan.json`,
  templates de headers y headers de rollback opcionales) para que los tickets de cambio y hooks de
  lint de CI puedan verificar que cada paquete DG-3 referencia los planes de promocion/rollback
  canonicos antes de la aprobacion;
- metadatos opcionales de invalidacion de cache (endpoint de purge, variable de auth, payload JSON,
  y ejemplo de comando `curl`); y
- hints de rollback apuntando al descriptor previo (tag de release y digest del manifiesto)
  para que los tickets capturen una ruta de fallback determinista.

Cuando el release requiere purgas de cache, genera un plan canonico junto al descriptor de cutover:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Adjunta el `portal.cache_plan.json` resultante al paquete DG-3 para que los operadores
tengan hosts/paths deterministas (y los hints de auth que coinciden) al emitir requests `PURGE`.
La seccion opcional de cache del descriptor puede referenciar este archivo directamente,
manteniendo alineados a los revisores de change-control sobre exactamente que endpoints se limpian
durante un cutover.

Cada paquete DG-3 tambien necesita un checklist de promocion + rollback. Generala via
`cargo xtask soradns-route-plan` para que los revisores de change-control puedan seguir los pasos
exactos de preflight, cutover y rollback por alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

El `gateway.route_plan.json` emitido captura hosts canonicos/pretty, recordatorios de health-check
por etapas, actualizaciones de binding GAR, purgas de cache y acciones de rollback. Incluyelo con
los artefactos GAR/binding/cutover antes de enviar el ticket de cambio para que Ops pueda ensayar y
aprobar los mismos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa este descriptor y se ejecuta automaticamente desde
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

Rellena los metadatos opcionales via variables de entorno antes de ejecutar el helper de pin:

| Variable | Proposito |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID de ticket almacenado en el descriptor. |
| `DNS_CUTOVER_WINDOW` | Ventana de cutover ISO8601 (por ejemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Hostname de produccion + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias on-call o contacto de escalamiento. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de purge de cache registrado en el descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contiene el token de purge (default: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Ruta al descriptor de cutover previo para metadatos de rollback. |

Adjunta el JSON al review de cambio DNS para que los aprobadores puedan verificar digests de manifiesto,
bindings de alias y comandos probe sin tener que revisar logs de CI. Los flags de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, y `--previous-dns-plan` proveen los mismos overrides
cuando se ejecuta el helper fuera de CI.

## Paso 8 - Emite el esqueleto de zonefile del resolver (opcional)

Cuando la ventana de cutover de produccion es conocida, el script de release puede emitir el
esqueleto de zonefile SNS y el snippet de resolver automaticamente. Pasa los registros DNS deseados
y metadatos via variables de entorno o opciones CLI; el helper llamara a
`scripts/sns_zonefile_skeleton.py` inmediatamente despues de generar el descriptor de cutover.
Provee al menos un valor A/AAAA/CNAME y el digest GAR (BLAKE3-256 del payload GAR firmado). Si la
zona/hostname son conocidos y `--dns-zonefile-out` se omite, el helper escribe en
`artifacts/sns/zonefiles/<zone>/<hostname>.json` y llena
`ops/soradns/static_zones.<hostname>.json` como el snippet de resolver.

| Variable / flag | Proposito |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Ruta para el esqueleto de zonefile generado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Ruta del snippet de resolver (default: `ops/soradns/static_zones.<hostname>.json` cuando se omite). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL aplicado a los registros generados (default: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direcciones IPv4 (env separado por comas o flag CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direcciones IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Target CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pines SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionales (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override del label de version de zonefile computado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Fuerza el timestamp `effective_at` (RFC3339) en lugar del inicio de la ventana de cutover. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override del literal proof registrado en los metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override del CID registrado en los metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de freeze de guardian (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia de ticket de guardian/council para freezes. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 para thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de freeze adicionales (env separado por comas o flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) del payload GAR firmado. Requerido cuando hay bindings de gateway. |

El workflow de GitHub Actions lee estos valores desde secrets del repositorio para que cada pin de
produccion emita los artefactos de zonefile automaticamente. Configura los siguientes secrets
(strings pueden contener listas separadas por comas para campos multivalor):

| Secret | Proposito |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Hostname/zona de produccion pasados al helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias on-call almacenado en el descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registros IPv4/IPv6 a publicar. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Target CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pines SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionales. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadatos de freeze registrados en el esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 en hex del payload GAR firmado. |

Al disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proporciona los inputs
`dns_change_ticket` y `dns_cutover_window` para que el descriptor/zonefile hereden la ventana
correcta. Dejarlos en blanco solo cuando ejecutes dry runs.

Invocacion tipica (coincide con el runbook del owner SN-7):

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

El helper automaticamente arrastra el ticket de cambio como entrada TXT y hereda el inicio de la
ventana de cutover como timestamp `effective_at` a menos que se haga override. Para el flujo
operativo completo, ver `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegacion DNS publica

El skeleton de zonefile solo define registros autoritativos de la zona. Aun
debes configurar la delegacion NS/DS de la zona padre en tu registrador o
proveedor DNS para que el internet publico pueda encontrar los nameservers.

- Para cutovers en el apex/TLD, usa ALIAS/ANAME (dependiente del proveedor) o
  publica registros A/AAAA que apunten a las IP anycast del gateway.
- Para subdominios, publica un CNAME hacia el pretty host derivado
  (`<fqdn>.gw.sora.name`).
- El host canonico (`<hash>.gw.sora.id`) permanece bajo el dominio del gateway
  y no se publica dentro de tu zona publica.

### Plantilla de headers del gateway

El helper de deploy tambien emite `portal.gateway.headers.txt` y
`portal.gateway.binding.json`, dos artefactos que satisfacen el requisito DG-3 de
gateway-content-binding:

- `portal.gateway.headers.txt` contiene el bloque completo de headers HTTP (incluyendo
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, y el descriptor
  `Sora-Route-Binding`) que los gateways de borde deben engrapar en cada respuesta.
- `portal.gateway.binding.json` registra la misma informacion en forma legible por maquinas
  para que los tickets de cambio y la automatizacion puedan comparar bindings host/cid sin
  raspar la salida de shell.

Se generan automaticamente via
`cargo xtask soradns-binding-template`
y capturan el alias, el digest del manifiesto y el hostname de gateway que se
pasaron a `sorafs-pin-release.sh`. Para regenerar o personalizar el bloque de headers,
ejecuta:

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
de los templates de headers por default cuando un despliegue necesita directivas adicionales;
combinalos con los switches `--no-*` existentes para eliminar un header por completo.

Adjunta el snippet de headers al request de cambio de CDN y alimenta el documento JSON
al pipeline de automatizacion de gateway para que la promocion real de host coincida con la
evidencia de release.

El script de release ejecuta automaticamente el helper de verificacion para que los
tickets DG-3 siempre incluyan evidencia reciente. Vuelve a ejecutarlo manualmente
cuando edites el binding JSON a mano:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

El comando decodifica el payload `Sora-Proof` engrapado, asegura que el metadata `Sora-Route-Binding`
coincida con el CID del manifiesto + hostname, y falla rapido si algun header se desvia.
Archiva la salida de consola junto a los otros artefactos de despliegue siempre que ejecutes
el comando fuera de CI para que los revisores DG-3 tengan prueba de que el binding fue validado
antes del cutover.

> **Integracion del descriptor DNS:** `portal.dns-cutover.json` ahora incrusta una seccion
> `gateway_binding` apuntando a estos artefactos (rutas, content CID, estado de proof y el
> template literal de headers) **y** una estrofa `route_plan` referenciando
> `gateway.route_plan.json` mas los templates de headers principal y rollback. Incluye esos
> bloques en cada ticket de cambio DG-3 para que los revisores puedan comparar los valores
> exactos de `Sora-Name/Sora-Proof/CSP` y confirmar que los planes de promocion/rollback
> coinciden con el bundle de evidencia sin abrir el archivo de build.

## Paso 9 - Ejecuta monitores de publicacion

El item del roadmap **DOCS-3c** requiere evidencia continua de que el portal, el proxy Try it y los
bindings del gateway se mantienen saludables despues de un release. Ejecuta el monitor consolidado
inmediatamente despues de los Pasos 7-8 y conectalo a tus probes programados:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` carga el archivo de config (ver
  `docs/portal/docs/devportal/publishing-monitoring.md` para el esquema) y
  ejecuta tres checks: probes de paths del portal + validacion de CSP/Permissions-Policy,
  probes del proxy Try it (opcionalmente golpeando su endpoint `/metrics`), y el verificador
  de binding de gateway (`cargo xtask soradns-verify-binding`) que ahora exige la presencia y
  el valor esperado de Sora-Content-CID junto con las verificaciones de alias/manifiesto.
- El comando termina con non-zero cuando falla algun probe para que CI, cron jobs, o operadores
  de runbook puedan detener un release antes de promocionar alias.
- Pasar `--json-out` escribe un resumen JSON unico con estado por target; `--evidence-dir`
  emite `summary.json`, `portal.json`, `tryit.json`, `binding.json` y `checksums.sha256` para que
  los revisores de governance puedan comparar resultados sin re-ejecutar los monitores. Archiva
  este directorio bajo `artifacts/sorafs/<tag>/monitoring/` junto al bundle Sigstore y el
  descriptor de cutover DNS.
- Incluye la salida del monitor, el export de Grafana (`dashboards/grafana/docs_portal.json`),
  y el ID del drill de Alertmanager en el ticket de release para que el SLO DOCS-3c pueda ser
  auditado luego. El playbook dedicado de monitor de publishing vive en
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Los probes del portal requieren HTTPS y rechazan URLs base `http://` a menos que `allowInsecureHttp`
este configurado en la config del monitor; manten targets de produccion/staging en TLS y solo
habilita el override para previews locales.

Automatiza el monitor via `npm run monitor:publishing` en Buildkite/cron una vez que el portal
este en vivo. El mismo comando, apuntado a URLs de produccion, alimenta los checks de salud
continuos que SRE/Docs usan entre releases.

## Automatizacion con `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula los Pasos 2-6. Este:

1. archiva `build/` en un tarball determinista,
2. ejecuta `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   y `proof verify`,
3. opcionalmente ejecuta `manifest submit` (incluyendo alias binding) cuando hay credenciales
   Torii, y
4. escribe `artifacts/sorafs/portal.pin.report.json`, el opcional
  `portal.pin.proposal.json`, el descriptor de cutover DNS (despues de submissions),
  y el bundle de binding de gateway (`portal.gateway.binding.json` mas el bloque de headers)
  para que los equipos de governance, networking y ops puedan comparar el bundle de evidencia
  sin revisar logs de CI.

Configura `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, y (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar el script. Usa `--skip-submit` para dry
runs; el workflow de GitHub descrito abajo alterna esto via el input `perform_submit`.

## Paso 8 - Publica especificaciones OpenAPI y paquetes SBOM

DOCS-7 requiere que el build del portal, la especificacion OpenAPI y los artefactos SBOM viajen
por el mismo pipeline determinista. Los helpers existentes cubren los tres:

1. **Regenera y firma la especificacion.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Pasa una etiqueta de release via `--version=<label>` cuando quieras preservar un snapshot
   historico (por ejemplo `2025-q3`). El helper escribe el snapshot en
   `static/openapi/versions/<label>/torii.json`, lo espeja en
   `versions/current`, y registra los metadatos (SHA-256, estado del manifiesto, y
   timestamp actualizado) en `static/openapi/versions.json`. El portal de desarrolladores
   lee ese indice para que los paneles Swagger/RapiDoc puedan presentar un selector de version
   y mostrar el digest/firma asociado en linea. Omitir `--version` mantiene las etiquetas del
   release previo intactas y solo refresca los punteros `current` + `latest`.

   El manifiesto captura digests SHA-256/BLAKE3 para que el gateway pueda engrapar
   headers `Sora-Proof` para `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** El pipeline de release ya espera SBOMs basados en syft
   segun `docs/source/sorafs_release_pipeline_plan.md`. Manten la salida
   junto a los artefactos de build:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueta cada payload en un CAR.**

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

   Sigue los mismos pasos de `manifest build` / `manifest sign` que el sitio principal,
   ajustando alias por artefacto (por ejemplo, `docs-openapi.sora` para la especificacion y
   `docs-sbom.sora` para el bundle SBOM firmado). Mantener alias distintos mantiene SoraDNS,
   GARs y tickets de rollback acotados al payload exacto.

4. **Submit y bind.** Reutiliza la autoridad existente y el bundle Sigstore, pero registra
   el tuple de alias en el checklist de release para que los auditores rastreen que nombre Sora
   mapea a que digest de manifiesto.

Archivar los manifiestos de spec/SBOM junto al build del portal asegura que cada ticket de
release contiene el set completo de artefactos sin re-ejecutar el packer.

### Helper de automatizacion (CI/package script)

`./ci/package_docs_portal_sorafs.sh` codifica los Pasos 1-8 para que el item del roadmap
**DOCS-7** pueda ejercitarse con un solo comando. El helper:

- ejecuta la preparacion requerida del portal (`npm ci`, sync OpenAPI/norito, tests de widgets);
- emite los CARs y pares de manifiesto del portal, OpenAPI y SBOM via `sorafs_cli`;
- opcionalmente ejecuta `sorafs_cli proof verify` (`--proof`) y la firma Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deja todos los artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` y
  escribe `package_summary.json` para que CI/tooling de release pueda ingerir el bundle; y
- refresca `artifacts/devportal/sorafs/latest` para apuntar a la ejecucion mas reciente.

Ejemplo (pipeline completo con Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Flags a conocer:

- `--out <dir>` - override del root de artefactos (default mantiene carpetas con timestamp).
- `--skip-build` - reutiliza un `docs/portal/build` existente (util cuando CI no puede
  reconstruir por mirrors offline).
- `--skip-sync-openapi` - omite `npm run sync-openapi` cuando `cargo xtask openapi`
  no puede llegar a crates.io.
- `--skip-sbom` - evita llamar a `syft` cuando el binario no esta instalado (el script imprime
  una advertencia en su lugar).
- `--proof` - ejecuta `sorafs_cli proof verify` para cada par CAR/manifiesto. Los payloads de
  multiples archivos aun requieren soporte de chunk-plan en el CLI, asi que deja este flag
  sin establecer si encuentras errores de `plan chunk count` y verifica manualmente cuando
  llegue el gate upstream.
- `--sign` - invoca `sorafs_cli manifest sign`. Provee un token con
  `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) o deja que el CLI lo obtenga usando
  `--sigstore-provider/--sigstore-audience`.

Cuando envies artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Ahora empaqueta el portal, OpenAPI y payloads SBOM, firma cada manifiesto, y registra metadatos
extra de assets en `portal.additional_assets.json`. El helper entiende los mismos knobs opcionales
usados por el packager de CI mas los nuevos switches `--openapi-*`, `--portal-sbom-*`, y
`--openapi-sbom-*` para asignar tuples de alias por artefacto, override de la fuente SBOM via
`--openapi-sbom-source`, omitir ciertos payloads (`--skip-openapi`/`--skip-sbom`),
y apuntar a un binario `syft` no default con `--syft-bin`.

El script expone cada comando que ejecuta; copia el log en el ticket de release
junto a `package_summary.json` para que los revisores puedan comparar digests de CAR, metadatos de
plan, y hashes del bundle Sigstore sin revisar salida de shell ad hoc.

## Paso 9 - Verificacion de gateway + SoraDNS

Antes de anunciar un cutover, prueba que el alias nuevo resuelve via SoraDNS y que los gateways
engrapan pruebas frescas:

1. **Ejecuta el gate de probe.** `ci/check_sorafs_gateway_probe.sh` ejercita
   `cargo xtask sorafs-gateway-probe` contra los fixtures demo en
   `fixtures/sorafs_gateway/probe_demo/`. Para despliegues reales, apunta el probe al hostname objetivo:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   El probe decodifica `Sora-Name`, `Sora-Proof`, y `Sora-Proof-Status` segun
   `docs/source/sorafs_alias_policy.md` y falla cuando el digest del manifiesto,
   los TTLs o los bindings GAR se desvian.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **Captura evidencia de drill.** Para drills de operador o simulacros de PagerDuty, envuelve
   el probe con `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. El wrapper guarda headers/logs bajo
   `artifacts/sorafs_gateway_probe/<stamp>/`, actualiza `ops/drill-log.md`, y
   (opcionalmente) dispara hooks de rollback o payloads PagerDuty. Establece
   `--host docs.sora` para validar la ruta SoraDNS en lugar de hard-codear una IP.

3. **Verifica bindings DNS.** Cuando governance publica el proof del alias, registra el archivo GAR
   referenciado por el probe (`--gar`) y adjuntalo a la evidencia de release. Los owners de resolver
   pueden reflejar el mismo input via `tools/soradns-resolver` para asegurar que las entradas en cache
   respeten el manifiesto nuevo. Antes de adjuntar el JSON, ejecuta
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   para que el mapeo determinista de host, los metadatos del manifiesto y las etiquetas de telemetry se
   validen offline. El helper puede emitir un resumen `--json-out` junto al GAR firmado para que los
   revisores tengan evidencia verificable sin abrir el binario.
  Cuando redactes un GAR nuevo, prefiere
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (vuelve a `--manifest-cid <cid>` solo cuando el archivo de manifiesto no este disponible). El helper
  ahora deriva el CID **y** el digest BLAKE3 directamente del manifest JSON, recorta espacios en blanco,
  deduplica flags `--telemetry-label` repetidos, ordena las etiquetas, y emite los templates por default
  de CSP/HSTS/Permissions-Policy antes de escribir el JSON para que el payload se mantenga determinista
  incluso cuando los operadores capturan etiquetas desde shells distintas.

4. **Observa metricas de alias.** Manten `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` en pantalla mientras el probe
   corre; ambas series estan graficadas en `dashboards/grafana/docs_portal.json`.

## Paso 10 - Monitoreo y bundle de evidencia

- **Dashboards.** Exporta `dashboards/grafana/docs_portal.json` (SLOs del portal),
  `dashboards/grafana/sorafs_gateway_observability.json` (latencia del gateway +
  salud de proof), y `dashboards/grafana/sorafs_fetch_observability.json`
  (salud del orchestrator) para cada release. Adjunta los exports JSON al ticket de
  release para que los revisores puedan reproducir los queries de Prometheus.
- **Archivos de probe.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex
  o tu bucket de evidencia. Incluye el resumen del probe, headers, y payload PagerDuty
  capturado por el script de telemetry.
- **Bundle de release.** Guarda los resumenes de CAR del portal/SBOM/OpenAPI, bundles de manifiesto,
  firmas Sigstore, `portal.pin.report.json`, logs de probe Try-It, y reportes de link-check
  bajo una carpeta con timestamp (por ejemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Cuando los probes son parte de un drill, deja que
  `scripts/telemetry/run_sorafs_gateway_probe.sh` agregue entradas a `ops/drill-log.md`
  para que la misma evidencia satisfaga el requisito de caos SNNet-5.
- **Links de ticket.** Referencia los IDs de panel de Grafana o exports PNG adjuntos en el ticket
  de cambio, junto con la ruta del reporte de probe, para que los revisores puedan cruzar los SLOs
  sin acceso a shell.

## Paso 11 - Drill de fetch multi-source y evidencia de scoreboard

Publicar en SoraFS ahora requiere evidencia de fetch multi-source (DOCS-7/SF-6)
junto a las pruebas de DNS/gateway anteriores. Despues de fijar el manifiesto:

1. **Ejecuta `sorafs_fetch` contra el manifiesto live.** Usa los mismos artefactos de plan/manifiesto
   producidos en los Pasos 2-3 mas las credenciales de gateway emitidas para cada proveedor. Persiste
   toda la salida para que los auditores puedan reproducir el rastro de decision del orchestrator:

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

   - Busca primero los adverts de proveedores referenciados por el manifiesto (por ejemplo
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     y pasalos via `--provider-advert name=path` para que el scoreboard pueda evaluar ventanas
     de capacidad de forma determinista. Usa
     `--allow-implicit-provider-metadata` **solo** cuando reproduces fixtures en CI; los drills
     de produccion deben citar los adverts firmados que llegaron con el pin.
   - Cuando el manifiesto referencie regiones adicionales, repite el comando con los tuples de
     proveedores correspondientes para que cada cache/alias tenga un artefacto de fetch asociado.

2. **Archiva las salidas.** Guarda `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, y `chunk_receipts.ndjson` bajo la carpeta de evidencia del
   release. Estos archivos capturan el weighting de peers, el retry budget, la latencia EWMA, y los
   receipts por chunk que el paquete de governance debe retener para SF-7.

3. **Actualiza telemetry.** Importa las salidas de fetch en el dashboard **SoraFS Fetch
   Observability** (`dashboards/grafana/sorafs_fetch_observability.json`),
   observando `torii_sorafs_fetch_duration_ms`/`_failures_total` y los paneles de rango por proveedor
   para anomalias. Enlaza los snapshots de panel Grafana al ticket de release junto con la ruta del
   scoreboard.

4. **Smokea las reglas de alerta.** Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   para validar el bundle de alertas Prometheus antes de cerrar el release. Adjunta la salida de
   promtool al ticket para que los revisores DOCS-7 confirmen que las alertas de stall y
   slow-provider siguen armadas.

5. **Cablea en CI.** El workflow de pin del portal mantiene un paso `sorafs_fetch` detras del input
   `perform_fetch_probe`; habilitalo para ejecuciones de staging/produccion para que la evidencia de
   fetch se produzca junto al bundle de manifiesto sin intervencion manual. Los drills locales pueden
   reutilizar el mismo script exportando los tokens de gateway y estableciendo `PIN_FETCH_PROVIDERS`
   a la lista de proveedores separada por comas.

## Promocion, observabilidad y rollback

1. **Promocion:** manten alias separados para staging y produccion. Promociona re-ejecutando
   `manifest submit` con el mismo manifiesto/bundle, cambiando
   `--alias-namespace/--alias-name` para apuntar al alias de produccion. Esto evita
   reconstruir o re-firmar una vez que QA aprueba el pin de staging.
2. **Monitoreo:** importa el dashboard de pin-registry
   (`docs/source/grafana_sorafs_pin_registry.json`) mas los probes especificos del portal
   (ver `docs/portal/docs/devportal/observability.md`). Alerta en drift de checksums,
   probes fallidos o picos de retry de proof.
3. **Rollback:** para revertir, reenvia el manifiesto previo (o retira el alias actual) usando
   `sorafs_cli manifest submit --alias ... --retire`. Manten siempre el ultimo bundle
   conocido como bueno y el resumen CAR para que las pruebas de rollback puedan recrearse si
   los logs de CI se rotan.

## Plantilla de workflow CI

Como minimo, tu pipeline debe:

1. Build + lint (`npm ci`, `npm run build`, generacion de checksums).
2. Empaquetar (`car pack`) y computar manifiestos.
3. Firmar usando el token OIDC del job (`manifest sign`).
4. Subir artefactos (CAR, manifiesto, bundle, plan, summaries) para auditoria.
5. Enviar al pin registry:
   - Pull requests -> `docs-preview.sora`.
   - Tags / branches protegidas -> promocion de alias de produccion.
6. Ejecutar probes + gates de verificacion de proof antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todos estos pasos para releases manuales.
El workflow:

- construye/testea el portal,
- empaqueta el build via `scripts/sorafs-pin-release.sh`,
- firma/verifica el bundle de manifiesto usando GitHub OIDC,
- sube el CAR/manifiesto/bundle/plan/resumenes como artifacts, y
- (opcionalmente) envia el manifiesto + alias binding cuando hay secrets.

Configura los siguientes repository secrets/variables antes de disparar el job:

| Name | Proposito |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expone `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de epoch registrado con submissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de firma para el submit del manifiesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple de alias enlazado al manifiesto cuando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle de proof de alias codificado en base64 (opcional; omite para saltar alias binding). |
| `DOCS_ANALYTICS_*` | Endpoints de analytics/probe existentes reutilizados por otros workflows. |

Dispara el workflow via la UI de Actions:

1. Provee `alias_label` (por ejemplo, `docs.sora.link`), `proposal_alias` opcional,
   y un override opcional de `release_tag`.
2. Deja `perform_submit` sin marcar para generar artefactos sin tocar Torii
   (util para dry runs) o habilitalo para publicar directamente al alias configurado.

`docs/source/sorafs_ci_templates.md` aun documenta los helpers de CI genericos para
proyectos fuera de este repo, pero el workflow del portal debe ser la opcion preferida
para releases del dia a dia.

## Checklist

- [ ] `npm run build`, `npm run test:*`, y `npm run check:links` estan en verde.
- [ ] `build/checksums.sha256` y `build/release.json` capturados en artefactos.
- [ ] CAR, plan, manifiesto, y resumen generados bajo `artifacts/`.
- [ ] Bundle Sigstore + firma separada almacenados con logs.
- [ ] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
      capturados cuando hay submissions.
- [ ] `portal.pin.report.json` (y opcional `portal.pin.proposal.json`)
      archivado junto a los artefactos CAR/manifiesto.
- [ ] Logs de `proof verify` y `manifest verify-signature` archivados.
- [ ] Dashboards de Grafana actualizados + probes Try-It exitosos.
- [ ] Notas de rollback (ID de manifiesto previo + digest de alias) adjuntas al
      ticket de release.
