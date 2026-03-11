---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Panorama general

Este playbook convierte los elementos del roadmap **DOCS-7** (publicación de SoraFS) y **DOCS-8**
(automatización de pin de CI/CD) en un procedimiento accionable para el portal de desarrolladores.
Cubre la fase de build/lint, el empaquetado SoraFS, la firma de manifiestos con Sigstore,
la promoción de alias, la verificación y los ejercicios de rollback para que cada vista previa y lanzamiento
sea reproducible y auditable.

El flujo supone que tienes el binario `sorafs_cli` (construido con `--features cli`), acceso a
un endpoint Torii con permisos de pin-registry, y credenciales OIDC para Sigstore. guarda secretos
de larga vida (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) en tu bóveda de CI; las
ejecuciones locales pueden cargarlos desde exports del shell.

##Requisitos previos- Nodo 18.18+ con `npm` o `pnpm`.
- `sorafs_cli` desde `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que exponen `/v1/sorafs/*` mas una cuenta/clave privada de autoridad que pueda enviar manifiestos y alias.
- Emisor OIDC (GitHub Actions, GitLab, identidad de carga de trabajo, etc.) para emitir un `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para simulacros y `docs/source/sorafs_ci_templates.md` para scaffolding de flujos de trabajo de GitHub/GitLab.
- Configura las variables OAuth de Try it (`DOCS_OAUTH_*`) y ejecuta la
  [lista de verificación de refuerzo de seguridad](./security-hardening.md) antes de promocionar un build
  fuera del laboratorio. El build del portal ahora falla cuando estas variables faltan
  o cuando las perillas de TTL/polling quedan fuera de las ventanas aplicadas; exporta
  `DOCS_OAUTH_ALLOW_INSECURE=1` solo para vistas previas locales desechables. Adjunta la
  evidencia de pen-test al ticket de liberación.

## Paso 0 - Captura un paquete del proxy Pruébalo

Antes de promocionar un avance a Netlify o al gateway, sella las fuentes del proxy Pruébalo y el
resumen del manifiesto OpenAPI firmado en un paquete determinista:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copia los helpers de proxy/probe/rollback, verifica la firma
OpenAPI y escribe `release.json` más `checksums.sha256`. Adjunta este paquete al ticket de
promocion de Netlify/SoraFS gateway para que los revisores puedan reproducir las fuentes exactas
del proxy y las pistas del target Torii sin reconstruir. El paquete tambien registra si los
portadores provistos por el cliente estaban habilitados (`allow_client_auth`) para mantener el plan
de rollout y las reglas CSP en sincronización.

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

`npm run build` ejecuta automáticamente `scripts/write-checksums.mjs`, produciendo:

- `build/checksums.sha256` - manifiesto SHA256 adecuado para `sha256sum -c`.
- `build/release.json` - metadatos (`tag`, `generated_at`, `source`) fijados en cada CAR/manifiesto.

Archiva ambos archivos junto al resumen CAR para que los revisores puedan comparar artefactos de
vista previa sin reconstruir.

## Paso 2 - Empaqueta los activos estaticos

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

El resumen JSON captura contenidos de fragmentos, resúmenes y pistas de planeación de prueba que
`manifest build` y los tableros de CI se reutilizan después.

## Paso 2b - Empaqueta compañeros OpenAPI y SBOMDOCS-7 requiere publicar el sitio del portal, el snapshot OpenAPI y los payloads SBOM
como manifiestos distintos para que los gateways puedan grabar los encabezados
`Sora-Proof`/`Sora-Content-CID` para cada artefacto. El ayudante de liberación
(`scripts/sorafs-pin-release.sh`) ya empaqueta el directorio OpenAPI
(`static/openapi/`) y los SBOMs emitidos vía `syft` en CARs separados
`openapi.*`/`*-sbom.*` y registra los metadatos en
`artifacts/sorafs/portal.additional_assets.json`. Al ejecutar el flujo manual,
repite los Pasos 2-4 para cada carga útil con sus propios prefijos y etiquetas de metadatos
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

Ajusta los flags de política de pin según tu ventana de liberación (por ejemplo, `--pin-storage-class
hot` para canarios). La variante JSON es opcional pero conveniente para la revisión del código.

## Paso 4 - Firma con Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```El paquete registra el digest del manifiesto, los digests de chunks y un hash BLAKE3 del token
OIDC sin persistir el JWT. Guarda tanto el paquete como la firma separada; las promociones de
produccion pueden reutilizar los mismos artefactos en lugar de volver a firmar. Las ejecuciones locales
pueden reemplazar las banderas del proveedor con `--identity-token-env` (o establecer
`SIGSTORE_ID_TOKEN` en el entorno) cuando un ayudante OIDC externo emite el token.

## Paso 5 - Envia al registro pin

Envia el manifiesto firmado (y el plan de trozos) a Torii. Solicita siempre un resumen para que
la entrada/alias resultante sea auditable.

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

Cuando se despliega un alias de previa o canary (`docs-preview.sora`), repite el
envio con un alias unico para que QA pueda verificar el contenido antes de la promocion a
producción.

El enlace de alias requiere tres campos: `--alias-namespace`, `--alias-name` y `--alias-proof`.
Governance produce el paquete de prueba (base64 o bytes Norito) cuando se aprueba la solicitud
del alias; guárdelo en secretos de CI y expónlo como archivo antes de invocar `manifest submit`.
Deja los flags de alias sin establecer cuando solo piensas fijar el manifiesto sin tocar DNS.

## Paso 5b - Genera una propuesta de gobernanzaCada manifiesto debe viajar con una propuesta lista para el Parlamento para que cualquier ciudadano
Sora pueda introducir el cambio sin pedir credenciales privilegiadas. Despues de los pasos
enviar/firmar, ejecuta:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura la instrucción canónica `RegisterPinManifest`,
el digest de chunks, la politica y la pista de alias. Adjuntalo al ticket de gobernancia o al
portal Parlamento para que los delegados puedan comparar la carga útil sin reconstruir los artefactos.
Como el comando nunca toca la clave de autoridad de Torii, cualquier ciudadano puede redactar la
propuesta localmente.

## Paso 6 - Verifica pruebas y telemetria

Después de fijar, ejecuta los pasos de verificación determinista:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Verifica `torii_sorafs_gateway_refusals_total` y
  `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalías.
- Ejecuta `npm run probe:portal` para ejercitar el proxy Try-It y los enlaces registrados.
  contra el contenido recién fijado.
- Captura la evidencia de monitoreo descrita en
  [Publishing & Monitoring](./publishing-monitoring.md) para que la puerta de observabilidad DOCS-3c
  se satisfaga junto a los pasos de publicacion. El ayudante ahora acepta múltiples entradas
  `bindings` (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) y aplicaciones `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  en el host objetivo vía el guardia opcional `hostname`. La invocacion de abajo escribe tanto un
  resumen JSON único como el paquete de evidencia (`portal.json`, `tryit.json`, `binding.json` y
  `checksums.sha256`) bajo el directorio de lanzamiento:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - Planifica certificados del gateway

Deriva el plan de SAN/challenge TLS antes de crear paquetes GAR para que el equipo de gateway y los
Los aprobadores de DNS revisan la misma evidencia. El nuevo ayudante refleja los insumos de automatizacion.
DG-3 enumerando hosts canónicos comodín, SAN Pretty-host, etiquetas DNS-01 y desafíos ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```Commite el JSON junto al paquete de liberación (o subelo con el ticket de cambio) para que los operadores
puedan pegar los valores SAN en la configuración `torii.sorafs_gateway.acme` de Torii y los revisores
de GAR puedan confirmar los mapeos canonico/pretty sin re-ejecutar derivaciones de host. agrega
argumentos `--name` adicionales para cada sufijo promocionado en el mismo comunicado.

## Paso 6b - Deriva mapeos de host canónicos

Antes de las cargas útiles templarias GAR, registra el mapeo de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` en su etiqueta canónica
(`<base32>.gw.sora.id`), emite el comodín requerido (`*.gw.sora.id`) y deriva el Pretty Host
(`<alias>.gw.sora.name`). Persiste la salida en los artefactos de liberación para que los revisores
DG-3 puedan comparar el mapeo junto al envío GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Usa `--verify-host-patterns <file>` para fallar rápido cuando un JSON de GAR o enlace de puerta de enlace
omitir uno de los hosts requeridos. El ayudante acepta múltiples archivos de verificación, haciendo
facil lint de tanto la plantilla GAR como el `portal.gateway.binding.json` grabado en la misma invocacion:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Adjunta el resumen JSON y el registro de verificación al ticket de cambio de DNS/gateway para que
auditores puedan confirmar los hosts canónicos, wildcard y Pretty sin reejecutar los scripts.
Vuelva a ejecutar el comando cuando se agreguen nuevos alias al paquete para que las actualizaciones GAR
hereden la misma evidencia.

## Paso 7 - Genera el descriptor de transferencia DNS

Los recortes de producción requieren un paquete de cambio auditable. Después de un envío exitoso
(binding de alias), el ayudante emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:- metadatos de vinculación de alias (espacio de nombres/nombre/prueba, resumen del manifiesto, URL Torii,
  época enviada, autoridad);
- contexto de lanzamiento (etiqueta, alias etiqueta, rutas de manifiesto/CAR, plan de trozos, paquete Sigstore);
- punteros de verificación (comando sonda, alias + endpoint Torii); y
- campos opcionales de control de cambios (id de ticket, ventana de cutover, contacto ops,
  nombre de host/zona de producción);
- metadatos de promoción de rutas derivadas del header `Sora-Route-Binding`
  (host canonico/CID, rutas de header + vinculante, comandos de verificación), asegurando que la promoción
  GAR y los ejercicios de respaldo referencian la misma evidencia;
- los artefactos de route-plan generados (`gateway.route_plan.json`,
  templates de headers y headers de rollback opcionales) para que los tickets de cambio y ganchos de
  lint de CI puedan verificar que cada paquete DG-3 referencia los planos de promoción/rollback
  canónicos antes de la aprobación;
- metadatos opcionales de invalidación de caché (punto final de purga, variable de autenticación, carga útil JSON,
  y ejemplo de comando `curl`); y
- sugerencias de rollback apuntando al descriptor anterior (etiqueta de lanzamiento y resumen del manifiesto)
  para que los tickets capturen una ruta determinista alternativa.

Cuando el lanzamiento requiere purgas de caché, genera un plan canónico junto al descriptor de cutover:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Adjunta el `portal.cache_plan.json` resultante al paquete DG-3 para que los operadores
tengan hosts/paths deterministas (y los tips de auth que coinciden) al emitir solicitudes `PURGE`.
La sección opcional de caché del descriptor puede hacer referencia a este archivo directamente,
mantener alineados a los revisores de control de cambios sobre exactamente que endpoints se limpian
durante una transición.

Cada paquete DG-3 también necesita un checklist de promoción + rollback. Generala vía
`cargo xtask soradns-route-plan` para que los revisores de control de cambios puedan seguir los pasos
exactos de verificación previa, corte y reversión por alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

El `gateway.route_plan.json` emitido captura hosts canonicos/pretty, recordatorios de chequeo de salud
por etapas, actualizaciones de vinculante GAR, purgas de caché y acciones de reversión. Incluyelo con
los artefactos GAR/binding/cutover antes de enviar el ticket de cambio para que Ops pueda ensayar y
aprobar los mismos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa este descriptor y se ejecuta automáticamente desde
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

Rellene los metadatos opcionales mediante variables de entorno antes de ejecutar el ayudante de pin:| Variables | propuesta |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID de billete almacenado en el descriptor. |
| `DNS_CUTOVER_WINDOW` | Ventana de transición ISO8601 (por ejemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nombre de host de producción + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias ​​de guardia o contacto de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Punto final de purga de caché registrado en el descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contiene el token de purga (predeterminado: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Ruta al descriptor de cutover anterior para metadatos de rollback. |

Adjunta el JSON al review de cambio DNS para que los aprobadores puedan verificar resúmenes de manifiesto,
enlaces de alias y comandos sondeo sin tener que revisar registros de CI. Las banderas de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, y `--previous-dns-plan` proveen los mismos overrides
cuando se ejecuta el ayudante fuera de CI.

## Paso 8 - Emite el esqueleto de zona del solucionador (opcional)Cuando la ventana de corte de producción es conocida, el guión de lanzamiento puede emitir el
esqueleto de Zonefile SNS y el fragmento de resolución automáticamente. Pasa los registros DNS deseados
y metadatos vía variables de entorno o opciones CLI; el ayudante llamara a
`scripts/sns_zonefile_skeleton.py` inmediatamente después de generar el descriptor de corte.
Provee al menos un valor A/AAAA/CNAME y el digest GAR (BLAKE3-256 del payload GAR firmado). Si la
zona/hostname son conocidos y `--dns-zonefile-out` se omite, el helper escribe en
`artifacts/sns/zonefiles/<zone>/<hostname>.json` y llena
`ops/soradns/static_zones.<hostname>.json` como el fragmento de resolución.| Variable/bandera | propuesta |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Ruta para el esqueleto de Zonefile generado. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Ruta del fragmento de resolución (predeterminado: `ops/soradns/static_zones.<hostname>.json` cuando se omite). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL aplicado a los registros generados (predeterminado: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direcciones IPv4 (env separado por comas o flag CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direcciones IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Objetivo CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pinos SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionales (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Anular la etiqueta de versión del archivo de zona calculado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Fuerza el timestamp `effective_at` (RFC3339) en lugar del inicio de la ventana de cutover. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Anular la prueba literal registrada en los metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Anular el CID registrado en los metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelación de guardián (suave, duro, descongelación, seguimiento, emergencia). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia de ticket de guardian/council para congelaciones. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Marca de tiempo RFC3339 para descongelación. || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de congelación adicionales (env separado por comas o flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Resumen BLAKE3-256 (hex) de la carga útil GAR firmado. Requerido cuando hay fijaciones de gateway. |

El flujo de trabajo de GitHub Actions lee estos valores desde secrets del repositorio para que cada pin de
La produccion emite los artefactos de Zonefile automaticamente. Configura los siguientes secretos
(las cadenas pueden contener listas separadas por comas para campos multivalor):

| Secreto | propuesta |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nombre de host/zona de producción pasados ​​al ayudante. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de guardia almacenado en el descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registros IPv4/IPv6 a publicar. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Objetivo CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pinos SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionales. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadatos de congelación registrados en el esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 en hexadecimal de la carga útil GAR firmado. |

Al disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proporciona las entradas
`dns_change_ticket` y `dns_cutover_window` para que el descriptor/zonefile herede la ventana
correcto. Dejarlos en blanco solo cuando ejecutes dry run.

Invocacion tipica (coincide con el runbook del propietario SN-7):

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
```El ayudante automáticamente arrastra el ticket de cambio como entrada TXT y hereda el inicio de la
ventana de cutover como timestamp `effective_at` a menos que se haga override. Para el flujo
operativo completo, ver `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegación DNS pública

El esqueleto de Zonefile solo define registros autorizados de la zona. tía
debes configurar la delegación NS/DS de la zona padre en tu registrador o
Proveedor DNS para que el Internet público pueda encontrar los servidores de nombres.

- Para cutovers en el apex/TLD, usa ALIAS/ANAME (dependiente del proveedor) o
  publica registros A/AAAA que apuntan a las IP anycast del gateway.
- Para subdominios, publica un CNAME hacia el Pretty Host derivado
  (`<fqdn>.gw.sora.name`).
- El host canonico (`<hash>.gw.sora.id`) permanece bajo el dominio del gateway
  y no se publica dentro de tu zona pública.

### Plantilla de encabezados del gateway

El ayudante de implementación también emite `portal.gateway.headers.txt` y
`portal.gateway.binding.json`, dos artefactos que satisfacen el requisito DG-3 de
enlace de contenido de puerta de enlace:- `portal.gateway.headers.txt` contiene el bloque completo de encabezados HTTP (incluido
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS y el descriptor
  `Sora-Route-Binding`) que los gateways de borde deben grabar en cada respuesta.
- `portal.gateway.binding.json` registra la misma información en forma legible por máquinas
  para que los tickets de cambio y la automatización puedan comparar enlaces host/cid sin
  raspar la salida de cáscara.

Se genera automáticamente mediante
`cargo xtask soradns-binding-template`
y capturan el alias, el digest del manifiesto y el hostname de gateway que se
pasó a `sorafs-pin-release.sh`. Para regenerar o personalizar el bloque de encabezados,
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

Pasa `--csp-template`, `--permissions-template`, o `--hsts-template` para anular
de los templates de headers por default cuando un despliegue necesita directivas adicionales;
combinelos con los switchs `--no-*` existentes para eliminar un header por completo.

Adjunta el snippet de headers al request de cambio de CDN y alimenta el documento JSON
al pipeline de automatizacion de gateway para que la promoción real de host coincida con la
evidencia de liberación.

El script de liberación ejecuta automáticamente el asistente de verificación para que los
Los billetes DG-3 siempre incluyen evidencia reciente. Vuelve a ejecutarlo manualmente
cuando editas el JSON vinculante a mano:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```El comando decodifica la carga útil `Sora-Proof` grabada, asegura que el metadato `Sora-Route-Binding`
coincide con el CID del manifiesto + hostname, y falla rapido si algún header se desvia.
Archiva la salida de consola junto a los otros artefactos de despliegue siempre que ejecutas.
el comando fuera de CI para que los revisores DG-3 tengan prueba de que el enlace fue validado
antes del corte.

> **Integración del descriptor DNS:** `portal.dns-cutover.json` ahora incrusta una sección
> `gateway_binding` apuntando a estos artefactos (rutas, content CID, estado de prueba y el
> template literal de headers) **y** una estrofa `route_plan` referenciando
> `gateway.route_plan.json` mas las plantillas de encabezados principales y rollback. Incluye esos
> bloques en cada billete de cambio DG-3 para que los revisores puedan comparar los valores
> exactos de `Sora-Name/Sora-Proof/CSP` y confirmar que los planos de promoción/rollback
> coincide con el paquete de evidencia sin abrir el archivo de construcción.

## Paso 9 - Ejecuta monitores de publicacion

El item del roadmap **DOCS-3c** requiere evidencia continua de que el portal, el proxy Try it y los
Las fijaciones del gateway se mantienen saludables después de un lanzamiento. Ejecuta el monitor consolidado
inmediatamente despues de los Pasos 7-8 y conectalo a tus sondas programadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` carga el archivo de configuración (ver
  `docs/portal/docs/devportal/publishing-monitoring.md` para el esquema) y
  ejecuta tres comprobaciones: sondas de rutas del portal + validación de CSP/Permisos-Política,
  probes del proxy Try it (opcionalmente golpeando su endpoint `/metrics`), y el verificador
  de vinculante de gateway (`cargo xtask soradns-verify-binding`) que ahora exige la presencia y
  el valor esperado de Sora-Content-CID junto con las verificaciones de alias/manifiesto.
- El comando termina con distinto de cero cuando falla algun probe para que CI, cron jobs u operadores
  de runbook podrán detener un lanzamiento antes de promocionar alias.
- Pasar `--json-out` escribe un resumen JSON único con estado por target; `--evidence-dir`
  emite `summary.json`, `portal.json`, `tryit.json`, `binding.json` y `checksums.sha256` para que
  los revisores de gobernanza puedan comparar resultados sin reejecutar los monitores. Archivo
  este directorio bajo `artifacts/sorafs/<tag>/monitoring/` junto al paquete Sigstore y el
  descriptor de transferencia DNS.
- Incluye la salida del monitor, la exportación de Grafana (`dashboards/grafana/docs_portal.json`),
  y el ID del taladro de Alertmanager en el ticket de liberación para que el SLO DOCS-3c pueda ser
  auditado luego. El playbook dedicado de monitor de editorial vive en
  `docs/portal/docs/devportal/publishing-monitoring.md`.Las sondas del portal requieren HTTPS y rechazan URL base `http://` a menos que `allowInsecureHttp`
este configurado en la configuración del monitor; manten objetivos de produccion/staging en TLS y solo
habilita la anulación para vistas previas locales.

Automatiza el monitor vía `npm run monitor:publishing` en Buildkite/cron una vez que el portal
este en vivo. El mismo comando, apuntado a URLs de produccion, alimenta los cheques de salud
Continúa que SRE/Docs usan entre lanzamientos.

## Automatización con `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula los Pasos 2-6. Este:

1. archiva `build/` en un tarball determinista,
2. ejecuta `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   y `proof verify`,
3. opcionalmente ejecuta `manifest submit` (incluyendo alias vinculante) cuando hay credenciales
   Torii, y
4. escribe `artifacts/sorafs/portal.pin.report.json`, el opcional
  `portal.pin.proposal.json`, el descriptor de cutover DNS (después de envíos),
  y el paquete de vinculación de puerta de enlace (`portal.gateway.binding.json` más el bloque de encabezados)
  para que los equipos de gobernanza, networking y ops puedan comparar el paquete de evidencia
  Sin revisar los registros de CI.

Configura `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, y (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar el script. Usa `--skip-submit` para secar
corre; El flujo de trabajo de GitHub se describe a continuación alternativamente a través de la entrada `perform_submit`.## Paso 8 - Publica especificaciones OpenAPI y paquetes SBOM

DOCS-7 requiere que el build del portal, la especificación OpenAPI y los artefactos SBOM viajen
por el mismo oleoducto determinista. Los ayudantes existentes cubren los tres:

1. **Regenera y firma la especificación.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Pasa una etiqueta de liberación vía `--version=<label>` cuando quieras preservar una instantánea
   histórico (por ejemplo `2025-q3`). El ayudante escribe la instantánea en
   `static/openapi/versions/<label>/torii.json`, lo espeja en
   `versions/current`, y registra los metadatos (SHA-256, estado del manifiesto, y
   marca de tiempo actualizada) en `static/openapi/versions.json`. El portal de desarrolladores
   Lee ese índice para que los paneles Swagger/RapiDoc puedan presentar un selector de versión.
   y mostrar el digest/firma asociado en linea. Omitir `--version` mantiene las etiquetas del
   release anterior intactas y solo refresca los punteros `current` + `latest`.

   El manifiesto captura digiere SHA-256/BLAKE3 para que el gateway pueda grabar
   encabezados `Sora-Proof` para `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** El pipeline de lanzamiento ya espera SBOMs basados en syft
   según `docs/source/sorafs_release_pipeline_plan.md`. Manten la salida
   junto a los artefactos de construcción:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueta cada carga útil en un CAR.**

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
   ```Sigue los mismos pasos de `manifest build` / `manifest sign` que el sitio principal,
   ajustando alias por artefacto (por ejemplo, `docs-openapi.sora` para la especificación y
   `docs-sbom.sora` para el paquete SBOM firmado). Mantener alias distintos mantiene SoraDNS,
   GAR y tickets de rollback acotados al payload exacto.

4. **Submit y bind.** Reutiliza la autoridad existente y el paquete Sigstore, pero registra
   la tupla de alias en el checklist de liberación para que los auditores rastreen que nombre Sora
   mapea a que digerir de manifiesto.

Archivar los manifiestos de spec/SBOM junto al build del portal asegura que cada ticket de
Release contiene el conjunto completo de artefactos sin reejecutar el packer.

### Ayudante de automatización (CI/paquete script)

`./ci/package_docs_portal_sorafs.sh` codifica los Pasos 1-8 para que el item del roadmap
**DOCS-7** pueda ejercitarse con un solo comando. El ayudante:- ejecuta la preparación requerida del portal (`npm ci`, sync OpenAPI/norito, pruebas de widgets);
- emite los CARs y pares de manifiesto del portal, OpenAPI y SBOM vía `sorafs_cli`;
- opcionalmente ejecuta `sorafs_cli proof verify` (`--proof`) y la firma Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deja todos los artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` y
  escribe `package_summary.json` para que CI/tooling de release pueda ingerir el paquete; y
- refresca `artifacts/devportal/sorafs/latest` para apuntar a la ejecucion mas reciente.

Ejemplo (pipeline completo con Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Marcas a conocer:- `--out <dir>` - anula la raíz de artefactos (por defecto mantiene carpetas con marca de tiempo).
- `--skip-build` - reutiliza un `docs/portal/build` existente (útil cuando CI no puede
  reconstruir por espejos sin conexión).
- `--skip-sync-openapi` - omitir `npm run sync-openapi` cuando `cargo xtask openapi`
  no puede llegar a crates.io.
- `--skip-sbom` - evita llamar a `syft` cuando el binario no está instalado (el script imprime
  una advertencia en su lugar).
- `--proof` - ejecuta `sorafs_cli proof verify` para cada par CAR/manifiesto. Las cargas útiles de
  Múltiples archivos aún requieren soporte de chunk-plan en el CLI, así que deja este flag
  sin establecer si encuentras errores de `plan chunk count` y verifica manualmente cuando
  llegue el gate aguas arriba.
- `--sign` - invoca `sorafs_cli manifest sign`. Probar una estafa simbólica
  `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) o deja que el CLI lo obtenga usando
  `--sigstore-provider/--sigstore-audience`.Cuando envidias artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Ahora empaqueta el portal, OpenAPI y payloads SBOM, firma cada manifiesto, y registra metadatos
extra de activos en `portal.additional_assets.json`. El ayudante entiende los mismos mandos opcionales.
usados por el empaquetador de CI mas los nuevos switchs `--openapi-*`, `--portal-sbom-*`, y
`--openapi-sbom-*` para asignar tuplas de alias por artefacto, anular la fuente SBOM vía
`--openapi-sbom-source`, omitir ciertas cargas útiles (`--skip-openapi`/`--skip-sbom`),
y apuntar a un binario `syft` no default con `--syft-bin`.

El script expone cada comando que ejecuta; copia el registro en el boleto de liberación
junto a `package_summary.json` para que los revisores puedan comparar resúmenes de CAR, metadatos de
plan, y hashes del paquete Sigstore sin revisar salida de shell ad hoc.

## Paso 9 - Verificación de puerta de enlace + SoraDNS

Antes de anunciar un cutover, prueba que el alias nuevo se resuelve vía SoraDNS y que los gateways
engrapan pruebas frescas:

1. **Ejecuta el gate de probe.** `ci/check_sorafs_gateway_probe.sh` ejercita
   `cargo xtask sorafs-gateway-probe` contra los accesorios demo en
   `fixtures/sorafs_gateway/probe_demo/`. Para despliegues reales, apunta el sondeo al nombre de host objetivo:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   La sonda decodifica `Sora-Name`, `Sora-Proof`, y `Sora-Proof-Status` a continuación
   `docs/source/sorafs_alias_policy.md` y falla cuando el digest del manifiesto,
   los TTL o los enlaces GAR se desvian.Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.

2. **Captura evidencia de taladro.** Para ejercicios de operador o simulacros de PagerDuty, envuelve
   el probe con `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   lanzamiento del portal de desarrollo -- ...`. El wrapper guarda headers/logs bajo
   `artifacts/sorafs_gateway_probe/<stamp>/`, actualiza `ops/drill-log.md`, y
   (opcionalmente) dispara ganchos de reversión o cargas útiles PagerDuty. Establece
   `--host docs.sora` para validar la ruta SoraDNS en lugar de codificar una IP.3. **Verifica vinculaciones DNS.** Cuando el gobierno publica la prueba del alias, registra el archivo GAR
   referenciado por el probe (`--gar`) y adjuntalo a la evidencia de liberación. Los propietarios de resolver
   pueden reflejar el mismo input via `tools/soradns-resolver` para asegurar que las entradas en cache
   respeten el manifiesto nuevo. Antes de adjuntar el JSON, ejecuta
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   para que el mapeo determinista de host, los metadatos del manifiesto y las etiquetas de telemetría se
   validar sin conexión. El ayudante puede emitir un resumen `--json-out` junto al GAR firmado para que los
   Los revisores tengan evidencia verificable sin abrir el binario.
  Cuando redacta un GAR nuevo, prefiere
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (vuelve a `--manifest-cid <cid>` solo cuando el archivo de manifiesto no este disponible). El ayudante
  ahora deriva el CID **y** el digest BLAKE3 directamente del manifest JSON, recorta espacios en blanco,
  deduplica flags `--telemetry-label` repite, ordena las etiquetas y emite las plantillas por defecto
  de CSP/HSTS/Permissions-Policy antes de escribir el JSON para que la carga útil se mantenga determinista
  incluso cuando los operadores capturan etiquetas desde shells distintos.

4. **Observa métricas de alias.** Manten `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` en pantalla mientras la sonda
   corre; ambas series están graficadas en `dashboards/grafana/docs_portal.json`.## Paso 10 - Monitoreo y paquete de evidencia

- **Dashboards.** Exporta `dashboards/grafana/docs_portal.json` (SLOs del portal),
  `dashboards/grafana/sorafs_gateway_observability.json` (latencia del gateway +
  salud de prueba), y `dashboards/grafana/sorafs_fetch_observability.json`
  (salud del orquestador) para cada liberación. Adjunta los exports JSON al ticket de
  Release para que los revisores puedan reproducir las consultas de Prometheus.
- **Archivos de sonda.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex
  o tu cubo de evidencia. Incluye el resumen de la sonda, los encabezados y la carga útil de PagerDuty.
  capturado por el script de telemetría.
- **Bundle de release.** Guarda los resúmenes de CAR del portal/SBOM/OpenAPI, bundles de manifiesto,
  firmas Sigstore, `portal.pin.report.json`, registros de sonda Try-It y informes de link-check
  bajo una carpeta con marca de tiempo (por ejemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Cuando los sondas son parte de un taladro, deja que
  `scripts/telemetry/run_sorafs_gateway_probe.sh` agregar entradas a `ops/drill-log.md`
  para que la misma evidencia satisfaga el requisito de caos SNNet-5.
- **Links de ticket.** Referencia los IDs de panel de Grafana o exports PNG adjuntos en el ticket
  de cambio, junto con la ruta del reporte de probe, para que los revisores puedan cruzar los SLOs
  sin acceso a concha.

## Paso 11 - Drill de fetch multi-source y evidencia de marcadorPublicar en SoraFS ahora requiere evidencia de fetch multi-source (DOCS-7/SF-6)
junto a las pruebas de DNS/gateway anteriores. Después de fijar el manifiesto:

1. **Ejecuta `sorafs_fetch` contra el manifiesto live.** Usa los mismos artefactos de plan/manifiesto
   producidos en los Pasos 2-3 más las credenciales de gateway emitidas para cada proveedor. persistir
   toda la salida para que los auditores puedan reproducir el rastro de decisión del orquestador:

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

   - Busca primero los anuncios de proveedores referenciados por el manifiesto (por ejemplo
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     y pasalos vía `--provider-advert name=path` para que el marcador pueda evaluar ventanas
     de capacidad de forma determinista. Estados Unidos
     `--allow-implicit-provider-metadata` **solo** cuando reproduce aparatos en CI; los ejercicios
     de produccion deben citar los anuncios firmados que llegaron con el pin.
   - Cuando el manifiesto hace referencia a regiones adicionales, repite el comando con los tuplas de
     proveedores correspondientes para que cada caché/alias tenga un artefacto de fetch asociado.

2. **Archiva las salidas.** Guarda `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, y `chunk_receipts.ndjson` bajo la carpeta de evidencia del
   liberación. Estos archivos capturan la ponderación de pares, el presupuesto de reintento, la latencia EWMA y los
   recibos por trozos que el paquete de gobernanza debe retener para SF-7.3. **Actualiza telemetría.** Importa las salidas de fetch en el tablero **SoraFS Fetch
   Observabilidad** (`dashboards/grafana/sorafs_fetch_observability.json`),
   observando `torii_sorafs_fetch_duration_ms`/`_failures_total` y los paneles de rango por proveedor
   para anomalías. Enlaza los snapshots de panel Grafana al ticket de release junto con la ruta del
   marcador.

4. **Smokea las reglas de alerta.** Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   para validar el paquete de alertas Prometheus antes de cerrar el lanzamiento. Adjunta la salida de
   promtool al ticket para que los revisores DOCS-7 confirmen que las alertas de parada y
   proveedor lento siguen armadas.

5. **Cable en CI.** El flujo de trabajo de pin del portal mantiene un paso `sorafs_fetch` detrás del input
   `perform_fetch_probe`; habilitalo para ejecuciones de staging/produccion para que la evidencia de
   fetch se produce junto al paquete de manifiesto sin intervención manual. Los simulacros locales pueden
   reutilizar el mismo script exportando los tokens de gateway y estableciendo `PIN_FETCH_PROVIDERS`
   a la lista de proveedores separados por comas.

## Promoción, observabilidad y rollback1. **Promoción:** manten alias separados para puesta en escena y producción. Promociona reejecutando
   `manifest submit` con el mismo manifiesto/bundle, cambiando
   `--alias-namespace/--alias-name` para apuntar al alias de producción. Esto evita
   reconstruir o refirmar una vez que QA aprueba el pin de staging.
2. **Monitoreo:** importa el panel de registro de pin
   (`docs/source/grafana_sorafs_pin_registry.json`) mas las sondas específicas del portal
   (ver `docs/portal/docs/devportal/observability.md`). Alerta en deriva de sumas de control,
   probes fallidos o picos de reintento de prueba.
3. **Rollback:** para revertir, reenvia el manifiesto anterior (o retira el alias actual) usando
   `sorafs_cli manifest submit --alias ... --retire`. Manten siempre el ultimo paquete
   conocido como bueno y el resumen CAR para que las pruebas de rollback puedan recrearse si
   los logs de CI se rotan.

## Plantilla de CI de flujo de trabajo

Como mínimo, tu tubería debe:

1. Build + lint (`npm ci`, `npm run build`, generacion de checksums).
2. Empaquetar (`car pack`) y computar manifiestos.
3. Firmar usando el token OIDC del trabajo (`manifest sign`).
4. Subir artefactos (CAR, manifiesto, paquete, plano, resúmenes) para auditoria.
5. Enviar al pin de registro:
   - Solicitudes de extracción -> `docs-preview.sora`.
   - Tags / sucursales protegidas -> promoción de alias de producción.
6. Ejecutar sondas + puertas de verificación de prueba antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todos estos pasos para versiones manuales.
El flujo de trabajo:- construye/testea el portal,
- empaqueta el build vía `scripts/sorafs-pin-release.sh`,
- firma/verifica el paquete de manifiesto usando GitHub OIDC,
- sube el CAR/manifiesto/bundle/plan/resumenes como artefactos, y
- (opcionalmente) envia el manifiesto + alias vinculante cuando hay secretos.

Configure los siguientes secretos/variables del repositorio antes de disparar el trabajo:

| Nombre | propuesta |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que exponen `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrado con presentaciones. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de firma para el sometimiento del manifiesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias enlazado al manifiesto cuando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Paquete de prueba de alias codificado en base64 (opcional; omita para saltar el enlace de alias). |
| `DOCS_ANALYTICS_*` | Puntos finales de análisis/sonda existentes reutilizados por otros flujos de trabajo. |

Dispara el flujo de trabajo a través de la UI de Acciones:

1. Provee `alias_label` (por ejemplo, `docs.sora.link`), `proposal_alias` opcional,
   y una anulación opcional de `release_tag`.
2. Deja `perform_submit` sin marcar para generar artefactos sin tocar Torii
   (Util para simulacros) o habilitarlo para publicar directamente al alias configurado.`docs/source/sorafs_ci_templates.md` aun documenta los helpers de CI genericos para
proyectos fuera de este repositorio, pero el flujo de trabajo del portal debe ser la opción preferida
para lanzamientos del dia a dia.

## Lista de verificación

- [ ] `npm run build`, `npm run test:*`, y `npm run check:links` están en verde.
- [ ] `build/checksums.sha256` y `build/release.json` capturados en artefactos.
- [ ] CAR, plan, manifiesto, y resumen generados bajo `artifacts/`.
- [ ] Bundle Sigstore + firma separadas con logs.
- [ ] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
      capturados cuando hay presentaciones.
- [ ] `portal.pin.report.json` (y opcional `portal.pin.proposal.json`)
      archivado junto a los artefactos CAR/manifiesto.
- [ ] Registros de `proof verify` y `manifest verify-signature` archivados.
- [ ] Dashboards de Grafana actualizados + sondas Try-It exitosas.
- [ ] Notas de rollback (ID de manifiesto anterior + resumen de alias) adjuntas al
      billete de liberación.