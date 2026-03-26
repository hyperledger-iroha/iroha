---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota Fuente canónica
Esta pagina refleja `docs/source/sns/address_display_guidelines.md` y ahora sirve
como la copia canónica del portal. El archivo fuente se mantiene para PRs de
traducción.
:::

Las billeteras, exploradores y ejemplos de SDK deben tratar las direcciones de
cuenta como cargas útiles inmutables. El ejemplo de billetera minorista de Android en
`examples/android/retail-wallet` ahora demuestra el patrón de UX requerido:- **Dos objetivos de copia.** Envia dos botones de copia explícitos: I105
  (preferido) y la forma comprimida solo Sora (`sora...`, segunda mejor opción).
  I105 siempre es seguro para compartir externamente y alimenta la carga útil del QR. La variante
  comprimida debe incluir una advertencia en línea porque solo funciona dentro
  de aplicaciones con soporte de Sora. El ejemplo de billetera minorista de Android
  conecta ambos botones Material y sus tooltips en
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, y la
  demo iOS SwiftUI refleja el mismo UX vía `AddressPreviewCard` dentro de
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monoespacio, texto seleccionable.** Renderiza ambas cadenas con una fuente
  monospace y `textIsSelectable="true"` para que los usuarios puedan inspeccionar
  valores sin invocar un IME. Evita campos editables: los IME pueden reescribir
  kana o inyectar puntos de código de ancho cero.
- **Pistas del dominio por defecto implícito.** Cuando el selector apunta al
  dominio implícito `default`, muestra un caption recordando a los operadores
  que no se requiere sufijo. Los exploradores también deben resaltar la etiqueta.
  de dominio canonica cuando el selector codifica un digest.
- **QR I105.** Los códigos QR deben codificar la cadena I105. si la generacion
  del QR falla, muestra un error explícito en lugar de una imagen en blanco.
- **Mensajería del portapapeles.** Después de copiar la forma comprimida, emiteun brindis o snackbar recordando a los usuarios que es solo Sora y propensa a la
  distorsión por IME.

Seguir estas pautas evita la corrupción Unicode/IME y satisface los criterios de
aceptación del roadmap ADDR-6 para UX de billeteras/exploradores.

## Capturas de pantalla de referencia

Usa las siguientes referencias durante la revisión de localización para asegurar
que las etiquetas de botones, información sobre herramientas y advertencias se mantienen alineadas
entre plataformas:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de doble copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS de doble copia](/img/sns/address_copy_ios.svg)

## Ayudantes de SDK

Cada SDK expone un asistente de conveniencia que devuelve las formas I105 y
comprimida junto con la cadena de advertencia para que las capas UI se mantengan
consistentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)` devuelve la cadena de advertencia
  comprimida y la agrega a `warnings` cuando los llamadores suministran un
  literal `sora...`, de modo que los exploradores/dashboards de billeteras puedan
  mostrar el aviso solo Sora durante los flujos de pegado/validacion en lugar de
  hacerlo solo cuando genere la forma comprimida por su cuenta.
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilice estos ayudantes en lugar de reimplementar la lógica de codificación en las capas UI.
El ayudante de JavaScript también expone un payload `selector` en `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`) para que las UI puedan indicar si
un selector es Local-12 o respaldado por registro sin volver a analizar la carga útil
en bruto.

## Demostración de instrumentación del explorador



Los exploradores deben reflejar el trabajo de telemetría y accesibilidad de la
billetera:- Aplica `data-copy-mode="i105|qr"` a los botones de copia para que
  los front-ends pueden emitir contadores de uso junto con la métrica Torii
  `torii_address_format_total`. El componente demo anterior despacha un evento
  `iroha:address-copy` con `{mode,timestamp}`: conecta esto a tu tubería de
  analitica/telemetria (por ejemplo, envia a Segment o a un colector respaldado
  por NORITO) para que los tableros puedan correlacionar el uso de formatos de
  dirección del servidor con los modos de copia del cliente. También refleja los
  contadores de dominio de Torii (`torii_address_domain_total{domain_kind}`) en
  el mismo feed para que las revisiones de retiro de Local-12 puedan exportar
  una prueba de 30 días `domain_kind="local12"` directamente desde el tablero
  `address_ingest` de Grafana.
- Emparejar cada control con pistas `aria-label`/`aria-describedby` distintas que
  expliquen si un literal es seguro para compartir (I105) o solo Sora
  (comprimido). Incluye el título de dominio implícito en la descripción para
  que la tecnologia asistiva muestra el mismo contexto visual.
- Expone una región viva (por ejemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia y advertencias, igualando el comportamiento de
  VoiceOver/TalkBack ya está conectado en los ejemplos Swift/Android.Esta instrumentación satisface ADDR-6b al demostrar que los operadores pueden
observar tanto la ingestión Torii como los modos de copia del cliente antes de
que se deshabiliten los selectores locales.

## Kit de herramientas de migración Local -> Global

Usa el [toolkit Local -> Global](local-to-global-toolkit.md) para automatizar la
revisión y conversión de selectores Local heredados. El ayudante emite tanto el
reporte de auditoria JSON como la lista convertida I105/comprimida que los
Los operadores adjuntan a los tickets de preparación, mientras que el runbook.
Acompañante enlaza los paneles de Grafana y las reglas de Alertmanager que
controle el corte en modo estricto.

## Referencia rápida del diseño binario (ADDR-1a)

Cuando los SDKs expongan herramientas avanzadas de direcciones (inspectores, pistas de
validacion, constructores de manifest), dirijan a los desarrolladores al formato
wire canonico capturado en `docs/account_structure.md`. El diseño siempre es
`header · selector · controller`, donde los bits del encabezado son:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```- `addr_version = 0` (bits 7-5) hoy; valores no cero estan reservados y deben
  lanzar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue entre controladores simples (`0`) y multifirma (`1`).
- `norm_version = 1` codifica las reglas de selector Norm v1. Normas futuras
  reutilizarán el mismo campo de 2 bits.
- `ext_flag` siempre es `0`; bits activos indican extensiones de carga útil no
  soportadas.

El selector sigue inmediatamente al encabezado:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Las UI y SDK deben estar listas para mostrar el tipo de selector:

- `0x00` = dominio por defecto implícito (sin payload).
- `0x01` = resumen local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Ejemplos hex canónicos que las herramientas de billetera pueden enlazar o
insertar en docs/tests:

| Tipo de selector | Hex canónico |
|---------------|---------------|
| Implícito por defecto | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumen local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro mundial (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Consulta `docs/source/references/address_norm_v1.md` para la tabla completa de
selector/estado y `docs/account_structure.md` para el diagrama de bytes completo.

## Forzar formas canónicas

Los operadores que convierten codificaciones locales heredadas a I105 canonico o
Las cadenas comprimidas deben seguir el flujo CLI documentado en ADDR-5:1. `iroha tools address inspect` ahora emite un resumen JSON estructurado con I105,
   comprimido y payloads hex canónicos. El resumen también incluye un objeto.
   `domain` con campos `kind`/`warning` y refleja cualquier dominio proporcionado
   vía el campo `input_domain`. Cuando `kind` es `local12`, el CLI imprime una
   advertencia a stderr y el resumen JSON refleja la misma guía para que los
   Los pipelines de CI y los SDK pueden mostrarla. Pasa `legacy  suffix` cuando
   quieras que la codificación convertida se reproduzca como `<i105>@<domain>`.
2. Los SDK pueden mostrar la misma advertencia/resumen a través del ayudante de
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
  proporciones explícitamente `networkPrefix`, por lo que los resúmenes para
  redes no default no se re-renderizan silenciosamente con el prefijo por
  defecto.3. Convierte el payload canonico reutilizando los campos `i105.value` o
   `i105` del resumen (o solicita otra codificación vía `--format`). estas
   cadenas ya son seguras para compartir externamente.
4. Actualiza manifiestos, registros y documentos de cara al cliente con la
   forma canónica y notifica a las contrapartes que los selectores locales serán
   rechazados una vez completado el cutover.
5. Para conjuntos de datos masivos, ejecuta
   `iroha tools address audit --input addresses.txt --network-prefix 753`. El comando
   lee literales separados por nueva linea (comentarios que empiezan con `#` se
   ignoran, y `--input -` o ningun flag usa STDIN), emite un informe JSON con
   resúmenes canónicos/I105/comprimidos para cada entrada, y cuenta errores de
   parse y advertencias de dominio Local. Usa `--allow-errors` al auditar dumps
   heredados que contienen filas de basura, y bloquea la automatización con
   `strict CI post-check` cuando los operadores están listos para bloquear
   selectores Local en CI.
6. Cuando necesites una reescritura linea a linea, usa
  Para hojas de calculo de remediacion de selectores Local, usa
  para exportar un CSV `input,status,format,...` que resalta codificaciones
  canónicas, advertencias y fallos de parse en una sola pasada.
   El ayudante omite filas no Local por defecto, convierte cada entrada restante
   a la codificación solicitada (I105/comprimido/hex/JSON), y preserva el dominiooriginal cuando se usa `legacy  suffix`. Combinalo con `--allow-errors` para
   seguir escaneando incluso cuando un dump contiene literales mal formados.
7. La automatización de CI/lint puede ejecutar `ci/check_address_normalize.sh`,
   que extrae los selectores Local de `fixtures/account/address_vectors.json`,
   los convierte vía `iroha tools address normalize`, y vuelve a ejecutar
   `iroha tools address audit` para demostrar que los lanzamientos ya no
   emiten resúmenes Locales.`torii_address_local8_total{endpoint}` junto con
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, y el tablero Grafana
`dashboards/grafana/address_ingest.json` proporciona la señal de cumplimiento:
cuando los tableros de producción muestran cero envíos Local legitimos y cero
colisiones Local-12 durante 30 días consecutivos, Torii cambiara el gate Local-8
para fallar en duro en mainnet, seguido por Local-12 cuando los dominios globales
cuenten con entradas de registro correspondientes. Considere la salida del CLI
como el aviso para operadores de este congelamiento: la misma cadena de
advertencia se usa en información sobre herramientas de SDK y automatización para mantener la paridad con
los criterios de salida del roadmap. Torii ahora usa por defecto
cuando diagnosticas regresiones. Sigue reflejando
`torii_address_domain_total{domain_kind}` y Grafana
(`dashboards/grafana/address_ingest.json`) para que el paquete de evidencia
ADDR-7 demuestre que `domain_kind="local12"` permanecio en cero durante la
ventana requerida de 30 días antes de que mainnet desactive los selectores
(`dashboards/alerts/address_ingest_rules.yml`) agrega tres barandillas:- `AddressLocal8Resurgence` pagina cuando un contexto reporta un incremento
  Fresco local-8. Deten los rollouts de modo estricto, localiza el SDK responsable
  en el tablero y, si es necesario, configure temporalmente
  el predeterminado (`true`).
- `AddressLocal12Collision` se dispara cuando dos etiquetas Local-12 hacen hash
  al mismo digerir. Pausa las promociones de manifest, ejecuta el kit de herramientas
  Local -> Global para auditar el mapeo de digests y coordinar con la gobernanza
  de Nexus antes de reemitir la entrada de registro o reactivar rollouts aguas
  abajo.
- `AddressInvalidRatioSlo` avisa cuando la proporción de invalidos en toda la
  flota (excluyendo rechazos Local-8/strict-mode) excede el SLO de 0.1% durante
  diez minutos. Usa `torii_address_invalid_total` para identificar el
  contexto/razon responsable y coordina con el equipo SDK propietario antes de
  reactivar el modo estricto.

### Fragmento para notas de lanzamiento (billetera y exploradora)

Incluye la siguiente viñeta en las notas de lanzamiento de billetera/explorador
al publicar el cutover:> **Direcciones:** Se agrega el ayudante `iroha tools address normalize`
> y se conecta en CI (`ci/check_address_normalize.sh`) para que las tuberías de
> billetera/explorador puedan convertir selectores Local heredados a formas
> canónicas I105/comprimidas antes de que Local-8/Local-12 se bloqueen en mainnet.
> Actualiza cualquier exportación personalizada para ejecutar el comando y
> adjunte la lista normalizada al paquete de evidencia de liberación.