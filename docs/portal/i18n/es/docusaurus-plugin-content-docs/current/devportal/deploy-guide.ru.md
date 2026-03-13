---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Objeto

Este bloque de almacenamiento permite colocar puntos en tarjetas **DOCS-7** (publicación SoraFS) y **DOCS-8** (pin automático en CI/CD) en Procedimiento práctico para el portal разработчиков. En el caso de build/lint, se utiliza SoraFS, se pueden configurar archivos Sigstore, otros productos, proveedores y тренировки отката, чтобы каждый vista previa y lanzamiento были воспроизводимыми и аудируемыми.

Este es un archivo predeterminado que está binario `sorafs_cli` (combinado con `--features cli`), descarga el punto final Torii con un registro pin completo y OIDC está conectado a Sigstore. Todos los secretos (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) se encuentran en la bóveda de CI; Los archivos locales se pueden exportar en Shell.

## Предварительные условия- Nodo 18.18+ con `npm` o `pnpm`.
- `sorafs_cli` y `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii, introduzca el código `/v2/sorafs/*`, haga clic más o haga clic en el botón izquierdo del ratón y muestre los manifiestos. алиасы.
- Emisor OIDC (GitHub Actions, GitLab, identidad de carga de trabajo y т. п.) para el usuario `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para ejecución en seco y `docs/source/sorafs_ci_templates.md` para flujos de trabajo de GitHub/GitLab.
- Instale OAuth permanentemente Pruébelo (`DOCS_OAUTH_*`) y presione
  [lista de verificación de refuerzo de seguridad](./security-hardening.md) перед продвижением билда за пределы lab. Tenga en cuenta que el portal de inicio de sesión, la configuración permanente o los parámetros TTL/Pulling están configurados de forma predeterminada; `DOCS_OAUTH_ALLOW_INSECURE=1` utiliza esta opción para obtener una vista previa local. Realice una prueba de penetración con una prueba de precisión.

## Шаг 0 - Зафиксировать paquete de paquetes Pruébalo

Antes de la vista previa de la vista previa en Netlify o la puerta de enlace de la instalación de configuración Pruébelo, proxy y resumen del paquete OpenAPI se muestran en el paquete predeterminado:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copia las ayudas de proxy/sonda/reversión, puede verificar OpenAPI y пишет `release.json` más `checksums.sha256`. Solicite este paquete con el ticket de puerta de enlace de Netlify/SoraFS, cuáles son los principales proveedores de procesos y aplicaciones Torii objetivo без пересборки. Paquete de funciones, incluidos clientes de tokens portadores (`allow_client_auth`), implementación de planes y sincronización de CSP.

## Paso 1 - Portal Build y Lint

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

`npm run build` автоматически запускает `scripts/write-checksums.mjs`, создавая:

- `build/checksums.sha256` - Manifestante SHA256 para `sha256sum -c`.
- `build/release.json` - metadano (`tag`, `generated_at`, `source`) para el coche/manifestador.

Архивируйте оба файла вместе с CAR resumen, чтобы ревьюеры могли сравнивать артефакты без пересборки.

## Paso 2 - Упаковать статические ассеты

Introduzca el empaquetador de automóviles en el catálogo actual Docusaurus. Por primera vez, no hay detalles sobre los artefactos en `artifacts/devportal/`.

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

Resumen de archivos JSON de archivos, datos y datos para la planificación, datos de los paneles `manifest build` y CI.

## Paso 2b - Упаковать OpenAPI y compañeros de SBOMEl archivo DOCS-7 que se publica en el portal, la instantánea OpenAPI y las cargas útiles de SBOM como manifiestos externos y las puertas de enlace que se pueden implementar Encabezados `Sora-Proof`/`Sora-Content-CID` para el arte del arte. Release helper (`scripts/sorafs-pin-release.sh`) en el paquete OpenAPI, catálogo (`static/openapi/`) y SBOM en `syft` en otros coches `openapi.*`/`*-sbom.*` y se almacenan en `artifacts/sorafs/portal.additional_assets.json`. En el primer paso, muestre las secciones 2-4 de la carga útil del archivo para obtener ajustes y metadanos (por ejemplo, `--car-out "$OUT"/openapi.car` adicionalmente). `--metadata alias_label=docs.sora.link/openapi`). Registre el archivo de manifiesto/alias en Torii (por ejemplo, OpenAPI, portal SBOM, OpenAPI SBOM) para la configuración DNS, puerta de enlace мог отдавать pruebas grapadas для всех опубликованных артефактов.

## Capítulo 3 - Собрать манифест

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

Instale la bandera de política de PIN para activarla (por ejemplo, `--pin-storage-class hot` para el canal). Variante JSON opcional, no disponible para la revisión.

## Paso 4 - Подписать через Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Paquete de archivos adjuntos, archivos adjuntos y BLAKE3 con token OIDC, no compatible con JWT. Paquete completo y firma separada; продакшен продвижения могут переиспользовать те же артефакты вместо повторной подписи. Las aplicaciones locales pueden cambiar la bandera del proveedor en `--identity-token-env` (o descargar `SIGSTORE_ID_TOKEN` en abierto), junto con OIDC. ayudante выпускает токен.## Paso 5 - Отправить в registro de PIN

Utilice un manifiesto múltiple (y un plan fragmentado) en Torii. Всегда запрашивайте resumen, чтобы запись registro/alias была аудируемой.

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

La vista previa del lanzamiento y el alias canario (`docs-preview.sora`) se muestran con un alias único, y el control de calidad puede proteger el contenido antes de la promoción de producción.

Enlace de alias tres polos: `--alias-namespace`, `--alias-name` y `--alias-proof`. Paquete de prueba de gobierno (base64 o Norito bytes) después de un alias; Eche un vistazo a CI secrets y escriba un archivo antes de `manifest submit`. Instale las banderas de alias, o puede abrir el manifiesto mediante la configuración DNS.

## Шаг 5b - Сгенерировать propuesta de gobernanza

Каждый манифест должен сопровождаться парламентским propuesta, чтобы любой гражданин Sora мог внести изменение без доступа к привилегированным ключам. Después de enviar/firmar выполните:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` фиксирует каноническую инструкцию `RegisterPinManifest`, resumen de códigos, políticas y sugerencias de alias. Como parte del ticket de gobernanza o del portal del Parlamento, los delegados pueden crear una carga útil en los peresbourkis. Si no utiliza las teclas Torii, esta máquina de escribir puede permitir la propuesta local.

## Paso 6 - Pruebas y televisores

Después de pin выполните детерминированные проверки:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Muévase por `torii_sorafs_gateway_refusals_total` y `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Introduzca `npm run probe:portal`, para obtener el proxy Try-It y aplicaciones gratuitas en su nuevo contenido.
- Соберите evidencia monitoreo из
  [Publicación y seguimiento](./publishing-monitoring.md), чтобы соблюсти Puerta de observabilidad DOCS-3c. Helper теперь принимает несколько I18NI000000135X (сайт, OpenAPI, portal SBOM, OpenAPI SBOM) y требует `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` en el alojamiento principal de la protección `hostname`. No hay ningún resumen JSON completo y paquete de evidencia (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) en el catálogo:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - Планирование сертификатов gateway

Forme un plan TLS SAN/challenge para almacenar paquetes GAR, puertas de enlace de comandos y aprobadores de DNS relacionados con la evidencia antigua. El nuevo asistente incluye entradas DG-3, hosts comodín canónicos, SAN de host bonito, etiquetas DNS-01 y desafíos ACME recomendados:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Convierta el paquete de versión JSON (o solicite un boleto de cambio), los operadores pueden configurar la configuración de SAN en la configuración `torii.sorafs_gateway.acme`, GAR Los revisores suelen publicar asignaciones canónicas/bonitas sin una publicación reciente. Coloque la configuración `--name` para el producto en una versión determinada.

## Paso 6b - Получить asignaciones de hosts canónicosAntes de configurar las cargas útiles de GAR, configure el mapeo de host determinado para el alias del archivo. `cargo xtask soradns-hosts` incluye el nombre de `--name` en la etiqueta canónica (`<base32>.gw.sora.id`), emite un comodín nuevo (`*.gw.sora.id`) y un host bonito. (`<alias>.gw.sora.name`). Сохраняйте вывод в lanzamiento de artefactos, чтобы DG-3 revisores могли сверять mapeo рядом с GAR envío:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilice `--verify-host-patterns <file>` para actualizar el código GAR o el JSON de enlace de puerta de enlace que no admite archivos adjuntos. El asistente contiene nuevos archivos, una prueba legal y una plantilla GAR y `portal.gateway.binding.json` con el siguiente comando:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Resumen de resumen de JSON y registro de verificación de ticket de cambio de DNS/puerta de enlace, los auditores pueden almacenar hosts canónicos/comodines/pretty sin necesidad de un registro de verificación actual. Перезапускайте команду при добавлении новых alias, чтобы GAR actualiza наследовали те же evidencia.

## Paso 7 - Crear descriptor de transición de DNS

Recortes de producción требуют аудируемого paquete de cambio. Ayudante de envío posterior (enlace de alias) generado por `artifacts/sorafs/portal.dns-cutover.json`, ficticio:- enlace de alias de metadatos (espacio de nombres/nombre/prueba, resumen de manifiesto, URL Torii, época de envío, autoridad);
- contexto de lanzamiento (etiqueta, etiqueta de alias, manifiesto/CAR пути, plan de fragmentos, paquete Sigstore);
- punteros de verificación (comando de sonda, alias + punto final Torii); y
- Opciones de control de cambios (identificación del ticket, ventana de transición, contacto de operaciones, nombre de host/zona de producción);
- promoción de ruta de metadatos con encabezado `Sora-Route-Binding` (host/CID canónico, encabezado de enlace + enlace, comandos de configuración), promoción de GAR y ejercicios de respaldo combinados con evidencia nueva;
- Artefactos de plan de ruta genéricos (`gateway.route_plan.json`, plantillas de encabezado y encabezados de reversión opcionales), boletos de cambio y ganchos de pelusa CI para mayor protección, como paquete DG-3. en planes canónicos de promoción/reversión;
- Metadatos opcionales para la invalidación de caché (punto final de purga, variable de autenticación, carga útil JSON y modelo `curl`);
- sugerencias de reversión para el descriptor anterior (etiqueta de lanzamiento y resumen de manifiesto), cómo cambiar boletos de respaldo ficticio determinista.

Para liberar la purga de caché del archivo, el plan canónico creado incluye el descriptor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Utilice `portal.cache_plan.json` para el paquete DG-3, cuyos operadores determinan hosts/rutas (y sugerencias de autenticación) para `PURGE`. запросах. El descriptor de sección de caché opcional puede incluirse en este archivo de configuración, donde se incluyen revisores de control de cambios y puntos finales que se encuentran en la transición.

Paquete Каждый DG-3 также требует promoción + lista de verificación de reversión. Creado desde `cargo xtask soradns-route-plan`, estos revisores de control de cambios pueden eliminar la verificación previa/corte/reversión de un alias de alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` incluye hosts canónicos/bonitos, recordatorios de verificación de estado por etapas, actualizaciones de enlaces GAR, purgas de caché y acciones de reversión. Si utiliza dispositivos GAR/binding/cutover para cambiar el billete, es posible que las operaciones repitan el cambio.

`scripts/generate-dns-cutover-plan.mjs` forma el descriptor y se escribe en `sorafs-pin-release.sh`. Для ручной генерации:

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

Utilice metadatos opcionales para realizar ajustes previos al cerrar el asistente de pin:| Permanente | Назначение |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID del billete, который сохраняется в descriptor. |
| `DNS_CUTOVER_WINDOW` | ISO8601 окно cutover (por ejemplo, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nombre de host de producción + zona autorizada. |
| `DNS_OPS_CONTACT` | Alias ​​de guardia o contactos de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Punto final de purga, записываемый в descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var с token de purga (predeterminado: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Coloque el descriptor de transición anterior para los metadatos de reversión. |

Registre JSON en la revisión de cambios de DNS, los aprobadores pueden verificar el resumen del manifiesto, enlaces de alias y comandos de sonda en los logotipos de CI. Banderas CLI `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`, `--cache-purge-auth-env` y `--previous-dns-plan` muestra la anulación del asistente de eliminación del CI.

## Paso 8 - Formar el esqueleto del archivo de zona de resolución (opcional)Una ventana de transición de producción abierta, un script de lanzamiento puede generar automáticamente el esqueleto del archivo de zona SNS y el fragmento de resolución. Asegúrese de que no haya archivos DNS y metadatos en el entorno o la CLI; El asistente utiliza `scripts/sns_zonefile_skeleton.py` según el descriptor de transición de generación. Utilice el resumen A/AAAA/CNAME y GAR (BLAKE3-256 compatible con la carga útil de GAR). Si la zona/nombre de host está disponible en la aplicación `--dns-zonefile-out`, el asistente busca en `artifacts/sns/zonefiles/<zone>/<hostname>.json` y utiliza el fragmento de resolución del archivo `ops/soradns/static_zones.<hostname>.json`.| Permanente / bandera | Назначение |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Utilice el esqueleto del archivo de zona. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Guarde el fragmento de resolución (por ejemplo `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL para записей (predeterminado: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Dirección IPv4 (entorno separado por comas o bandera de correo electrónico). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Dirección IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Destino CNAME opcional. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pines SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Mensajes TXT duplicados (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Anular la etiqueta de versión del archivo de zona calculado. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Utilice la marca de tiempo `effective_at` (RFC3339) en la ventana de transición. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Anular el literal de prueba en los metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Anular el CID en los metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de congelación del guardián (suave, duro, descongelación, monitoreo, emergencia). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia del billete de tutor/consejo para congelar. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 marca de tiempo para la descongelación. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Дополнительные congelar notas (env separados por comas o env повторяемый флаг). || `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 resumen (hexadecimal) carga útil de GAR. Требуется при enlaces de puerta de enlace. |

Flujo de trabajo de acciones de GitHub que incluye un repositorio de secretos, un pin de producción automático que elimina artefactos de archivos de zona. Establezca secretos de secretos (puede utilizar listas separadas por comas para una pole de valores múltiples):

| Secreto | Назначение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nombre de host de producción/zona del ayudante. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de guardia â descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 descargas para publicaciones. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Destino CNAME opcional. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pines SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные TXT записи. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Congelar metadatos en el esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex BLAKE3 digest подписанного GAR carga útil. |

Al escribir `.github/workflows/docs-portal-sorafs-pin.yml`, use las entradas `dns_change_ticket` e `dns_cutover_window`, ya que estos descriptores/archivos de zona no son correctos. Asegúrese de que esto sea necesario para un funcionamiento en seco.

Comando típico (según el propietario del runbook SN-7):

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

Los asistentes automáticos activan el cambio de ticket en TXT y abren la ventana de transición como `effective_at`, o no. Полный flujo de trabajo operativo см. en `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Примечание о публичной DNS-делегацииEl archivo de zona seleccionado está disponible para zonas automáticas. Delegado NS/DS
родительской зоны нужно настроить у регистратора или DNS-провайдера, чтобы
Hay muchos servidores de nombres disponibles en Internet.

- Para la transición a Apex/TLD se utiliza ALIAS/ANAME (descarga del proveedor) o
  Publique la información A/AAAA y colóquela en la puerta de enlace Anycast-IP.
- Este usuario publica CNAME en un host bastante derivado
  (`<fqdn>.gw.sora.name`).
- Канонический хост (`<hash>.gw.sora.id`) instalado en la puerta de enlace doméstica y no
  публикуется в вашей публичной зоне.

### Puerta de enlace de Shablon заголовков

Implemente el asistente de implementación de los generadores `portal.gateway.headers.txt` e `portal.gateway.binding.json`: dos artefactos que integran el enlace de contenido de puerta de enlace DG-3:

- `portal.gateway.headers.txt` incluye encabezados HTTP de bloque de polos (incluidos `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS e `Sora-Route-Binding`), La puerta de enlace Edge debe conectarse a un dispositivo externo.
- `portal.gateway.binding.json` Ficha para obtener información sobre el vídeo de la máquina, cómo cambiar billetes y cómo automatizar enlaces host/cid без парсинга shell вывода.

Estos generadores son `cargo xtask soradns-binding-template` y alias de archivos, resumen de manifiesto y nombre de host de puerta de enlace, que se encuentran en `sorafs-pin-release.sh`. Los encabezados de bloques de regeneración o de conversión incluyen:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```Asegúrese de que `--csp-template`, `--permissions-template` o `--hsts-template`, чтобы переопределить шаблоны заголовков; Utilice la bandera `--no-*` para el encabezado del grupo.

Utilice fragmentos de código de solicitud de cambio de CDN y actualice JSON en la canalización de automatización de puerta de enlace, lo que proporciona evidencia real de datos.

Guión de lanzamiento автоматически запускает ayudante de verificación, чтобы DG-3 tickets всегда содержали свежую evidencia. Tenga en cuenta que es necesario vincular el formato JSON:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

El comando decodificación grapado `Sora-Proof`, proporciona la información `Sora-Route-Binding` con CID de manifiesto + nombre de host y contiene encabezados separados. Архивируйте вывод вместе с другими release artefactos при запуске вне CI, чтобы DG-3 revisores имели доказательство проверки.

> **Integración descriptor DNS:** `portal.dns-cutover.json` теперь включает секцию `gateway_binding`, указывающую на эти артефакты (пути, content CID, prueba de estado y plantilla de encabezado literal) **y** sección `route_plan`, conexión en `gateway.route_plan.json` y plantilla de actualización/reversión. Agregue estos bloques al boleto de cambio DG-3, todos los revisores pueden buscar la información `Sora-Name/Sora-Proof/CSP` y enviar mensajes de correo electrónico Paquete de evidencia de promoción/reversión de planes без открытия build архива.

## Шаг 9 - Запустить monitores de publicaciónEl elemento de la hoja de ruta **DOCS-3c** contiene enlaces de proxy y puerta de enlace que están disponibles después de la resolución. Запускайте monitor consolidado сразу после шагов 7-8 y подключайте его к вашим sondas programadas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` загружает config (см. `docs/portal/docs/devportal/publishing-monitoring.md` для схемы) y выполняет три проверки: sondas путей портала + CSP/validación de políticas de permisos, Pruébelo sondas proxy (opcional `/metrics`), y el verificador de enlace de puerta de enlace (`cargo xtask soradns-verify-binding`), el archivo de configuración actual y el software de Sora-Content-CID con alias/manifest проверками.
- El comando guarda con un valor distinto de cero, cuando la sonda está activada, los operadores de CI/cron/runbook pueden establecer una respuesta al alias del producto.
- El indicador `--json-out` muestra un resumen JSON según el formato; `--evidence-dir` incluye `summary.json`, `portal.json`, `tryit.json`, `binding.json` y `checksums.sha256`, los revisores de gobernanza más importantes результаты без повторного запуска мониторинга. Obtenga este catálogo bajo `artifacts/sorafs/<tag>/monitoring/` junto con el paquete Sigstore y el descriptor de transferencia de DNS.
- Haga clic en el monitor, exporte Grafana (`dashboards/grafana/docs_portal.json`) y Alertmanager Drill ID en el ticket de lanzamiento, y DOCS-3c SLO puede ser una opción de audio. Monitoreo del libro de jugadas publicado en `docs/portal/docs/devportal/publishing-monitoring.md`.Las sondas del portal envían HTTPS y bloquean la URL `http://`, y no utilizan `allowInsecureHttp` en la configuración del monitor; Configure la producción/puesta en escena en TLS y anule la vista previa local.

Para automatizar el monitor, consulte `npm run monitor:publishing` en Buildkite/cron después del portal de inicio. Este comando incluye URL de producción y puede realizar controles de salud posteriores.

## Automático por `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula la sección 2-6. En:

1. архивирует `build/` en детерминированный tarball,
2. запускает `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` y `proof verify`,
3. Opción de enlace de alias `manifest submit` (vinculación de alias) para credenciales Torii, y
4. пишет `artifacts/sorafs/portal.pin.report.json`, опциональный `portal.pin.proposal.json`, descriptor de transición de DNS (envío de enlace) y paquete de enlace de puerta de enlace (`portal.gateway.binding.json` más bloque de encabezado), чтобы comandos de gobernanza/redes/operaciones Se pueden verificar pruebas mediante el uso de los logotipos de CI.

Antes de iniciar sesión, instale `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` y (opcionalmente) `PIN_ALIAS_PROOF_PATH`. Utilice `--skip-submit` para un funcionamiento en seco; El flujo de trabajo de GitHub no requiere esta entrada `perform_submit`.

## Paso 8 - Publicación de especificaciones OpenAPI y paquetes SBOM

El archivo DOCS-7, este portal, las especificaciones OpenAPI y los artefactos SBOM proporcionan datos y toda la tubería determinada. Los ayudantes de Имеющиеся покрывают все три:1. **Implementar y actualizar las especificaciones.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Utilice `--version=<label>` para una instantánea histórica histórica (nombre `2025-q3`). El asistente muestra la instantánea en `static/openapi/versions/<label>/torii.json`, la descarga en `versions/current` y ​​completa los metadatos (SHA-256, estado del manifiesto, marca de tiempo actualizada) en `static/openapi/versions.json`. El portal utiliza estos índices para las versiones y la firma del resumen/firma. Además de `--version`, no se incluyen etiquetas personalizadas ni etiquetas nuevas `current` + `latest`.

   Resúmenes de archivos de manifiesto SHA-256/BLAKE3, cuya puerta de enlace puede bloquear `Sora-Proof` para `/reference/torii-swagger`.

2. **Сформировать CycloneDX SBOM.** Canal de lanzamiento para actualizar los SBOM basados ​​en syft según `docs/source/sorafs_release_pipeline_plan.md`. Держите вывод рядом с construir artefactos:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Упаковать каждый carga útil en CAR.**

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

   Seleccione el nombre `manifest build`/`manifest sign`, y para su nombre original, seleccione otro alias en el arte (como, por ejemplo, `docs-openapi.sora` para especificaciones y `docs-sbom.sora` para SBOM). Los alias antiguos instalados en SoraDNS, GAR y tickets de reversión privan a la carga útil de la configuración.

4. **Enviar y vincular.** Consulte la autoridad técnica y el paquete Sigstore, no fiksirуйте alias tupla en la lista de verificación de lanzamiento, чтобы аудиторы знали, какой nombre de Sora соответствует какому resumen.Архивация spec/SBOM maniфестов вместе с build портала гарантирует, что каждый release ticket содержит полный набор артефактов без повторного empacador de запуска.

### Asistente de automatización (CI/script de paquete)

`./ci/package_docs_portal_sorafs.sh` кодирует шаги 1-8, чтобы roadmap item **DOCS-7** можно было выполнить одной командой. Ayudante:

- выполняет подготовку портала (sincronización `npm ci`, OpenAPI/Norito, pruebas de widgets);
- formar CARs y pares de manifiestos en el portal, OpenAPI y SBOM entre `sorafs_cli`;
- firma opcional `sorafs_cli proof verify` (`--proof`) y Sigstore (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- Clade все артефакты в `artifacts/devportal/sorafs/<timestamp>/` y пишет `package_summary.json` para CI/herramientas de liberación; y
- обновляет `artifacts/devportal/sorafs/latest` на последний запуск.

Ejemplo (canalización completa con Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Banderas polacas:- `--out <dir>` - anular корня артефактов (по умолчанию каталоги с timestamp).
- `--skip-build` - использовать существующий `docs/portal/build` (полезно при mirrors offline).
- `--skip-sync-openapi` - introduzca `npm run sync-openapi`, mientras que `cargo xtask openapi` no puede enviarse a crates.io.
- `--skip-sbom` - не вызывать `syft`, если бинарь не установлен (script печатает предупреждение).
- `--proof` - выполнить `sorafs_cli proof verify` для каждой пары CAR/manifest. Las cargas útiles de múltiples funciones incluyen soporte de plan de fragmentos en la CLI, y pueden instalar esta bandera en el dispositivo `plan chunk count`. проверяйте вручную после появления puerta.
- `--sign` - вызвать `sorafs_cli manifest sign`. Conecte el token a `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) y conecte la CLI a `--sigstore-provider/--sigstore-audience`.

Para los artefactos de producción se utiliza `docs/portal/scripts/sorafs-pin-release.sh`. En el portal de paquetes, OpenAPI y SBOM, se pueden guardar el manifiesto del archivo y se almacenan los metadatos compartidos en `portal.additional_assets.json`. El asistente activa las perillas, el empaquetador CI y las nuevas tuplas de alias `--openapi-*`, `--portal-sbom-*` e `--openapi-sbom-*` para anular la fuente SBOM. `--openapi-sbom-source`, cargas útiles adicionales (`--skip-openapi`/`--skip-sbom`) y ubicación predeterminada `syft` `--syft-bin`.Script выводит все команды; Siga el registro en el ticket de lanzamiento en `package_summary.json`, estos revisores pueden consultar resúmenes de CAR, metadatos del plan y hashes de paquetes Sigstore sin ayuda del programa. случайного shell вывода.

## Paso 9 - Puerta de enlace de proveedor + SoraDNS

Antes de comprobar la transición, qué nuevas soluciones de alias tienen SoraDNS y las puertas de enlace muestran algunas pruebas:

1. **Запустить sonda gate.** `ci/check_sorafs_gateway_probe.sh` coloca `cargo xtask sorafs-gateway-probe` en accesorios de demostración en `fixtures/sorafs_gateway/probe_demo/`. Los implementadores reales utilizan el nombre de host de destino:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Las sondas codifican `Sora-Name`, `Sora-Proof` y `Sora-Proof-Status` y `docs/source/sorafs_alias_policy.md` y se conectan a un manifiesto de resumen, enlaces TTL o GAR.

   Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.

2. **Собрать evidencia для taladro.** Для operador taladros или PagerDuty simulacros используйте `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. El contenedor incluye encabezados/registros en `artifacts/sorafs_gateway_probe/<stamp>/`, incluye `ops/drill-log.md` y opcionalmente incluye ganchos de reversión y cargas útiles de PagerDuty. Instale `--host docs.sora`, para probar SoraDNS, sin IP.3. **Ver enlaces de DNS.** La gobernanza de Cogda publica prueba de alias, actualiza el archivo GAR, utiliza la sonda (`--gar`) y solicita evidencia de liberación. Los propietarios del Resolver pueden hacer un seguimiento de su `tools/soradns-resolver`, de cómo hacerlo o de sus nuevas actualizaciones. manifiesto. Antes de que los archivos JSON incluyan `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`, todas las asignaciones de host determinadas, los manifiestos de metadatos y las etiquetas de telemetría están disponibles. El ayudante puede formalizar el resumen `--json-out` рядом с подписанным GAR.
  En una nueva versión de GAR, utilice `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (cambie a `--manifest-cid <cid>` junto con un archivo de manifiesto externo). El tipo de ayuda contiene CID ** y ** BLAKE3 digest desde el manifiesto JSON, descarga de problemas, descarga de archivos `--telemetry-label`, etiquetas de clasificación y Las plantillas predeterminadas de CSP/HSTS/Permisos-Política se generan mediante JSON y la carga útil se establece de forma predeterminada.

4. **Следить за метриками alias.** Introduzca la pantalla `torii_sorafs_alias_cache_refresh_duration_ms` y `torii_sorafs_gateway_refusals_total{profile="docs"}`; обе серии есть в `dashboards/grafana/docs_portal.json`.

## Paso 10 - Monitoreo y agrupación de evidencia- **Paneles de control.** Exporte `dashboards/grafana/docs_portal.json` (portal SLO), `dashboards/grafana/sorafs_gateway_observability.json` (puerta de enlace de latencia + estado de prueba) y `dashboards/grafana/sorafs_fetch_observability.json` (estado del orquestador) para el registro de salida. Прикладывайте JSON экспорт к lanzamiento de ticket.
- **Archivos de sonda.** Consulte `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex o en el depósito de evidencia. Vea el resumen de la sonda, los encabezados y la carga útil de PagerDuty.
- **Paquete de lanzamiento.** Portal de resumen de CAR actualizado/SBOM/OpenAPI, paquetes de manifiesto, firmas Sigstore, `portal.pin.report.json`, registros de sonda Try-It e informes de verificación de enlaces en el nuevo documento (por ejemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Registro de perforación.** Когда sondas - часть drill, `scripts/telemetry/run_sorafs_gateway_probe.sh` добавляет запись в `ops/drill-log.md`, чтобы это покрывало SNNet-5 caos requisito.
- **Enlaces de tickets.** Utilice los ID del panel Grafana o exporte PNG en el ticket de cambio desde el informe de sonda, y puede consultar los SLO desde el enlace. shell доступа.

## Paso 11 - Ejercicio de búsqueda de fuentes múltiples y evidencia de marcador

Publicado en SoraFS, puede buscar evidencia de recuperación de múltiples fuentes (DOCS-7/SF-6) junto con pruebas de DNS/gateway. После pin manifiesto:

1. **Ingrese `sorafs_fetch` en el manifiesto en vivo.** Utilice los artefactos del plan/manifiesto entre los números 2 y 3 y las credenciales de la puerta de enlace del administrador del código. Сохраняйте все выходные данные, чтобы аудиторы могли повторить решение Orchestrator:

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
   ```- Busque anuncios de proveedores, coloque un manifiesto (por ejemplo, `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) y consulte su marcador `--provider-advert name=path`. оценивал окна возможностей детерминированно. `--allow-implicit-provider-metadata` utiliza **только** para el equipo CI; taladros de producción должны ссылаться на подписанные anuncios из pin.
   - En las regiones más populares se pueden crear comandos con tuplas de proveedores compatibles, caché/alias de caché o archivos de recuperación. artefacto.

2. **Архивировать выходы.** Сохраняйте `scoreboard.json`, `providers.ndjson`, `fetch.json` y `chunk_receipts.ndjson` en el catálogo de pruebas. Estos archivos incluyen ponderación de pares, presupuesto de reintento, latencia EWMA y recibos de fragmentos para SF-7.

3. **Actualizar telemetría.** Importar salidas de búsqueda en el tablero **SoraFS Observabilidad de búsqueda** (`dashboards/grafana/sorafs_fetch_observability.json`), deslícese desde `torii_sorafs_fetch_duration_ms`/`_failures_total` y панелями по rangos de proveedores. Coloque instantáneas Grafana en el boleto de lanzamiento junto con el marcador.

4. **Mostrar reglas de alerta.** Presione `scripts/telemetry/test_sorafs_fetch_alerts.sh` para proteger el paquete de alertas Prometheus para la descarga. Pruebe la salida de Promtool, los revisores de DOCS-7 que pueden activarse y las alertas de bloqueo/proveedor lento.5. **Подключить в CI.** Flujo de trabajo del pin del portal desde `sorafs_fetch` desde la entrada `perform_fetch_probe`; включите его для puesta en escena/producción, чтобы buscar evidencia формировалась вместе с manifiesto paquete. Los ejercicios locales pueden implementarse en el script, exportar tokens de puerta de enlace y datos `PIN_FETCH_PROVIDERS` (programas separados por comas).

## Promoción, observabilidad y reversión

1. **Promoción:** держите отдельные alias de puesta en escena y producción. Produzca, primero, `manifest submit` con el manifiesto/paquete, entre ellos `--alias-namespace/--alias-name` con el alias de producción. Esto se excluye después del control de calidad de la aprobación.
2. **Monitoreo:** registro de pines del tablero (`docs/source/grafana_sorafs_pin_registry.json`) y sondas del portal (`docs/portal/docs/devportal/observability.md`). Utilice la deriva de la suma de comprobación, las sondas fallidas o la prueba de reintento de picos.
3. **Revertir:** para eliminar el manifiesto anterior (o retirar el alias actual) en `sorafs_cli manifest submit --alias ... --retire`. Además de un paquete en buen estado conocido y un resumen de CAR, muchas pruebas de reversión pueden ser una gran cantidad de registros de CI de rotación.

## Flujo de trabajo de CI de Shablon

Минимальный tubería должен:1. Build + lint (`npm ci`, `npm run build`, sumas de comprobación de generación).
2. Упаковка (`car pack`) и вычисление manifiesto.
3. Introduzca el token OIDC con ámbito de trabajo (`manifest sign`).
4. Загрузка артефактов (CAR, manifiesto, paquete, plan, resúmenes) для аудита.
5. Отправка в registro de PIN:
   - Solicitudes de extracción -> `docs-preview.sora`.
   - Etiquetas/ramas protegidas -> alias de producción de promoción.
6. Выполнение sondas + puertas de verificación de prueba перед завершением.

`.github/workflows/docs-portal-sorafs-pin.yml` utiliza estas versiones para versiones manuales. Flujo de trabajo:

- construir/probar el portal,
- упаковка build через `scripts/sorafs-pin-release.sh`,
- подпись/проверка manifiesto paquete через GitHub OIDC,
- загрузка CAR/manifest/bundle/plan/proof resúmenes как artefactos, и
- (опционально) отправка manifest + alias vinculante при наличии secretos.

Configure los secretos/variables del repositorio antes del trabajo:

| Nombre | Назначение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii con `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época, записанный при envío. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Подписывающая autoridad для presentación манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tupla de alias, creada por `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Paquete de prueba de alias Base64 (opcional). |
| `DOCS_ANALYTICS_*` | Существующие análisis/puntos finales de sonda. |

Complete el flujo de trabajo en la interfaz de usuario de acciones:1. Utilice `alias_label` (por ejemplo, `docs.sora.link`), opcionalmente `proposal_alias` y opcionalmente `release_tag`.
2. Instale el artefacto `perform_submit` en el generador de artefactos junto con Torii (ejecución en seco) o presione las publicaciones en alias настроенный.

`docs/source/sorafs_ci_templates.md` ofrece ayudas de CI para proyectos de software, pero no para aplicaciones de flujo de trabajo del portal.

## Cheklist

- [ ] `npm run build`, `npm run test:*` y `npm run check:links`.
- [] `build/checksums.sha256` y `build/release.json` asociados con artefactos.
- [] CAR, plan, manifiesto y resumen de los diseños de `artifacts/`.
- [] Paquete Sigstore + firma separada сохранены с логами.
- [] `portal.manifest.submit.summary.json` e `portal.manifest.submit.response.json` relacionados con el envío.
- [ ] `portal.pin.report.json` (y opcionalmente `portal.pin.proposal.json`) архивированы рядом с CAR/artefactos manifiestos.
- [ ] Логи `proof verify` y `manifest verify-signature` сохранены.
- [] Paneles de control Grafana actualizados + sondas Try-It disponibles.
- [] Notas de reversión (ID предыдущего манифеста + resumen de alias) приложены к ticket de liberación.