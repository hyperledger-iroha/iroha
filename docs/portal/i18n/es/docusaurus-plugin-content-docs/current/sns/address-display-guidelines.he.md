---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f923e1c8308471aac313cbd011858261f7e42ebaf42c4948a4b6aa20ca8d8332
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: address-display-guidelines
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Fuente canonica
Esta pagina refleja `docs/source/sns/address_display_guidelines.md` y ahora sirve
como la copia canonica del portal. El archivo fuente se mantiene para PRs de
traduccion.
:::

Las billeteras, exploradores y ejemplos de SDK deben tratar las direcciones de
cuenta como payloads inmutables. El ejemplo de billetera retail de Android en
`examples/android/retail-wallet` ahora demuestra el patron de UX requerido:

- **Dos objetivos de copia.** Envia dos botones de copia explicitos: I105
  (preferido) y la forma comprimida solo Sora (`sora...`, segunda mejor opción).
  I105 siempre es seguro para compartir externamente y alimenta el payload del QR. La variante
  comprimida debe incluir una advertencia en linea porque solo funciona dentro
  de apps con soporte de Sora. El ejemplo de billetera retail de Android
  conecta ambos botones Material y sus tooltips en
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, y la
  demo iOS SwiftUI refleja el mismo UX via `AddressPreviewCard` dentro de
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texto seleccionable.** Renderiza ambas cadenas con una fuente
  monospace y `textIsSelectable="true"` para que los usuarios puedan inspeccionar
  valores sin invocar un IME. Evita campos editables: los IME pueden reescribir
  kana o inyectar puntos de codigo de ancho cero.
- **Pistas del dominio por defecto implicito.** Cuando el selector apunta al
  dominio implicito `default`, muestra un caption recordando a los operadores
  que no se requiere sufijo. Los exploradores tambien deben resaltar la etiqueta
  de dominio canonica cuando el selector codifica un digest.
- **QR I105.** Los codigos QR deben codificar la cadena I105. Si la generacion
  del QR falla, muestra un error explicito en lugar de una imagen en blanco.
- **Mensajeria del portapapeles.** Despues de copiar la forma comprimida, emite
  un toast o snackbar recordando a los usuarios que es solo Sora y propensa a la
  distorsion por IME.

Seguir estas pautas evita la corrupcion Unicode/IME y satisface los criterios de
aceptacion del roadmap ADDR-6 para UX de billeteras/exploradores.

## Capturas de pantalla de referencia

Usa las siguientes referencias durante revisiones de localizacion para asegurar
que las etiquetas de botones, tooltips y advertencias se mantengan alineadas
entre plataformas:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de doble copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS de doble copia](/img/sns/address_copy_ios.svg)

## Helpers de SDK

Cada SDK expone un helper de conveniencia que devuelve las formas I105 y
comprimida junto con la cadena de advertencia para que las capas UI se mantengan
consistentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` devuelve la cadena de advertencia
  comprimida y la agrega a `warnings` cuando los llamadores proporcionan un
  literal `sora...`, de modo que los exploradores/dashboards de billeteras puedan
  mostrar el aviso solo Sora durante los flujos de pegado/validacion en lugar de
  hacerlo solo cuando generan la forma comprimida por su cuenta.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Usa estos helpers en lugar de reimplementar la logica de encode en las capas UI.
El helper de JavaScript tambien expone un payload `selector` en `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`) para que las UIs puedan indicar si
un selector es Local-12 o respaldado por registro sin volver a parsear el payload
en bruto.

## Demo de instrumentacion del explorador

<ExplorerAddressCard />

Los exploradores deben reflejar el trabajo de telemetria y accesibilidad de la
billetera:

- Aplica `data-copy-mode="i105|qr"` a los botones de copia para que
  los front-ends puedan emitir contadores de uso junto con la metrica Torii
  `torii_address_format_total`. El componente demo anterior despacha un evento
  `iroha:address-copy` con `{mode,timestamp}`: conecta esto a tu pipeline de
  analitica/telemetria (por ejemplo, envia a Segment o a un colector respaldado
  por NORITO) para que los dashboards puedan correlacionar el uso de formatos de
  direccion del servidor con los modos de copia del cliente. Tambien refleja los
  contadores de dominio de Torii (`torii_address_domain_total{domain_kind}`) en
  el mismo feed para que las revisiones de retiro de Local-12 puedan exportar
  una prueba de 30 dias `domain_kind="local12"` directamente desde el tablero
  `address_ingest` de Grafana.
- Empareja cada control con pistas `aria-label`/`aria-describedby` distintas que
  expliquen si un literal es seguro para compartir (I105) o solo Sora
  (comprimido). Incluye el caption de dominio implicito en la descripcion para
  que la tecnologia asistiva muestre el mismo contexto visual.
- Expone una region viva (por ejemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia y advertencias, igualando el comportamiento de
  VoiceOver/TalkBack ya conectado en los ejemplos Swift/Android.

Esta instrumentacion satisface ADDR-6b al demostrar que los operadores pueden
observar tanto la ingestion Torii como los modos de copia del cliente antes de
que se deshabiliten los selectores Local.

## Toolkit de migracion Local -> Global

Usa el [toolkit Local -> Global](local-to-global-toolkit.md) para automatizar la
revision y conversion de selectores Local heredados. El helper emite tanto el
reporte de auditoria JSON como la lista convertida I105/comprimida que los
operadores adjuntan a los tickets de readiness, mientras que el runbook
acompanante enlaza los dashboards de Grafana y las reglas de Alertmanager que
controlan el cutover en modo estricto.

## Referencia rapida del layout binario (ADDR-1a)

Cuando los SDKs expongan tooling avanzado de direcciones (inspectores, pistas de
validacion, constructores de manifest), dirijan a los desarrolladores al formato
wire canonico capturado en `docs/account_structure.md`. El layout siempre es
`header · selector · controller`, donde los bits del header son:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoy; valores no cero estan reservados y deben
  lanzar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue entre controladores simples (`0`) y multisig (`1`).
- `norm_version = 1` codifica las reglas de selector Norm v1. Norms futuras
  reutilizaran el mismo campo de 2 bits.
- `ext_flag` siempre es `0`; bits activos indican extensiones de payload no
  soportadas.

El selector sigue inmediatamente al header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Las UIs y SDKs deben estar listas para mostrar el tipo de selector:

- `0x00` = dominio por defecto implicito (sin payload).
- `0x01` = digest local (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Ejemplos hex canonicos que las herramientas de billetera pueden enlazar o
insertar en docs/tests:

| Tipo de selector | Hex canonico |
|---------------|---------------|
| Implicito por defecto | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Consulta `docs/source/references/address_norm_v1.md` para la tabla completa de
selector/estado y `docs/account_structure.md` para el diagrama de bytes completo.

## Forzar formas canonicas

Los operadores que convierten codificaciones Local heredadas a I105 canonico o
cadenas comprimidas deben seguir el flujo CLI documentado en ADDR-5:

1. `iroha tools address inspect` ahora emite un resumen JSON estructurado con I105,
   comprimido y payloads hex canonicos. El resumen tambien incluye un objeto
   `domain` con campos `kind`/`warning` y refleja cualquier dominio proporcionado
   via el campo `input_domain`. Cuando `kind` es `local12`, el CLI imprime una
   advertencia a stderr y el resumen JSON refleja la misma guia para que los
   pipelines de CI y los SDKs puedan mostrarla. Pasa `legacy  suffix` cuando
   quieras que la codificacion convertida se reproduzca como `<i105>@<domain>`.
2. Los SDKs pueden mostrar la misma advertencia/resumen via el helper de
   JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  El helper conserva el prefijo I105 detectado del literal a menos que
  proporciones explicitamente `networkPrefix`, por lo que los resumenes para
  redes no default no se re-renderizan silenciosamente con el prefijo por
  defecto.

3. Convierte el payload canonico reutilizando los campos `i105.value` o
   `i105` del resumen (o solicita otra codificacion via `--format`). Estas
   cadenas ya son seguras para compartir externamente.
4. Actualiza manifiestos, registros y documentos de cara al cliente con la
   forma canonica y notifica a las contrapartes que los selectores Local seran
   rechazados una vez completado el cutover.
5. Para conjuntos de datos masivos, ejecuta
   `iroha tools address audit --input addresses.txt --network-prefix 753`. El comando
   lee literales separados por nueva linea (comentarios que empiezan con `#` se
   ignoran, y `--input -` o ningun flag usa STDIN), emite un reporte JSON con
   resumenes canonicos/I105/comprimidos para cada entrada, y cuenta errores de
   parse y advertencias de dominio Local. Usa `--allow-errors` al auditar dumps
   heredados que contienen filas basura, y bloquea la automatizacion con
   `strict CI post-check` cuando los operadores esten listos para bloquear
   selectores Local en CI.
6. Cuando necesites una reescritura linea a linea, usa
  Para hojas de calculo de remediacion de selectores Local, usa
  para exportar un CSV `input,status,format,...` que resalta codificaciones
  canonicas, advertencias y fallos de parse en una sola pasada.
   El helper omite filas no Local por defecto, convierte cada entrada restante
   a la codificacion solicitada (I105/comprimido/hex/JSON), y preserva el dominio
   original cuando se usa `legacy  suffix`. Combinalo con `--allow-errors` para
   seguir escaneando incluso cuando un dump contiene literales mal formados.
7. La automatizacion de CI/lint puede ejecutar `ci/check_address_normalize.sh`,
   que extrae los selectores Local de `fixtures/account/address_vectors.json`,
   los convierte via `iroha tools address normalize`, y vuelve a ejecutar
   `iroha tools address audit` para demostrar que los releases ya no
   emiten digests Local.

`torii_address_local8_total{endpoint}` junto con
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, y el tablero Grafana
`dashboards/grafana/address_ingest.json` proporcionan la senal de cumplimiento:
cuando los dashboards de produccion muestran cero envios Local legitimos y cero
colisiones Local-12 durante 30 dias consecutivos, Torii cambiara el gate Local-8
para fallar en duro en mainnet, seguido por Local-12 cuando los dominios globales
cuenten con entradas de registro correspondientes. Considera la salida del CLI
como el aviso para operadores de este congelamiento: la misma cadena de
advertencia se usa en tooltips de SDK y automatizacion para mantener paridad con
los criterios de salida del roadmap. Torii ahora usa por defecto
cuando diagnostiques regresiones. Sigue reflejando
`torii_address_domain_total{domain_kind}` en Grafana
(`dashboards/grafana/address_ingest.json`) para que el paquete de evidencia
ADDR-7 demuestre que `domain_kind="local12"` permanecio en cero durante la
ventana requerida de 30 dias antes de que mainnet deshabilite los selectores
(`dashboards/alerts/address_ingest_rules.yml`) agrega tres guardrails:

- `AddressLocal8Resurgence` pagina cuando un contexto reporta un incremento
  Local-8 fresco. Deten los rollouts de modo estricto, localiza el SDK responsable
  en el dashboard y, si es necesario, configura temporalmente
  el default (`true`).
- `AddressLocal12Collision` se dispara cuando dos etiquetas Local-12 hacen hash
  al mismo digest. Pausa las promociones de manifest, ejecuta el toolkit
  Local -> Global para auditar el mapeo de digests y coordina con la gobernanza
  de Nexus antes de reemitir la entrada de registro o reactivar rollouts aguas
  abajo.
- `AddressInvalidRatioSlo` avisa cuando la proporcion de invalidos en toda la
  flota (excluyendo rechazos Local-8/strict-mode) excede el SLO de 0.1% durante
  diez minutos. Usa `torii_address_invalid_total` para identificar el
  contexto/razon responsable y coordina con el equipo SDK propietario antes de
  reactivar el modo estricto.

### Fragmento para notas de lanzamiento (billetera y explorador)

Incluye el siguiente bullet en las notas de lanzamiento de billetera/explorador
al publicar el cutover:

> **Direcciones:** Se agrego el helper `iroha tools address normalize`
> y se conecto en CI (`ci/check_address_normalize.sh`) para que las pipelines de
> billetera/explorador puedan convertir selectores Local heredados a formas
> canonicas I105/comprimidas antes de que Local-8/Local-12 se bloqueen en mainnet.
> Actualiza cualquier exportacion personalizada para ejecutar el comando y
> adjunta la lista normalizada al bundle de evidencia de release.
