---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Visao general

Este libro de jugadas convierte los elementos de la hoja de ruta **DOCS-7** (publicada por SoraFS) y **DOCS-8**
(automático de pin de CI/CD) en un procedimiento activo para el portal de desarrolladores.
Cobre a fase de build/lint, o empacotamento SoraFS, a assinatura de manifestos com Sigstore,
promoción de alias, verificación y ejercicios de reversión para cada vista previa y lanzamiento
seja reproduzivel e auditavel.

El flujo asume que voce tem o binario `sorafs_cli` (construido con `--features cli`), acceso a
Un punto final Torii con permisos de registro PIN y credenciales OIDC para Sigstore. guardia secreta
de larga duración (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens de Torii) en su bóveda de CI; como
execucoes locais podem carrega-los a partir de exportaciones do shell.

##Requisitos previos- Nodo 18.18+ con `npm` o `pnpm`.
- `sorafs_cli` desde `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii que expone `/v2/sorafs/*` mais uma conta/chave privada de autoridade que possa enviar manifiestos y alias.
- Emisor OIDC (GitHub Actions, GitLab, identidad de carga de trabajo, etc.) para emitir un `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para ejecuciones dre e `docs/source/sorafs_ci_templates.md` para scaffolding de flujos de trabajo de GitHub/GitLab.
- Configurar como variables OAuth de Tre it (`DOCS_OAUTH_*`) y ejecutar un
 [lista de verificación de refuerzo de seguridad](./security-hardening.md) antes de promover una compilación
 fuera del laboratorio. O build del portal ahora falla cuando estas variables faltan
 o cuando los botones de TTL/polling quedan fuera de las ventanas aplicadas; exporta
 `DOCS_OAUTH_ALLOW_INSECURE=1` sólo para vistas previas locales desechables. anexo a
 evidencia de pen-test al ticket de liberación.

## Etapa 0 - Captura un paquete del proxe Tre it

Antes de promover una vista previa de Netlife o al gateway, seleccione como fuentes del proxe Tre it e o
resumen del manifiesto OpenAPI firmado en un paquete determinista:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copia los helpers de proxy/probe/rollback, verifica la configuración
OpenAPI e escribe `release.json` más `checksums.sha256`. Anexe este paquete al ticket de
promoción de Netlify/SoraFS gatewae para que los revisores puedan reproducir según fuentes exactas
del proxe e as pistas del target Torii sem reconstruir. O paquete tambem registra si os
portadores provistos por o cliente estaban habilitados (`allow_client_auth`) para manter o plan
de rollout e as reglas CSP em sincronización.

## Etapa 1 - Construir y pelusa del portal

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
- `build/release.json` - metadatos (`tag`, `generated_at`, `source`) fijados en cada CAR/manifesto.

Arquive ambos arquivos junto al resumo CAR para que os revisores possam compare artefactos de
vista previa sin reconstruir.

## Etapa 2 - Empaqueta de activos estaticos

Ejecute el empacotamador CAR contra el directorio de salida de Docusaurus. El ejemplo de abajo
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

El resumen de captura JSON de fragmentos, resúmenes y pistas de planeación de prueba que
`manifest build` y los paneles de CI reutilizan depósitos.

## Etapa 2b - Empaqueta acompañantes OpenAPI e SBOMDOCS-7 requiere publicar el sitio del portal, la instantánea OpenAPI y las cargas útiles SBOM
como manifiestos distintos para que os gateways possam engrapar os headers
`Sora-Proof`/`Sora-Content-CID` para cada artefacto. Oh ayudante de liberación
(`scripts/sorafs-pin-release.sh`) ya empaqueta o directorio OpenAPI
(`static/openapi/`) y los SBOM emitidos vía `syft` en CARs separados
`openapi.*`/`*-sbom.*` y registra los metadatos en
`artifacts/sorafs/portal.additional_assets.json`. Al ejecutar el manual fluxo,
repite las etapas 2-4 para cada carga útil con sus propios prefijos e etiquetas de metadatos
(por ejemplo `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Registra cada par manifiesto/alias
en Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) antes de cambiar DNS para que
o gatewae possa servir provas engrapadas para todos los artefactos publicados.

## Etapa 3 - Construye o manifiesto

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

Ajusta las banderas políticas de pin según tu ventana de liberación (por ejemplo, `--pin-storage-class
hot` para canarios). Una variante JSON es opcional pero conveniente para la revisión del código.

## Etapa 4 - Assinatura con Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```El paquete registra el resumen del manifiesto, los resúmenes de fragmentos y el hash BLAKE3 del token.
OIDC sin persistir en JWT. Guarda tanto el paquete como una assinatura separada; aspromociones de
produccion podem reutilizar os mesmos artefactos em lugar de re-assinar. Como execucoes locais
podem reemplazar os flags del proveedor com `--identity-token-env` (o establecer
`SIGSTORE_ID_TOKEN` en el entorno) cuando un asistente OIDC externo emite un token.

##Etapa 5 - Envie al pin registro

Envie el manifiesto firmado (y el plan de trozos) en Torii. Solicita siempre un currículum para que
a entrada/alias resultante seja auditavel.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite i105... \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

Cuando se despliega un alias de previa o canare (`docs-preview.sora`), repite o
envio com um alias unico para que QA possa verificar o contenido antes de una promoción a
producción.

La vinculación de alias requiere tres campos: `--alias-namespace`, `--alias-name` e `--alias-proof`.
Governance produce el paquete de prueba (base64 o bytes Norito) cuando se aprueba una solicitud
del alias; guárdelo en secretos de CI e expónlo como archivo antes de invocar `manifest submit`.
Deja las banderas de alias sin establecer cuando apenas piensas fijar el manifiesto sin tocar DNS.

## Etapa 5b - Genera una propuesta de gobernanzaCada manifiesto debe viajar con una lista de propuestas para el Parlamento para que cualquier ciudadano
Sora possa introducir o cambio sem pedir credenciales privilegiadas. Después de los pasos
enviar/firmar, ejecutar:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura una instrucción canónica `RegisterPinManifest`,
o digest de chunks, a politica y a pista de alias. Adjuntalo al ticket de gobernancia o al
portal Parlamento para que los delegados puedan comparar la carga útil sin reconstruir los artefactos.
Como o comando nunca toca a clave de autoridad de Torii, cualquier ciudadano puede redactar a
propuesta localmente.

## Etapa 6 - Verifique pruebas y telemetría

Después de fijar, ejecute las etapas de verificación determinista:

```bash
sorafs_cli proof verife \
 --manifest "$OUT"/portal.manifest.to \
 --car "$OUT"/portal.car \
 --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
 --manifest "$OUT"/portal.manifest.to \
 --bundle "$OUT"/portal.manifest.bundle.json \
 --chunk-plan "$OUT"/portal.plan.json
```- Verifique `torii_sorafs_gateway_refusals_total` e
 `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalías.
- Ejecute `npm run probe:portal` para ejercitar el proxy Try-It y los enlaces registrados.
 contra o contenido recién fijado.
- Captura a evidencia de monitoreo descrito em
 [Publishing & Monitoring](./publishing-monitoring.md) para que a puerta de observabilidad DOCS-3c
 se satisfaga junto a os etapas de publicacao. O helper ahora acepta múltiples entradas
 `bindings` (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) y aplicaciones `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 em o host objetivo vía o guard opcional `hostname`. A invocacao de abajo escribe tanto um
 Resumen JSON único como paquete de evidencia (`portal.json`, `tryit.json`, `binding.json` y
 `checksums.sha256`) bajo el directorio de lanzamiento:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a - Planifica certificados del gateway

Deriva el plan de SAN/challenge TLS antes de crear paquetes GAR para que o equipar de gateway y os
aprobadores de DNS revisan a mesma evidencia. El nuevo ayudante refleja los conocimientos de automatización.
DG-3 enumera hosts canónicos comodín, SAN de host bonito, etiquetas DNS-01 y desafíos recomendados por ACME:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```Commite el JSON junto al paquete de lanzamiento (o subelo con el ticket de cambio) para que los operadores
Puede pegar los valores SAN en la configuración `torii.sorafs_gateway.acme` de Torii y los revisores.
de GAR possam confirmar os mapeos canonico/prette sem re-ejecutar derivaciones de host. agrega
argumentos `--name` adicionales para cada sufijo promocionado em o mesmo release.

## Etapa 6b - Deriva mapeos de host canónicos

Antes de las cargas útiles templarias GAR, registra el mapeo de host determinista para cada alias.
`cargo xtask soradns-hosts` hashea cada `--name` en su etiqueta canónica
(`<base32>.gw.sora.id`), emite el comodín requerido (`*.gw.sora.id`) y deriva o prette host
(`<alias>.gw.sora.name`). Persiste a salida em os artefactos de liberación para que os revisores
DG-3 possam comparar o mapeo junto al envío GAR:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Usa `--verify-host-patterns <file>` para fallar rápidamente cuando un JSON de GAR o enlace de puerta de enlace
omita uno de los hosts requeridos. O helper acepta múltiples archivos de verificación, haciendo
fácil lint de tanto a plantilla GAR como o `portal.gateway.binding.json` grabado en una mesma invocacion:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Anexe el resumen JSON y el registro de verificación al ticket de cambio de DNS/gatewae para que
Los auditores pueden confirmar los hosts canónicos, comodines y bonitos sin volver a ejecutar los scripts.
Vuelva a ejecutar el comando cuando se agreguen nuevos alias al paquete para que as actualizaciones GAR
hereden a mesma evidencia.## Etapa 7 - Genera o descriptor de transferencia DNS

Los recortes de producción requieren un paquete de cambio auditado. Después de un envío exitoso
(binding de alias), o helper emite
`artifacts/sorafs/portal.dns-cutover.json`, capturando:- metadatos de vinculación de alias (espacio de nombres/nombre/prueba, resumen del manifiesto, URL Torii,
 época enviada, autoridad);
- contexto de lanzamiento (etiqueta, etiqueta de alias, rutas de manifiesto/CAR, plan de trozos, paquete Sigstore);
- punteros de verificacao (sonda de comando, alias + punto final Torii); mi
- campos opcionales de control de cambios (id de ticket, ventana de cutover, contacto ops,
 nombre de host/zona de producción);
- metadatos de promoción de rutas derivadas del header `Sora-Route-Binding`
 (host canonico/CID, rutas de header + vinculante, comandos de verificacao), asegurando que a promocao
 GAR e os drills de fallback referencian a mesma evidencia;
- os artefactos de route-plan generados (`gateway.route_plan.json`,
 plantillas de encabezados y encabezados de rollback opcionales) para que os tickets de cambio e ganchos de
 lint de CI possam verificar que cada paquete DG-3 referencia os planes de promoción/rollback
 canónicos antes de una aprobación;
- metadatos opcionales de invalidación de caché (punto final de purga, variable de autenticación, carga útil JSON,
 y ejemplo de comando `curl`); mi
- sugerencias de reversión apuntando al descriptor anterior (etiqueta de lanzamiento y resumen del manifiesto)
 para que los boletos capturados sean una ruta determinista alternativa.

Cuando la liberación requiere purgas de caché, genera un plan canónico junto al descriptor de cutover:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```Anexe o `portal.cache_plan.json` resultante al paquete DG-3 para que os operadores
tengan hosts/paths deterministas (y os sugerencias de autenticación que coinciden) al emitir solicitudes `PURGE`.
A secoo opcional de cache del descriptor pode referenciar este archivo directamente,
mantener alineados a los revisores de control de cambios sobre exactamente qué puntos finales se limpian
Durante una transición.

Cada paquete DG-3 también necesita una lista de verificación de promoción + reversión. Generala vía
`cargo xtask soradns-route-plan` para que los revisores de control de cambios puedan seguir los pasos
exactitudes de verificación previa, corte y reversión por alias:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

O `gateway.route_plan.json` emitida captura hosts canonicos/pretty, recordatorios de chequeo de salud
por etapas, actualizaciones de vinculación GAR, purgas de caché y acciones de reversión. Incluyelo com
os artefactos GAR/binding/cutover antes de enviar o ticket de cambio para que Ops possa ensayar e
Aprobar os mesmos etapas com guion.

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

Rellene los metadatos opcionales mediante variables de entorno antes de ejecutar o ayudante de pin:| Variables | propuesta |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID del billete almacenado en el descriptor. |
| `DNS_CUTOVER_WINDOW` | Ventana de transición ISO8601 (por ejemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nombre de host de producción + zona autoritativa. |
| `DNS_OPS_CONTACT` | Alias ​​de guardia o contacto de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Punto final de purga de caché registrado en el descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contiene el token de purga (predeterminado: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Ruta al descriptor de cutover anterior para metadatos de rollback. |

Anexe o JSON al review de cambio DNS para que los aprobadores puedan verificar los resúmenes del manifiesto,
Los enlaces de alias y los comandos sondean sin tener que revisar los registros de CI. Las banderas de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, e `--previous-dns-plan` proveen os mesmos anulaciones
cuando se ejecuta o ayudante fuera de CI.

## Etapa 8 - Emite o esqueleto de archivo de zona del solucionador (opcional)Quando a ventana de cutover de produccion es conocida, o script de release pode emitir o
esqueleto de archivo de zona SNS y fragmento de resolución automática. Pasar los registros DNS deseados
y metadatos a través de variables de entorno o opciones CLI; o ayudante llamara a
`scripts/sns_zonefile_skeleton.py` inmediatamente después de generar o descriptor de corte.
Provee al menos un valor A/AAAA/CNAME y o digest GAR (BLAKE3-256 del payload GAR firmado). si a
zona/hostname son conocidos e `--dns-zonefile-out` se omite, o helper escribe em
`artifacts/sns/zonefiles/<zone>/<hostname>.json` y lleno
`ops/soradns/static_zones.<hostname>.json` como fragmento de resolución.| Variable/bandera | propuesta |
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
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Fuerza o marca de tiempo `effective_at` (RFC3339) en lugar del inicio de una ventana de corte. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Anular la prueba literal registrada en los metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Anular el CID registrado en los metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelación de guardián (suave, duro, descongelación, seguimiento, emergencia). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia de ticket de guardian/council para congelaciones. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Marca de tiempo RFC3339 para descongelación. || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notas de congelación adicionales (env separado por comas o flag repetible). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Resumen BLAKE3-256 (hex) de la carga útil GAR firmado. Requerido quando hae binders de gateway. |

O flujo de trabajo de GitHub Actions lee estos valores a partir de secretos del repositorio para que cada pin de
La producción emite automáticamente los artefactos de Zonefile. Configura los siguientes secretos
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
| `DOCS_SORAFS_GAR_DIGEST` | Resumen BLAKE3 en hexadecimal de la carga útil GAR firmado. |

Al disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proporciona las entradas
`dns_change_ticket` e `dns_cutover_window` para que el descriptor/zonefile aparezca en la ventana
correcto. Dejarlos en blanco apenas cuando ejecutes dre run.

Invocación típica (coincide con el runbook del propietario SN-7):

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
```El ayudante automáticamente arrastra el ticket de cambio como entrada TXT e hereda o inicio de a
ventana de cutover como timestamp `effective_at` a menos que se haga override. Para o fluxo
operativo completo, ver `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegación pública de DNS

El esqueleto del archivo de zona define sólo registros autorizados de la zona. Ainda e
Es necesario configurar una delegación NS/DS de la zona del país no registrador o proveedor.
DNS para que Internet encuentre servidores de nombres.

- Para transferencias sin apex/TLD, use ALIAS/ANAME (dependiente del proveedor) o público
  registros A/AAAA apontando para las IPs anycast del gateway.
- Para subdominios, publique un CNAME para el derivado Pretty Host.
  (`<fqdn>.gw.sora.name`).
- O host canonico (`<hash>.gw.sora.id`) permanece sob o dominio do gateway e nao
  e publicado dentro de su zona pública.

### Plantilla de encabezados del gateway

El ayudante de implementación también emite `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, dos artefactos que satisfacen o requisito DG-3 de
enlace de contenido de puerta de enlace:- `portal.gateway.headers.txt` contiene el bloque completo de encabezados HTTP (incluido
 `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS y descriptor
 `Sora-Route-Binding`) que os gateways de borde deben grabar em cada respuesta.
- `portal.gateway.binding.json` registra a mesma información en forma legible por máquinas
 para que os tickets de cambio y un automacao puedan comparar enlaces host/cid sem
 raspar a salida de cáscara.

Se genera automáticamente mediante
`cargo xtask soradns-binding-template`
y capturaron el alias, el resumen del manifiesto y el nombre de host de gatewae que se
pasó a `sorafs-pin-release.sh`. Para regenerar o personalizar o bloque de encabezados,
ejecutar:

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
Las plantillas de encabezados predeterminadas cuando se implementan necesitan directivas adicionales;
Combinalos con los interruptores `--no-*` existentes para eliminar un encabezado por completo.

Anexo o fragmento de encabezados al solicitud de cambio de CDN y alimentación de documento JSON
al pipeline de automacao de gatewae para que a promocao real de host coincida con a
evidencia de liberación.

El script de liberación se ejecuta automáticamente o el asistente de verificación para el sistema operativo.
Los billetes DG-3 siempre incluyen evidencia reciente. Vuelve a ejecutarlo manualmente
Cuando editas o vinculas JSON a mano:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```El comando decodifica la carga útil `Sora-Proof` grabado, asegura que los metadatos `Sora-Route-Binding`
coincide con el CID del manifiesto + nombre de host, y falla rápidamente si algún encabezado se desvía.
Arquive a salida de consola junto a os otros artefactos de despliegue siempre que ejecuten
o comando fuera de CI para que os revisores DG-3 tengan prueba de que o vinculante fue validado
antes del corte.

> **Integración del descriptor DNS:** `portal.dns-cutover.json` ahora incrusta una sección
> `gateway_binding` apuntando a estos artefactos (rutas, content CID, estado de prueba e o
> template literal de headers) **e** uma estrofa `route_plan` referenciando
> `gateway.route_plan.json` más plantillas de encabezados principales y reversión. Incluye esos
> bloques en cada billete de cambio DG-3 para que los revisores puedan comparar los valores
> exactos de `Sora-Name/Sora-Proof/CSP` y confirmar que os planes de promocao/rollback
> coinciden con el paquete de evidencia sin abrir el archivo de construcción.

## Etapa 9 - Ejecutar monitores de publicacao

El elemento de la hoja de ruta **DOCS-3c** requiere evidencia continua de que el portal, el proxe Tre it y os
Las fijaciones del gatewae se mantienen saludables después de una liberación. Ejecutar o monitor consolidado
Inmediatamente después de os Etapas 7-8 e conectalo a tus sondas programadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` carga o archivo de configuración (ver
 `docs/portal/docs/devportal/publishing-monitoring.md` para el esquema) y
 ejecutar tres comprobaciones: sondas de rutas del portal + validación de CSP/Permisos-Política,
 sondas del proxe Tre it (opcionalmente golpeando su endpoint `/metrics`), y o verificador
 de vinculante de gatewae (`cargo xtask soradns-verify-binding`) que ahora exige una presencia e
 o valor esperado de Sora-Content-CID junto con verificaciones de alias/manifiesto.
- El comando termina con un valor distinto de cero cuando falla alguna sonda para que CI, cron jobs u operadores
 El runbook puede detener una liberación antes de promover un alias.
- Pasar `--json-out` escribe un resumen JSON único con el estado del objetivo; `--evidence-dir`
 emite `summary.json`, `portal.json`, `tryit.json`, `binding.json` e `checksums.sha256` para que
 Los revisores de gobernanza pueden comparar resultados sin volver a ejecutar los monitores. Archivar
 este directorio bajo `artifacts/sorafs/<tag>/monitoring/` junto al paquete Sigstore e o
 descriptor de transferencia DNS.
- Incluye una salida del monitor, o exportación de Grafana (`dashboards/grafana/docs_portal.json`),
 e o ID del simulacro de Alertmanager em o ticket de liberación para que o SLO DOCS-3c possa ser
 auditado luego. O playbook dedicado de monitor de publicación vive em
 `docs/portal/docs/devportal/publishing-monitoring.md`.Las sondas del portal requieren HTTPS y rechazan URL base `http://` a menos que `allowInsecureHttp`
este configurado en la configuración del monitor; mantenha objetivos de producción/puesta en escena en TLS e apenas
Habilita o anula las vistas previas locales.

Automatiza el monitor a través de `npm run monitor:publishing` en Buildkite/cron una vez que el portal
este en vivo. O mesmo comando, apuntado a URLs de produccion, alimenta os checks de salud
Continúa que SRE/Docs usan entre lanzamientos.

## Automacao con `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula las etapas 2-6. Este:

1. archivar `build/` en un tarball determinista,
2. ejecute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
 y `proof verify`,
3. opcionalmente ejecute `manifest submit` (incluyendo alias vinculante) cuando haya credenciais
 Torii, mi
4. escribe `artifacts/sorafs/portal.pin.report.json`, o opcional
 `portal.pin.proposal.json`, el descriptor de transferencia DNS (después de envíos),
 y el paquete de enlace de gateway (`portal.gateway.binding.json` más el bloque de encabezados)
 para que los equipos de gobernanza, redes y operaciones puedan comparar el paquete de evidencia
 Sem revisa los registros de CI.

Configura `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, y (opcionalmente)
`PIN_ALIAS_PROOF_PATH` antes de invocar el script. Usa `--skip-submit` para secar
corre; El flujo de trabajo de GitHub se describe a continuación alternativamente a través de la entrada `perform_submit`.

## Etapa 8 - Publica especificaciones OpenAPI y paquetes SBOMDOCS-7 requiere que la construcción del portal, la especificación OpenAPI y los artefactos SBOM viajen
por o mesmo oleoducto determinista. Os helpers existentes cubren os tres:

1. **Regenera e assinatura a especificación.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 Pasa una etiqueta de liberación vía `--version=<label>` cuando quieras preservar una instantánea
 histórico (por ejemplo `2025-q3`). O ayudante escribe o instantánea em
 `static/openapi/versions/<label>/torii.json`, lo espeja em
 `versions/current`, e registra os metadatos (SHA-256, estado del manifiesto, e
 marca de tiempo actualizada) en `static/openapi/versions.json`. El portal de desarrolladores
 Lee ese índice para que los paneles Swagger/RapiDoc puedan presentar un selector de versión.
 y mostrar o digest/assinatura asociado en línea. Omitir `--version` mantem as etiquetas del
 release anterior intactas e apenas refresca os punteros `current` + `latest`.

 La captura del manifiesto resume SHA-256/BLAKE3 para que la puerta de enlace pueda grabar
 encabezados `Sora-Proof` para `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** O pipeline de release ya espera SBOMs basados en syft
 según `docs/source/sorafs_release_pipeline_plan.md`. Mantenha a salida
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
 ```Sigue os mesmos etapas de `manifest build` / `manifest sign` que o sitio principal,
 ajustando alias por artefacto (por ejemplo, `docs-openapi.sora` para una especificación e
 `docs-sbom.sora` para el paquete SBOM firmado). Manter alias distintos mantem SoraDNS,
 GAR y tickets de rollback acotados al payload exacto.

4. **Submit e bind.** Reutiliza a autoridad existente y o paquete Sigstore, pero registra
 La tupla de alias en la lista de verificación de liberación para que los auditores rastreen el nombre de Sora.
 mapea a que digerir el manifiesto.

Arquivar os manifestos de spec/SBOM junto al build del portal asegura que cada ticket de
El lanzamiento contiene un conjunto completo de artefactos sin volver a ejecutar el empaquetador.

### Ayudante de automatización (CI/paquete script)

`./ci/package_docs_portal_sorafs.sh` codifica las etapas 1-8 para que o item del roadmap
**DOCS-7** puedes ejercitarte con un solo comando. Oh ayudante:

- ejecutar una preparación requerida del portal (`npm ci`, sync OpenAPI/norito, pruebas de widgets);
- emite os CARs y pares de manifiesto del portal, OpenAPI y SBOM vía `sorafs_cli`;
- opcionalmente ejecute `sorafs_cli proof verify` (`--proof`) y assinatura Sigstore
 (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deja todos los artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` e
 escribe `package_summary.json` para que CI/tooling de release possa ingerir o bundle; mi
- refresca `artifacts/devportal/sorafs/latest` para apuntar a a ejecucao más reciente.Ejemplo (canalización completa con Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

Marcas a conocer:

- `--out <dir>` - anulación de la raíz de artefactos (mantem carpetas com timestamp predeterminado).
- `--skip-build` - reutilizar un `docs/portal/build` existente (utilizando cuando CI no puede
 reconstruir por espejos sin conexión).
- `--skip-sync-openapi` - omitir `npm run sync-openapi` cuando `cargo xtask openapi`
 no pode llegar a crates.io.
- `--skip-sbom` - evita llamar a `syft` cuando o binario no está instalado (o script imprime
 una advertencia en su lugar).
- `--proof` - ejecute `sorafs_cli proof verify` para cada par CAR/manifesto. Os cargas útiles de
 Múltiples archivos aún requieren soporte de chunk-plan en la CLI, así que deja este flag
 Sem establecer si encuentras errores de `plan chunk count` y verificar manualmente cuando
 llegue o gate aguas arriba.
- `--sign` - invoca `sorafs_cli manifest sign`. Probar un token com
 `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) o deja que o CLI lo obtenga usando
 `--sigstore-provider/--sigstore-audience`.Quando envidia artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Ahora empaqueta el portal, OpenAPI y payloads SBOM, assinatura cada manifiesto, y registra metadatos
extra de activos en `portal.additional_assets.json`. O ayudante entiende os mesmos mandos opcionales
Usados por el empaquetador de CI con los nuevos interruptores `--openapi-*`, `--portal-sbom-*`, e.
`--openapi-sbom-*` para asignar tuplas de alias por artefacto, anular una fuente SBOM vía
`--openapi-sbom-source`, omitir ciertas cargas útiles (`--skip-openapi`/`--skip-sbom`),
y apuntar a un binario `syft` no predeterminado con `--syft-bin`.

O script expone cada comando que ejecutar; copia o registro em o boleto de liberación
junto a `package_summary.json` para que los revisores puedan comparar resúmenes de CAR, metadatos de
plan, y hashes del paquete Sigstore sin revisar la salida de shell ad hoc.

## Etapa 9 - Verificación de gateway + SoraDNS

Antes de anunciar una transición, compruebe que el alias nuevo se resuelve a través de SoraDNS y que os gateways
engrapan pruebas frescas:

1. **Ejecutar o puerta de sonda.** `ci/check_sorafs_gateway_probe.sh` ejercita
 `cargo xtask sorafs-gateway-probe` demostración de accesorios contra el sistema operativo
 `fixtures/sorafs_gateway/probe_demo/`. Para despliegues reales, apunta o probe al nombre de host objetivo:

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 O sonda decodifica `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status` según
 `docs/source/sorafs_alias_policy.md` y falla cuando o resumen del manifiesto,
 Los TTL o los enlaces GAR se desvian.Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.

2. **Captura evidencia de taladro.** Para ejercicios de operador o simulacros de PagerDuty, envuelve
 o probe com `scripts/telemetry/run_sorafs_gateway_probe.sh --escenario
 lanzamiento del portal de desarrollo -- ...`. O wrapper guarda headers/logs bajo
 `artifacts/sorafs_gateway_probe/<stamp>/`, actualiza `ops/drill-log.md`, e
 (opcionalmente) dispara ganchos de reversión o cargas útiles PagerDuty. Establece
 `--host docs.sora` para validar una ruta SoraDNS en lugar de codificar una IP.3. **Verifique vinculaciones DNS.** Cuando la gobernanza publica o prueba del alias, registra o archivo GAR
 referenciado por o probe (`--gar`) y adjuntalo a a evidencia de liberación. Os propietarios de resolver
 podem reflejar o mesmo input via `tools/soradns-resolver` para asegurar que as entradas em cache
 Respeten el manifiesto novo. Antes de anexar o JSON, ejecutar
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 para que o mapeo determinista de host, os metadatos del manifesto e as etiquetas de telemetre se
 validar sin conexión. O helper pode emitir un resumen `--json-out` junto al GAR firmado para que os
 Los revisores tengan evidencia verificable sin abrir o binario.
 Cuando redacta um GAR novo, prefiere
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (vuelve a `--manifest-cid <cid>` apenas cuando el archivo de manifiesto no está disponible). Oh ayudante
 ahora deriva el CID **e** o digiere BLAKE3 directamente del manifest JSON, recorta espacios en blanco,
 deduplica flags `--telemetry-label` repetidos, ordenados como etiquetas, y emite plantillas por defecto
 de CSP/HSTS/Permissions-Police antes de escribir o JSON para que o payload se mantenga determinista
 incluso cuando los operadores capturan etiquetas a partir de conchas distintas.

4. **Observar métricas de alias.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 e `torii_sorafs_gateway_refusals_total{profile="docs"}` en pantalla mientras o sonda
 corre; Ambas series están graficadas en `dashboards/grafana/docs_portal.json`.

## Etapa 10 - Monitoramento y paquete de evidencia- **Dashboards.** Exporta `dashboards/grafana/docs_portal.json` (SLOs del portal),
 `dashboards/grafana/sorafs_gateway_observability.json` (latencia del gateway +
 salud de prueba), e `dashboards/grafana/sorafs_fetch_observability.json`
 (salud del orquestador) para cada liberación. Anexe os exporta JSON al ticket de
 lanzamiento para que los revisores puedan reproducir las consultas de Prometheus.
- **Archivos de sonda.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex
 o tu cubo de evidencia. Incluye el resumen de la sonda, encabezados y carga útil PagerDuty
 capturado por o script de telemetría.
- **Bundle de release.** Guarda os resumos de CAR del portal/SBOM/OpenAPI, bundles de manifesto,
 firmas Sigstore, `portal.pin.report.json`, registros de sonda Try-It e informes de link-check
 bajo una carpeta con marca de tiempo (por ejemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Registro de perforación.** Cuando las sondas son parte de un taladro, deja que
 `scripts/telemetry/run_sorafs_gateway_probe.sh` agregar entradas a `ops/drill-log.md`
 para que a mesma evidencia satisfaga o requisito de caos SNNet-5.
- **Enlaces de ticket.** Referencia de los ID del panel de Grafana o exporta PNG adjuntos en el ticket
 de cambio, junto com a ruta del reporte de probe, para que os revisores possam cruzar os SLOs
 sin acceso a un caparazón.

## Etapa 11 - Drill de fetch multi-source e evidencia de marcador

Publicar en SoraFS ahora requiere evidencia de recuperación de múltiples fuentes (DOCS-7/SF-6)
junto a las pruebas de DNS/gatewae anteriores. Después de fijar el manifiesto:1. **Ejecutar `sorafs_fetch` contra o manifesto live.** Usa os mesmos artefactos de plan/manifesto
 producidos en os Etapas 2-3 mas as credenciais de gatewae emitidas para cada proveedor. persistir
 toda a salida para que los auditores puedan reproducir el rastro de decisión del orquestador:

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

 - Busca primero os anuncios de proveedores referenciados por o manifiesto (por ejemplo
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 y pasalos vía `--provider-advert name=path` para que el marcador pueda evaluar ventanas
 de capacidad de forma determinista. Estados Unidos
 `--allow-implicit-provider-metadata` **apenas** cuando reproduce accesorios en CI; ejercicios del sistema operativo
 de produccion deben citar los anuncios firmados que llegaron con o pin.
 - Cuando el manifiesto hace referencia a regiones adicionales, repite el comando con las tuplas de
 proveedores correspondientes para que cada caché/alias tenga un artefacto de fetch asociado.

2. **Archivar como salidas.** Guarda `scoreboard.json`,
 `providers.ndjson`, `fetch.json`, e `chunk_receipts.ndjson` bajo una carpeta de evidencia del
 liberación. Estos capturan archivos o ponderación de pares, o retiro de presupuesto, a latencia EWMA, y os
 recibos por trozos que el paquete de gobernanza debe retener para SF-7.3. **Actualiza telemetría.** Importa como salidas de fetch en el tablero **SoraFS Fetch
 Observabilidad** (`dashboards/grafana/sorafs_fetch_observability.json`),
 observando `torii_sorafs_fetch_duration_ms`/`_failures_total` y los paneles de rango por proveedor
 para anomalías. Enlaza os snapshots de panel Grafana al ticket de release junto com a ruta del
 marcador.

4. **Smokea como reglas de alerta.** Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh`
 para validar el paquete de alertas Prometheus antes de cerrar o liberar. Anexo a salida de
 Promtool al ticket para que los revisores DOCS-7 confirmen que as alertas de parada e
 proveedor lento siguen armadas.

5. **Cablea em CI.** O flujo de trabajo de pin del portal mantem um etapa `sorafs_fetch` detrás de la entrada
 `perform_fetch_probe`; habilitalo para execucoes de staging/produccion para que a evidencia de
 fetch se produce junto al paquete de manifiesto sin manual de intervención. Os ejercicios locales podem
 reutilizar el mismo script exportando los tokens de gatewae y estableciendo `PIN_FETCH_PROVIDERS`
 a una lista de proveedores separados por comas.

## Promoción, observabilidad y retroceso.1. **Promocao:** mantenha alias separados para puesta en escena y producción. Promociona re-ejecutando
 `manifest submit` con el mesmo manifesto/bundle, cambiando
 `--alias-namespace/--alias-name` para apuntar al alias de producción. Esto evita
 reconstruir o re-assinar una vez que QA aprueba o pin de staging.
2. **Monitoramento:** importación del panel de registro de PIN
 (`docs/source/grafana_sorafs_pin_registry.json`) mas os sondas especificos del portal
 (ver `docs/portal/docs/devportal/observability.md`). Alerta em deriva de sumas de control,
 sondas fallidos o picos de retre de prueba.
3. **Rollback:** para revertir, reenviar el manifiesto anterior (o retirar el alias actual) usando
 `sorafs_cli manifest submit --alias ... --retire`. Mantenha siempre o último paquete
 conocido como bueno e o resumo CAR para que as provas de rollback possam recrearse si
 os logs de CI se rotan.

## Plantilla de CI de flujo de trabajo

Como mínimo, tu tubería debe:

1. Build + lint (`npm ci`, `npm run build`, generacion de checksums).
2. Empacotar (`car pack`) e computar manifiestos.
3. Assinar usando el token OIDC del trabajo (`manifest sign`).
4. Subir artefactos (CAR, manifiesto, paquete, plan, resúmenes) para auditoria.
5. Enviar al pin de registro:
 - Solicitudes de extracción -> `docs-preview.sora`.
 - Tags / sucursales protegidas -> promocao de alias de produccion.
6. Ejecutar sondas + puertas de verificación de prueba antes de terminar.

`.github/workflows/docs-portal-sorafs-pin.yml` conecta todas estas etapas para versiones manuales.
O flujo de trabajo:- construye/testea el portal,
- empaqueta o construye vía `scripts/sorafs-pin-release.sh`,
- assinatura/verifique el paquete de manifiesto usando GitHub OIDC,
- sube o CAR/manifesto/bundle/plan/resumos como artefactos, e
- (opcionalmente) envie o manifesto + alias vinculante cuando hae secretos.

Configure los siguientes repositorios de secretos/variables antes de disparar o trabajar:

| Nombre | propuesta |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expone `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrado com envíos. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de assinatura para o presentar el manifiesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias enlazado al manifiesto cuando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Paquete de prueba de alias codificado en base64 (opcional; omita para saltar el enlace de alias). |
| `DOCS_ANALYTICS_*` | Puntos finales de análisis/sonda existentes reutilizados por otros flujos de trabajo. |

Dispara el flujo de trabajo a través de una UI de Acciones:

1. Probar `alias_label` (por ejemplo, `docs.sora.link`), `proposal_alias` opcional,
 y una anulación opcional de `release_tag`.
2. Deja `perform_submit` sin marcar para generar artefactos sin tocar Torii
 (Util para dre run) o habilitarlo para publicar directamente al alias configurado.`docs/source/sorafs_ci_templates.md` aun documenta os helpers de CI genericos para
proyectos fuera de este repositorio, pero el flujo de trabajo del portal debe ser una opción preferida
para lanzamientos del dia a dia.

## Lista de verificación

- [ ] `npm run build`, `npm run test:*`, e `npm run check:links` están en verde.
- [ ] `build/checksums.sha256` e `build/release.json` capturados en artefactos.
- [ ] CAR, plan, manifesto, e resumo generados bajo `artifacts/`.
- [ ] Paquete Sigstore + assinatura separadas almacenadas con logs.
- [ ] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
 capturados cuando hae presentaciones.
- [ ] `portal.pin.report.json` (y opcional `portal.pin.proposal.json`)
 archivado junto a os artefactos CAR/manifesto.
- [ ] Registros de `proof verify` e `manifest verify-signature` archivados.
- [ ] Dashboards de Grafana actualizados + sondas Try-It exitosas.
- [] Notas de rollback (ID de manifiesto anterior + resumen de alias) adjuntas al
 billete de liberación.