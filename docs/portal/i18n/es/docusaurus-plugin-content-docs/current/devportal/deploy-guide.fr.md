---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Vista del conjunto

Este libro de jugadas convierte los elementos de la hoja de ruta **DOCS-7** (publicación SoraFS) y **DOCS-8**
(Automatización del pin CI/CD) en un procedimiento ejecutable para el desarrollador minorista.
Il cuuvre la fase build/lint, el embalaje SoraFS, la firma de manifiestos con Sigstore,
la promoción de alias, la verificación y los ejercicios de reversión para cada vista previa y lanzamiento
es reproducible y auditable.

El flujo supone que tienes el binario `sorafs_cli` (compila con `--features cli`), accede a
un punto final Torii con los permisos de registro PIN y las credenciales OIDC para Sigstore.
Stockez les secrets longue durée (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) en
su cofre CI; Las ejecuciones locales pueden cargarse a través de las exportaciones del shell.

## Requisitos previos- Nodo 18.18+ con `npm` o `pnpm`.
- `sorafs_cli` vía `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii que expone `/v2/sorafs/*` plus una cuenta/cle privee d'autorite qui peut soumettre
  des manifestes et des alias.
- Emmeteur OIDC (GitHub Actions, GitLab, identidad de carga de trabajo, etc.) para emmettre un `SIGSTORE_ID_TOKEN`.
- Opcional: `examples/sorafs_cli_quickstart.sh` para ensayos en seco y
  `docs/source/sorafs_ci_templates.md` para el andamiaje de los flujos de trabajo GitHub/GitLab.
- Configura las variables OAuth Pruébalo (`DOCS_OAUTH_*`) y ejecuta la
  [lista de verificación de refuerzo de seguridad](./security-hardening.md) antes de promover una compilación
  fuera del laboratorio. La construcción del portal se hace eco del mantenimiento cuando estas variables se mantienen constantemente
  o cuando las perillas TTL/polling ordenan las ventanas impuestas; exportez
  `DOCS_OAUTH_ALLOW_INSECURE=1` exclusivo para vistas previas locales disponibles. joignez
  les preuves de pen-test au ticket de liberación.

## Etapa 0 - Capturador de un paquete de proxy Pruébalo

Antes de promocionar una vista previa de Netlify o la puerta de enlace, indique las fuentes del proxy Pruébelo
y el resumen del manifiesto OpenAPI firma en un paquete determinante:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copia los ayudantes proxy/probe/rollback, verifica la firma
OpenAPI y escriba `release.json` más `checksums.sha256`. Joignez ce bundle au ticket de
promoción Netlify/SoraFS gateway para que los revisores puedan mejorar las fuentes exactas
y las sugerencias del objetivo Torii sin reconstruir. El paquete se registra también si los portadores
Fournis par le client etaient actives (`allow_client_auth`) para guardar el plan de implementación
et les regles CSP en fase.

## Etapa 1 - Build et lint du portail

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

- `build/checksums.sha256` - manifiesto SHA256 para `sha256sum -c`.
- `build/release.json` - los metadatos (`tag`, `generated_at`, `source`) se corrigen en cada CAR/manifestado.

Archivez ces fichiers avec le resume CAR para que los revisores puedan comparar los artefactos
vista previa sin reconstruir.

## Etapa 2 - Empaquetar los activos estadísticos

Ejecute el packer CAR con el repertorio de salida Docusaurus. El ejemplo ci-dessous ecrit
les artefactos sous `artifacts/devportal/`.

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

El currículum JSON captura las cuentas de fragmentos, los resúmenes y las sugerencias de planificación de prueba.
que `manifest build` y los paneles de CI se reutilizan más tarde.

## Etapa 2b - Empaquetar los compañeros OpenAPI et SBOMDOCS-7 requiere publicar el sitio del portal, la instantánea OpenAPI y las cargas útiles SBOM
como los manifiestos distintivos para que las puertas de enlace puedan agregar los encabezados
`Sora-Proof`/`Sora-Content-CID` para cada artefacto. El ayudante de liberación
(`scripts/sorafs-pin-release.sh`) empaqueta deja le repertorio OpenAPI
(`static/openapi/`) y los SBOM emiten a través de `syft` en los CARs separa
`openapi.*`/`*-sbom.*` y registre los metadones dans
`artifacts/sorafs/portal.additional_assets.json`. En flujo manuel,
Repita las etapas 2-4 para cada carga útil con sus propios prefijos y etiquetas de metadatos.
(por ejemplo `--car-out "$OUT"/openapi.car` más
`--metadata alias_label=docs.sora.link/openapi`). Enregistrez chaque paire manifeste/alias
en Torii (sitio, OpenAPI, portal SBOM, SBOM OpenAPI) antes de basar el DNS para que
la gateway sirve des preuves agrafees pour tous les artefactos publies.

## Etapa 3 - Construir el manifiesto

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

Ajuste las banderas de pin-policy según su ventana de liberación (por ejemplo, `--pin-storage-class
hot` pour les canaries). La variante JSON es una opción más práctica para la revisión del código.

## Etapa 4 - Firmante con Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```El paquete registra el resumen del manifiesto, los resúmenes de fragmentos y un hash BLAKE3 del token.
OIDC sin persistir en JWT. Gardez le bundle et la firma detachee; las promociones de
La producción puede reutilizar los memes artefactos en lugar de volver a firmar. Las ejecuciones locales
Es posible reemplazar el proveedor de banderas por `--identity-token-env` (o definir `SIGSTORE_ID_TOKEN`).
) cuando un ayudante externo OIDC libera el token.

## Etapa 5 - Registro Soumettre au pin

Soumettez le manifeste signe (et le plan de chunks) a Torii. Demandez toujours un currículum para
que el entrante/alias resulte auditable.

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

Después de un lanzamiento de vista previa o de canary (`docs-preview.sora`), repita la misión
con un alias único para que QA pueda verificar el contenido antes de la producción de la promoción.

La encuadernación de alias requiere tres campeones: `--alias-namespace`, `--alias-name` y `--alias-proof`.
Gobernanza produce el paquete de prueba (base64 o bytes Norito) cuando la demanda de alias está
aprobado; stockez-le dans les secrets CI y exponez-le comme fichier avant d'invoquer
`manifest submit`. Laissez les flags d'alias vides quand vous voulez seulement pinner
El manifiesto sin tocar el DNS.

## Etapa 5b - Generar una propuesta de gobernanzaChaque manifeste doit voyager avec una proposition prete pour Parliament afin que tout citoyen
Sora puisse introduire le changement sans emprunter des credenciales privilegies. Después de les
etapes enviar/firmar, lancez:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captura la instrucción canónica `RegisterPinManifest`,
el resumen de fragmentos, la política y el índice de alias. Joignez-le au ticket de Governance ou au
Portal del Parlamento para que los delegados puedan comparar la carga útil sin reconstruirla.
Como la comando ne touche jamais la cle d'autorite Torii, tout citoyen peut rediger la
localización de la proposición.

## Etapa 6 - Verificador de pruebas y telemetría

Después de pin, ejecute las etapas de verificación determinantes:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Vigilancia `torii_sorafs_gateway_refusals_total` y
  `torii_sorafs_replication_sla_total{outcome="missed"}` para anomalías.
- Ejecute `npm run probe:portal` para ejecutar el proxy Try-It y registrar gravámenes
  contre le contenu fraichement pinne.
- Capturez la evidencia de monitoreo decrite dans
  [Publicación y seguimiento](./publishing-monitoring.md) pour que la gate d'observabilite
  DOCS-3c está satisfecho con las etapas de publicación. El ayudante acepta mantenimiento.
  plusieurs entrees `bindings` (sitio, OpenAPI, portal SBOM, SBOM OpenAPI) y aplicar
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` en el hotel cible a través de la opción de protección
  `hostname`. La invocación escrita de un currículum JSON único y el paquete de pruebas
  (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) en el repertorio
  de lanzamiento:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Etapa 6a - Planificador de certificados de puerta de enlace

Derivez le plan SAN/challenge TLS antes de creer los paquetes GAR para el equipo gateway et
les approbateurs DNS examinant la meme evidencia. El nuevo ayudante refleja las entradas DG-3
y enumera los hosts canónicos comodín, los SAN bastante-host, las etiquetas DNS-01 y
Los desafíos ACME recomiendan:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```Commita el JSON con el paquete de lanzamiento (o cargue con el ticket de cambio) para
que los operadores pueden recopilar los valores SAN en la configuración
`torii.sorafs_gateway.acme` de Torii y que los revisores GAR pudieron confirmar los
Mappings canonique/pretty sans re-ejecutor les derivations d'hotes. Ajoutez des arguments
`--name` complementos para cada sufijo promocionado en el lanzamiento del meme.

## Etapa 6b - Derivar les mapeos de hotes canónicos

Antes de modelar las cargas útiles GAR, registre el mapeo del momento determinante para cada alias.
`cargo xtask soradns-hosts` hashe chaque `--name` en son label canonique
(`<base32>.gw.sora.id`), introduzca los requisitos comodín (`*.gw.sora.id`) y derive el host bonito
(`<alias>.gw.sora.name`). Persistez la sortie dans les artefactos de liberación para que les
Los revisores DG-3 pudieron comparar el mapeo con la misión GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilice `--verify-host-patterns <file>` para reproducir vídeo cuando haya un GAR o un JSON de
puerta de enlace vinculante omet un des hosts requis. El ayudante acepta más archivos de verificación,
Esto facilita la pelusa de la plantilla GAR y el `portal.gateway.binding.json` agrafe dans
la invocación del meme:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Ingrese el JSON de currículum y el registro de verificación en el ticket de cambio DNS/gateway para que
Los auditores pueden confirmar los hosts canónicos, comodines y bonitos sin volver a ejecutar los archivos.
guiones. Vuelva a ejecutar el comando cuando los nuevos alias sean agregados al paquete para que les
actualiza la evidencia GAR heritent de la meme.

## Etapa 7 - Generar el descriptor de transferencia DNS

Los cortes de producción requieren un paquete de cambio auditable. Después de una cena
reussie (encuadernación de alias), le helper emet
`artifacts/sorafs/portal.dns-cutover.json`, capturante:- metadatos del enlace de alias (espacio de nombres/nombre/prueba, resumen del manifiesto, URL Torii,
  época soumis, autorite);
- contexto de lanzamiento (etiqueta, etiqueta de alias, manifiesto de caminos/CAR, plan de trozos, paquete Sigstore);
- punteros de verificación (sonda de comando, alias + punto final Torii); y
- campos opcionales de control de cambios (boleto de identificación, corte de ventana, operaciones de contacto,
  nombre de host/producción de zona);
- metadatos de promoción de ruta derivados del encabezado `Sora-Route-Binding`
  (host canonique/CID, caminos de encabezado + encuadernación, comandos de verificación), asegurando que la
  promoción GAR et les drills de fallback referencent la meme evidencia;
- les artefactos de plan de ruta generes (`gateway.route_plan.json`,
  plantillas de encabezados y opciones de reversión de encabezados) para los tickets de cambio y
  ganchos de pelusa CI puissent verifier que cada paquete DG-3 hace referencia a los planes de
  promoción/retroceso canónicos antes de la aprobación;
- Opciones de metadatos de invalidación de caché (punto final de purga, autenticación variable, carga útil JSON,
  y ejemplo de comando `curl`); y
- sugerencias de reversión pointant ver le descripteur precedente (etiqueta de liberación y resumen de manifiesto)
  Para que las entradas capturen un camino de retorno determinista.

Cuando se requiere la liberación de la purga de caché, genere un plan canónico con el descriptor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Inicie sesión en el paquete DG-3 `portal.cache_plan.json` para que los operadores se encuentren cerca de los hosts/rutas
 deterministas (y las sugerencias de autenticación correspondientes) lorsqu'ils emettent des requetes `PURGE`.
La sección de caché opcional del descriptor puede hacer referencia directamente a este archivo, gardant
Los revisores se alinean en los puntos finales y se purgan exactamente durante la transición.

Chaque paquet DG-3 también incluye una lista de verificación de promoción + reversión. Generez-le vía
`cargo xtask soradns-route-plan` para que los revisores puedan rastrear las etapas exactas
verificación previa, corte y reversión por alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Le `gateway.route_plan.json` emis capture hosts canoniques/pretty, rappels de health-check
por etapa, actualizaciones de enlace GAR, purgas de caché y acciones de reversión. Joignez-le aux
artefactos GAR/encuadernación/corte avant de soumettre le ticket de changement pour que Ops puisse
repetir y validar los memes etapas scriptes.

`scripts/generate-dns-cutover-plan.mjs` Alimente este descriptor y se ejecuta automáticamente después
`sorafs-pin-release.sh`. Para regenerar o personalizar manualmente:

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

Seleccione las opciones de metadatos a través de las variables de entorno antes del ejecutor del asistente de pin:| Variables | Pero |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID de ticket stocke dans le descripteur. |
| `DNS_CUTOVER_WINDOW` | Ventana de corte ISO8601 (ej: `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Producción de nombre de host + zona autorizada. |
| `DNS_OPS_CONTACT` | Alias ​​de guardia o contacto de escalada. |
| `DNS_CACHE_PURGE_ENDPOINT` | Registro de caché de purga de punto final en el descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contenido le token purga (predeterminado: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Cambie al descriptor precedente para revertir metadatos. |

Únase al JSON para revisar el cambio de DNS para que los aprobadores puedan verificar los resúmenes
manifiestos, enlaces de alias y comandos de investigación sin errores en los registros CI. Las banderas CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` y `--previous-dns-plan` proporcionan anulaciones de memes
quand le helper tourne fuera de CI.

## Etapa 8 - Emettre le squelette dezonefile du resolutor (opcional)Cuando la ventana de producción cutover está continuada, el guión de lanzamiento puede cancelarse
El esquema de archivos de zona SNS y la resolución automática de fragmentos. Pasar les
registra deseos DNS y metadatos a través de variables de entorno o opciones CLI; el ayudante
Llamada `scripts/sns_zonefile_skeleton.py` inmediatamente después de la generación del descriptor.
Introduzca al menos un valor A/AAAA/CNAME y el resumen GAR (BLAKE3-256 de la carga útil GAR signe).
Si la zona/nombre de host son connus e `--dns-zonefile-out` están omitidos, el ayudante escribe en
`artifacts/sns/zonefiles/<zone>/<hostname>.json` y remplit
`ops/soradns/static_zones.<hostname>.json` como resolución de fragmentos.| Variable/bandera | Pero |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Género del archivo de zona Chemin du squelette. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Camino al solucionador de fragmentos (predeterminado: `ops/soradns/static_zones.<hostname>.json` si omis). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | Géneros de registros auxiliares aplicados TTL (predeterminado: 600 segundos). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Direcciones IPv4 (entorno separado por vírgenes o bandera CLI repetible). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Direcciones IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Opción CNAME de destino. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pines SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entradas TXT adicionales (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Anular el cálculo del archivo de zona de la etiqueta de versión. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Fuerce la marca de tiempo `effective_at` (RFC3339) en lugar de abrir la ventana de corte. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Anule el registro literal de prueba en los metadatos. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Anule el registro CID en los metadatos. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Etat de frozen guardián (suave, duro, descongelación, seguimiento, emergencia). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Guardián de entradas de referencia/consejo para congelar. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Marca de tiempo RFC3339 para descongelación. || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Las notas congelan adicionales (env separe par virgules ou flag repetable). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Resumen BLAKE3-256 (hexadecimal) de la carga útil GAR signe. Requis quand des vinculaciones gateway sont presentes. |

El flujo de trabajo de GitHub Actions enciende estos valores después de los secretos del repositorio para cada producción de pin
Emet automáticamente los artefactos del archivo de zona. Configurez les secrets suivants (les valeurs peuvent
contiene listas separadas por virgules para campeones multivalores):

| Secreto | Pero |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | La producción de nombre de host/zona pasa au helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de guardia disponible en el descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registra IPv4/IPv6 como publicador. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Opción CNAME de destino. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pines SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entradas TXT adicionales. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Los metadatos congelan el registro en el esqueleto. |
| `DOCS_SORAFS_GAR_DIGEST` | Resumen BLAKE3 en hexadecimal de carga útil GAR signe. |

Si declina `.github/workflows/docs-portal-sorafs-pin.yml`, introduzca las entradas
`dns_change_ticket` e `dns_cutover_window` para que el descriptor/zonefile herede la
buena ventana. Laissez-les vides only pour des dry runs.

Tipo de invocación (en línea con el propietario del runbook SN-7):

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
```El asistente recupera automáticamente el billete de cambio como entrada TXT y hereda el debut de
La ventana de corte tiene la marca de tiempo `effective_at` o se anula. Para completar el flujo de trabajo, ver
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre la delegación DNS pública

El esqueleto de archivos de zona no define los registros autorizados de la
zona. Debes volver a configurar la delegación NS/DS de la zona principal chez
Su registrador o proveedor de DNS para que Internet público encuentre servidores
de nombres.

- Para las transferencias al Apex/TLD, utilice ALIAS/ANAME (según el proveedor) o
  Publiez des registrements A/AAAA pointant ver les IP anycast du gateway.
- Para los subdominios, publique un CNAME para derivar el Pretty Host.
  (`<fqdn>.gw.sora.name`).
- L'hote canonique (`<hash>.gw.sora.id`) descansa bajo el dominio du gateway et
  n'est pas publie dans tu zona pública.

### Plantilla de puerta de enlace de encabezados

El ayudante de implementación emet también `portal.gateway.headers.txt` y
`portal.gateway.binding.json`, dos artefactos que satisfacen la exigencia DG-3
para enlace de contenido de puerta de enlace:- `portal.gateway.headers.txt` contiene el bloque completo de encabezados HTTP (incluido
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS y el descriptor
  `Sora-Route-Binding`) que les gateways edge no dan ninguna respuesta.
- `portal.gateway.binding.json` registrar la información del meme en forma lisible máquina
  Para que los billetes de cambio y la automatización puedan compararse
  enlaces host/cid sans scraper la sortie shell.

Estos son generes automáticos a través de
`cargo xtask soradns-binding-template`
y capturen los alias, el resumen del manifiesto y el nombre de host de la puerta de enlace.
`sorafs-pin-release.sh`. Para regenerar o personalizar el bloque de encabezados, lanza:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pase `--csp-template`, `--permissions-template` o `--hsts-template` para anular
Las plantillas de encabezados por defecto cuando se implementan según las directivas.
suplementarios; Combinez-les avec les switchs `--no-*` existentes para supprimer
finalización de un encabezado.

Agregue el fragmento de encabezado a la solicitud de cambio CDN y actualice el documento JSON
au pipeline d'automatisation gateway pour que la promoción d'hote corresponde a
a la prueba de liberación.

El script de liberación se ejecuta automáticamente y el asistente de verificación para que les
Los billetes DG-3 incluyen pruebas recientes. Re-ejecutez-le manuellement si
Modifica el JSON de enlace a main:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```El comando decodifica la carga útil `Sora-Proof` y verifica los metadatos.
`Sora-Route-Binding` corresponde al CID del manifiesto + nombre de host y ecoue vite
si un encabezado deriva. Archivez la sortie console avec les autres artefactos de
despliegue cuando lanzas la orden fuera de CI para los revisores DG-3
aient la preuve que le vinculante a ete valide avant le cutover.

> **Integración del descriptor DNS:** `portal.dns-cutover.json` embarcar mantenimiento
> une sección `gateway_binding` pointant vers ces artefactos (chemins, content CID,
> estado de prueba y plantilla de encabezados literal) **y** una estrofa `route_plan`
> aquí referencia `gateway.route_plan.json` además de las plantillas de encabezados
> principal y reversión. Incluye estos bloques en cada billete DG-3 para que les
> los revisores pueden comparar los valores exactos `Sora-Name/Sora-Proof/CSP` et
> Confirmador de que los planes de promoción/reversión corresponden al paquete de evidencia.
> sin abrir el archivo build.

## Etapa 9 - Ejecutor de monitores de publicación

La hoja de ruta del artículo **DOCS-3c** requiere una evidencia para continuar con el portal, el proxy Pruébelo y
les vinculaciones puerta de enlace restent sains apres un lanzamiento. Ejecutar el monitor consolidado
juste apres les Etapes 7-8 et Branchez-le a vos probes programas:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` carga la configuración del archivo (ver
  `docs/portal/docs/devportal/publishing-monitoring.md` para el esquema) y
  ejecutar tres comprobaciones: sondas de rutas portail + validación CSP/Permisos-Política,
  proxy de sondas Pruébelo (opcional a través del punto final hijo `/metrics`), y el verificador
  de enlace de enlace (`cargo xtask soradns-verify-binding`) que exige mantenimiento la
  presencia y el valor asistente de Sora-Content-CID con los cheques alias/manifeste.
- El comando regresa a un valor distinto de cero cuando una sonda se hace eco de CI, trabajos cron o
  Los operadores de runbook pueden detener la liberación antes de promover el alias.
- Passer `--json-out` escribe un currículum JSON único con el estatuto par cible; `--evidence-dir`
  emet `summary.json`, `portal.json`, `tryit.json`, `binding.json` y `checksums.sha256`
  Para que los revisores gobiernen puedan comparar los resultados sin relanzar los monitores.
  Archivar este repertorio en `artifacts/sorafs/<tag>/monitoring/` con el paquete Sigstore
  y el descriptor DNS.
- Incluye la salida del monitor, la exportación Grafana (`dashboards/grafana/docs_portal.json`),
  y el ID de taladro Alertmanager en la liberación del ticket para que le salga el SLO DOCS-3c
  auditable más tardío. El manual de seguimiento de la publicación dedie vit a
  `docs/portal/docs/devportal/publishing-monitoring.md`.Las sondas del portal requieren HTTPS y rechazan las URL base `http://` o si
`allowInsecureHttp` está definido en la configuración del monitor; gardez les cibles producción/puesta en escena
en TLS y no activa la anulación para las vistas previas locales.

Automatiza el monitor a través de `npm run monitor:publishing` en Buildkite/cron una vez
le portail en ligne. La meme commande, pointee sur les URL production, alimente les checks
sante continúa entre lanzamientos.

## Automatización con `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsula las etapas 2-6. Yo:

1. archive `build/` en un tarball deterministe,
2. ejecute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   y `proof verify`,
3. Ejecute la opción `manifest submit` (incluye el enlace de alias) cuando
   Las credenciales Torii son presentes y
4. Escribir `artifacts/sorafs/portal.pin.report.json`, el opcional
  `portal.pin.proposal.json`, el descriptor DNS (después del envío),
  y el paquete de enlace de enlace (`portal.gateway.binding.json` más el bloque de encabezados)
  Para que los equipos de gobernanza, redes y operaciones puedan comparar la evidencia.
  sans fouiller les logs CI.

Definiciones `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` y (opcional)
`PIN_ALIAS_PROOF_PATH` antes de iniciar el script. Utilice `--skip-submit` para secar
corre; El flujo de trabajo de GitHub se inicia mediante la entrada `perform_submit`.

## Etapa 8: publique las especificaciones OpenAPI y paquetes SBOMDOCS-7 requiere que la construcción del portal, la especificación OpenAPI y los artefactos SBOM atravesados
El meme pipeline determina. Los ayudantes existentes couvrent les trois:

1. **Regenerar y firmar la especificación.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Fournissez un label release via `--version=<label>` quand vous voulez preserver un snapshot
   histórico (por ejemplo `2025-q3`). El ayudante escribe la instantánea en
   `static/openapi/versions/<label>/torii.json`, el espejo dentro
   `versions/current`, y registre los metadatos (SHA-256, estatuto del manifiesto, marca de tiempo
   mis a day) en `static/openapi/versions.json`. Le portail developmentpeur iluminado índice cet
   Para que los paneles Swagger/RapiDoc presenten un selector de versión y muestren
   digest/la firma asociada en línea. Omettre `--version` conservar las etiquetas de liberación
   precedente et ne rafraichit que les pointeurs `current` + `latest`.

   Le manifeste capture les digests SHA-256/BLAKE3 pour que la gateway puisse agrafer des
   encabezados `Sora-Proof` para `/reference/torii-swagger`.

2. **Emettre les SBOMs CycloneDX.** El lanzamiento de la tubería asiste a deja des SBOMs syft
   según `docs/source/sorafs_release_pipeline_plan.md`. Gardez la salida
   pres des artefactos de construcción:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaquetar cada carga útil en CAR.**

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
   ```Suivez les memes etapas `manifest build` / `manifest sign` que le site principal,
   ajusta los alias del artefacto (por ejemplo, `docs-openapi.sora` para la especificación y
   `docs-sbom.sora` para el paquete SBOM firmado). Garder des alias distingue mantenimiento
   SoraDNS, GAR y la reversión de tickets limitan exactamente la carga útil.

4. **Enviar y vincular.** Reutilice la autoridad existente y el paquete Sigstore, pero registrez
   le tuple d'alias dans le checklist release pour que les auditeurs puissent suivre quel
   nom Sora mappe vers quel digest manifeste.

Archiver les manifestes spec/SBOM avec le build portail asegura que cada vez que se libere el ticket
contiene el conjunto completo de artefactos sin volver a ejecutar el empaquetador.

### Ayudante de automatización (CI/script de paquete)

`./ci/package_docs_portal_sorafs.sh` codifica las etapas 1-8 para la hoja de ruta del artículo
**DOCS-7** es ejecutable con un solo comando. El ayudante:- ejecutar la preparación requerida por el portal (`npm ci`, sincronización OpenAPI/norito, prueba los widgets);
- emet les CARs et paires manifeste du portail, OpenAPI et SBOM via `sorafs_cli`;
- ejecute la opción `sorafs_cli proof verify` (`--proof`) y la firma Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- deponer todos los artefactos sous `artifacts/devportal/sorafs/<timestamp>/` et
  escriba `package_summary.json` para que CI/outillage release pueda introducir el paquete; y
- rafraichit `artifacts/devportal/sorafs/latest` para el puntero hacia la última ejecución.

Ejemplo (canalización completa con Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Banderas utiles:- `--out <dir>`: anula la raíz de los artefactos (la marca de tiempo predeterminada conserva la marca de tiempo de los expedientes).
- `--skip-build` - reutilizar un `docs/portal/build` existente (utilice quand CI no puede
  reconstruir a causa de espejos fuera de línea).
- `--skip-sync-openapi` - ignorar `npm run sync-openapi` cuando `cargo xtask openapi`
  No puedo unirme a crates.io.
- `--skip-sbom` - Evite llamar a `syft` cuando el binario no está instalado (el registro de script
  un anuncio a la place).
- `--proof` - Ejecute `sorafs_cli proof verify` para cada par de CAR/manifestado. Las cargas útiles
  Los archivos múltiples exigen además el soporte del plan de fragmentos en la CLI, sin dejar esta bandera.
  Desactive si encuentra errores `plan chunk count` y verifique manualmente
  une fois le gate aguas arriba livre.
- `--sign` - invocar `sorafs_cli manifest sign`. Fournissez un token vía
  `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) o libere el CLI para recuperarlo a través de
  `--sigstore-provider/--sigstore-audience`.Para la producción de artefactos, utilice `docs/portal/scripts/sorafs-pin-release.sh`.
Il empaquete maintenant le portail, OpenAPI et SBOM, signe chaque manifeste, et registre
Los activos de metadatos suplementarios en `portal.additional_assets.json`. El ayudante
comprend les memes potenciómetros opcionales que le packager CI plus les nouveaux switchs
`--openapi-*`, `--portal-sbom-*` y `--openapi-sbom-*` para asignar tuplas de alias
por artefacto, anular la fuente SBOM a través de `--openapi-sbom-source`, saltear ciertos
cargas útiles (`--skip-openapi`/`--skip-sbom`), y puntero frente a un binario `syft` no predeterminado
con `--syft-bin`.

Le script affiche cada comando ejecutar; copiar el registro en la liberación del billete
con `package_summary.json` para que los revisores puedan comparar los resúmenes de CAR,
Los metadatos del plan y los hashes del paquete Sigstore sin realizar una salida de shell ad hoc.

## Etapa 9 - Pasarela de verificación + SoraDNS

Antes de anunciar una transición, prouvez que le nouvel alias resol via SoraDNS y que les gateways
agraffent des pruebas frescas:

1. **Ejecutor le gate probe.** Ejercicio `ci/check_sorafs_gateway_probe.sh`
   `cargo xtask sorafs-gateway-probe` demostración de accesorios en el interior
   `fixtures/sorafs_gateway/probe_demo/`. Para los carretes de implementación, pointez le probe
   Ver el nombre de host cible:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   La sonda decodifica `Sora-Name`, `Sora-Proof` y `Sora-Proof-Status` según
   `docs/source/sorafs_alias_policy.md` et echoue quand le digest manifeste,
   Los TTL o los enlaces GAR derivados.Para controles puntuales ligeros (por ejemplo, cuando sólo el paquete de encuadernación
   cambiado), ejecute `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   El asistente valida el paquete de enlaces capturado y es útil para su lanzamiento.
   boletos que solo necesitan confirmación vinculante en lugar de un simulacro de sonda completa.

2. **Capturer la evidencia de simulacro.** Para el operador de simulacros o los ensayos en seco PagerDuty,
   envuelve la sonda con `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   lanzamiento del portal de desarrollo -- ...`. Le wrapper stocke encabezados/troncos sous
   `artifacts/sorafs_gateway_probe/<stamp>/`, conoció a un diario `ops/drill-log.md`, y
   (opcional) suelte los ganchos rollback o las cargas útiles PagerDuty. Definiciones
   `--host docs.sora` para validar el sistema SoraDNS en lugar de una IP codificada.3. **Verificador de enlaces DNS.** Cuando se publica el alias de prueba, regístrelo
   le fichier GAR reference par le probe (`--gar`) et joignez-le a l'evidence release.
   Los propietarios resolver pueden recuperar la entrada de memes a través de `tools/soradns-resolver` para
   s'assurer que les entrees en cache honorent le nouveau manifeste. Antes de unir el JSON,
   ejecutarz
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Para validar fuera de línea el host de mapeo determinado, el manifiesto de metadatos y las etiquetas
   telemetría. El ayudante puede editar un currículum `--json-out` con la firma GAR para que les
   Los revisores no encuentran evidencia verificable sin ouvrir le binaire.
  Quand vous redigez un nouveau GAR, preferez
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (revenez a `--manifest-cid <cid>` únicamente cuando el archivo manifiesto no está disponible
  disponibles). El asistente deriva el mantenimiento del CID **y** el resumen directo de BLAKE3
  Después del manifiesto JSON, cupo de espacios, deduplicación de banderas `--telemetry-label`,
  Pruebe las etiquetas y agregue las plantillas CSP/HSTS/Permissions-Policy por defecto
  Escribir el JSON para que la carga útil reste determinista meme si los operadores
  capturent des etiquetas después de conchas diferentes.

4. **Alias ​​de vigilancia de métricas.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   et `torii_sorafs_gateway_refusals_total{profile="docs"}` a l'ecran colgante le probe;
   Esta serie está gráfica en `dashboards/grafana/docs_portal.json`.

## Etapa 10 - Monitoreo y paquete de evidencia- **Paneles.** Exportez `dashboards/grafana/docs_portal.json` (portal de SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (puerta de enlace de latencia +
  prueba de salud), y `dashboards/grafana/sorafs_fetch_observability.json`
  (sante orquestador) pour cada liberación. Joignez les exporta JSON al liberar el ticket
  Para que los revisores puedan aumentar las solicitudes Prometheus.
- **Sonda de archivos.** Conservar `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex
  Nuestro cubo de pruebas. Incluya la sonda de currículum, los encabezados y la carga útil PagerDuty
  capturar la telemetría del script.
- **Bundle de release.** Stockez les resumes CAR du portail/SBOM/OpenAPI, les bundles manifestes,
  firmas Sigstore, `portal.pin.report.json`, sonda de registros Try-It e informes de verificación de enlaces
  bajo una marca de tiempo del expediente (por ejemplo, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Registro de perforación.** Quand les probes font partie d'un drill, laissez
  `scripts/telemetry/run_sorafs_gateway_probe.sh` agregar entradas a `ops/drill-log.md`
  Para que la evidencia meme satisfaga el requisito caos SNNet-5.
- **Liens de ticket.** Referencia a los ID del panel Grafana o a las exportaciones PNG adjuntas al archivo
  ticket de changement, avec le chemin du rapport probe, pour que les reviewers puissent
  Recupere los SLO sin acceso al shell.

## Etapa 11 - Cuadro de indicadores de pruebas y fuentes múltiples de búsqueda de simulacros

Publicado en SoraFS requiere mantener una búsqueda de evidencia de múltiples fuentes (DOCS-7/SF-6)
avec les preuves DNS/gateway ci-dessus. Después del pin du manifeste:1. **Lancer `sorafs_fetch` contre le manifeste live.** Utilisez les artefactos plan/manifeste
   productos en las etapas 2-3 con credenciales de puerta de enlace emitidas para cada proveedor.
   Persistez toutes les sorties pour que les auditeurs puissent rejouer el trace de decision
   orquestador:

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

   - Recuperez d'abord les referencias de proveedores de anuncios según el manifiesto (por ejemplo
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     et passez-les via `--provider-advert name=path` para que el marcador evalúe los archivos
     fenetres de capacite de facon deterministe. utilizar
     `--allow-implicit-provider-metadata` **único** cuando disfruta de los accesorios en CI;
     La producción de taladros debe citar los anuncios publicitarios que acompañan al pin.
   - Cuando el manifiesto haga referencia a las regiones adicionales, repita el comando con las
     Los proveedores de tuplas correspondientes para cada caché/alias son una asociación de búsqueda de artefactos.

2. **Archiver les sorties.** Conservaz `scoreboard.json`,
   `providers.ndjson`, `fetch.json` y `chunk_receipts.ndjson` bajo el expediente de pruebas
   du liberación. Estos archivos capturan la ponderación de pares, el presupuesto de reintento, la latencia EWMA
   y los recibos por trozos que le paquete de gobernanza deben retenerse para SF-7.3. **Abrir la telemetría cada día.** Importar las salidas a buscar en el tablero
   **SoraFS Obtención de observabilidad** (`dashboards/grafana/sorafs_fetch_observability.json`),
   Vigile `torii_sorafs_fetch_duration_ms`/`_failures_total` y los paneles de rango del proveedor.
   para las anomalías. Liez les snapshots Grafana au ticket release avec le chemin marcador.

4. **Probador de las reglas de alerta.** Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   Para validar el paquete de alertas Prometheus antes de cerrar la liberación. Joignez la sortie
   Promtool en el ticket para que los revisores DOCS-7 confirmen que las alertas de parada y
   Armés descansados de proveedores lentos.

5. **Brancher en CI.** El flujo de trabajo de pin portail guarda una etapa `sorafs_fetch` detrás
   la entrada `perform_fetch_probe`; activez-le pour les run puesta en escena/producción afin que
   l'evidence fetch soit produite avec le bundle manifeste sans intervention manuelle.
   Los taladros ubicados pueden reutilizar el meme script y exportar los tokens gateway y
   y define `PIN_FETCH_PROVIDERS` en la lista de proveedores separados por virgules.

## Promoción, observabilidad y reversión1. **Promoción:** gardez des alias distintivos para puesta en escena y producción. Promouvez es
   reejecutor `manifest submit` con el meme manifiesto/paquete, y cambiado
   `--alias-namespace/--alias-name` para puntero frente al alias de producción. cela evite
   de reconstruir o renovar el firmante una vez que QA apruebe el pin staging.
2. **Monitoreo:** importar el registro de pin del tablero
   (`docs/source/grafana_sorafs_pin_registry.json`) además de las sondas específicas del portal
   (ver `docs/portal/docs/devportal/observability.md`). Alertez sur drift de checksums,
   Las sondas hacen eco o las fotos de prueba de reintento.
3. **Revertir:** para regresar a casa, resoumettez le manifeste precedente (ou retirez
   l'alias courant) con `sorafs_cli manifest submit --alias ... --retire`.
   Gardez toujours le último paquete connu comme bon et le resume CAR pour que les
   Pruebas de reversión puissent etre recrees si les logs CI tournent.

## Modelo de flujo de trabajo CI

Al menos, su tubería debe hacer lo siguiente:

1. Build + lint (`npm ci`, `npm run build`, generación de sumas de comprobación).
2. Empaqueter (`car pack`) et calculer les manifestes.
3. Firmante a través del token OIDC del trabajo (`manifest sign`).
4. Cargar los artefactos (CAR, manifiesto, paquete, plan, currículums) para auditoría.
5. Registro Soumettre au pin:
   - Solicitudes de extracción -> `docs-preview.sora`.
   - Etiquetas / sucursales protegidas -> producción de alias de promoción.
6. Ejecutar las sondas + puertas de verificación prueba antes del determinante.`.github/workflows/docs-portal-sorafs-pin.yml` Conecte todas estas etapas para liberar manualmente.
El flujo de trabajo:

- construir/probar el portal,
- empaquete le build vía `scripts/sorafs-pin-release.sh`,
- firmar/verificar el manifiesto del paquete a través de GitHub OIDC,
- cargar el CAR/manifeste/bundle/plan/resumes como artefactos, etc.
- (opcional) soumet le manifeste + alias vinculante cuando los secretos son presentes.

Configure los secretos/variables siguientes antes de soltar el trabajo:

| Nombre | Pero |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii que expone `/v2/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificador de época registrado con los envíos. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autorité de firma para la misión manifiesta. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | El alias de tupla se encuentra en el manifiesto cuando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Codificación de alias de prueba de paquete en base64 (opcional; omitir el enlace de alias). |
| `DOCS_ANALYTICS_*` | Los análisis/sondeos de puntos finales existentes reutilizan otros flujos de trabajo. |

Declenchez le flujo de trabajo a través de las acciones de UI:

1. Fournissez `alias_label` (ej: `docs.sora.link`), un `proposal_alias` opcional,
   Y anule la opción `release_tag`.
2. Libere `perform_submit` sin marcar para generar artefactos sin contacto Torii
   (Utilícelo para ensayos) o actívelo para publicarlo directamente en el alias de configuración.`docs/source/sorafs_ci_templates.md` documente encore les helpers CI generiques pour
Los proyectos fuera del repositorio, pero el portal de flujo de trabajo debe ser la vía preferida.
pour les releases quotidiennes.

## Lista de verificación

- [] `npm run build`, `npm run test:*` y `npm run check:links` son verticales.
- [] `build/checksums.sha256` e `build/release.json` capturan los artefactos.
- [ ] CAR, plan, manifeste, et resume generes sous `artifacts/`.
- [ ] Paquete Sigstore + firma separada stockes avec les logs.
- [ ] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
      capturas quand des presentaciones en lugar.
- [ ] `portal.pin.report.json` (y opcional `portal.pin.proposal.json`)
      archive avec les artefactos CAR/manifeste.
- [] Registros de archivos `proof verify` y `manifest verify-signature`.
- [ ] Dashboards Grafana mis a jour + sondas Try-It reussis.
- [] Reversión de notas (ID manifiesto precedente + alias de resumen) juntas au
      liberación de boletos.