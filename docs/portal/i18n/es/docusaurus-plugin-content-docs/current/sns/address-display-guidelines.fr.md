---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota Fuente canónica
Esta página reflejada `docs/source/sns/address_display_guidelines.md` y sert
mantenimiento de copia canónica del portal. El archivo fuente restante para las relaciones públicas
de traducción.
:::

Los portafeuilles, exploradores y ejemplos de SDK deben tratar las direcciones
de cuenta como las cargas útiles immuables. El ejemplo de portefeuille retail Android
En `examples/android/retail-wallet` el reloj de mantenimiento del patrón UX requiere:- **Dos botones de copia.** Cuatro botones de copia explícitos: IH58
  (prefere) et la forme compressee Sora-only (`sora...`, segunda opción). IH58 está todavía en funcionamiento
  Asegúrese de compartir en forma externa y alimentaria la carga útil del QR. La variante compresée
  incluya un anuncio en línea porque no funciona en el
  Aplicaciones gratuitas a cargo de Sora. El ejemplo de Android rama con dos botones Material y
  información sobre herramientas en leurs
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, y la
  Demo iOS SwiftUI refleja el meme UX a través de `AddressPreviewCard` en
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monoespacio, texto seleccionable.** Mostrar las dos cadenas con una policía
  monospace et `textIsSelectable="true"` afin que les utilisateurs puissent
  Inspeccione los valores sin invocar un IME. Evitez les champs editables: les
  IME puede volver a escribir el kana o el inyector de puntos de código a cero más grande.
- **Indicaciones de dominio por defecto implícitas.** Quand le selecteur pointe sur
  El dominio implícito `default`, muestra una leyenda rappelante aux operadores.
  qu'aucun sufijo n'est requis. Les explorateurs doivent aussi mettre en avant
  La etiqueta de dominio canónico cuando el selector codifica un resumen.
- **QR IH58.** Los códigos QR no codifican la cadena IH58. Si la generación du
  QR echoue, muestra un error explícito en lugar de una imagen de vídeo.
- **Message presse-papiers.** Después de copiar la forma compresée, emettez unTostadas o snackbar rappelant aux utilisateurs qu'elle est Sora-only et sujette
  a la distorsión IME.

Suivre ces garde-fous evite la corrupción Unicode/IME y cumpla con los criterios
Aceptación de la hoja de ruta ADDR-6 para el usuario/explorador de UX.

## Capturas de referencia

Utilice las referencias siguientes de las revistas de localización para garantizar
que los libelles de botones, información sobre herramientas y anuncios se alinean entre
plataformas:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android doble copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS doble copia](/img/sns/address_copy_ios.svg)

## SDK de ayuda

Cada SDK expone un asistente de conveniencia que devuelve las formas IH58 y
Comprimir además la cadena de anuncios para que los sofás UI resten
coherentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)` devolver la cadena
  d'avertissement compressee et l'ajoute a `warnings` quand les apelantes
  Fournissent un literal `sora...`, para que les explorateurs/tableaux de bord de
  portefeuille puissent mostrar el aviso Sora-only colgante les flux de
  collage/validación plutot que seulement lorsqu'ils generent eux-memes la forme
  comprimir.
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilice estos ayudantes en lugar de reimplementar la lógica de codificación en los archivos
interfaz de usuario de sofás. El asistente JavaScript expone además una carga útil `selector` en
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) para las UI
Puede indicar si un seleccionador es Local-12 o agregar un registro sin
Reparser le payload brut.

## Demostración de instrumentación del explorador



Los exploradores no deben reproducir el trabajo de telemetría y accesibilidad
hecho para el portefeuille:- Apliques `data-copy-mode="ih58|compressed|qr"` con botones de copia después de que
  Los front-ends pueden configurar los ordenadores de uso en paralelo
  Métrica Torii `torii_address_format_total`. Le composant demo ci-dessus
  envoie un event `iroha:address-copy` con `{mode,timestamp}` - reliez cela
  a votre pipeline d'analytique/telemetrie (por ejemplo, enviar un segmento o una
  Collecteur base sur NORITO) para que los paneles de control puedan correlacionar el uso
  de format d'adresse cote server con los modos de copia cote client. reflétez
  Además de los ordenadores de dominio Torii (`torii_address_domain_total{domain_kind}`)
  dans le meme flux pour que les revues de retrait Local-12 puissent exportador une
  preuve de 30 jours `domain_kind="local12"` directement depuis le tableau
  `address_ingest` de Grafana.
- Associa cada control a las indicaciones `aria-label`/`aria-describedby`
  se distingue qui expliquent si un literal est sur a partager (IH58) o Sora-only
  (comprimir). Incluez la leyenda de dominio implícita en la descripción para
  Que las tecnologías de asistencia reflejan el contexto del mensaje.
- Exposez une region live (ej. `<output aria-live="polite">...</output>`) aquí
  annonce les resultats de copie et les avertissements, en alignant le
  El funcionamiento de VoiceOver/TalkBack se realiza mediante cable en los ejemplos Swift/Android.Esta instrumentación cumple con ADDR-6b y permite que los operadores puedan hacerlo.
observer a la fois l'ingestion Torii et les modes de copie client avant que les
selecteurs Local soient desactives.

## Kit de herramientas de migración Local -> Global

Utilice el [kit de herramientas Local -> Global](local-to-global-toolkit.md) para
Automatiser l'audit et la conversion des selecteurs Local heredites. El ayudante
Emet a la fois le rapport d'audit JSON y la lista convertie IH58/compressee que
los operadores se unen a los tickets de preparación, tandis que le runbook associe
se encuentran los paneles de control Grafana y las reglas Alertmanager que verrouillent le
corte en modo estricto.

## Referencia rápida del diseño binario (ADDR-1a)

Cuando el SDK expone un avance de dirección (inspectores, indicaciones de
validación, constructores de manifiesto), pointez les developmentpeurs vers le format
captura canónica por cable en `docs/account_structure.md`. El diseño es para siempre
`header · selector · controller`, o los bits del encabezado son:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```- `addr_version = 0` (bits 7-5) aujourd'hui; los valores distintos de cero son reservas
  y doivent palanca `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue los controladores simples (`0`) de los multifirma (`1`).
- `norm_version = 1` codifica las reglas de selección Norm v1. Futuros Les Norm
  reutiliseront le meme champ 2 bits.
- `ext_flag` para siempre `0`; Los bits activos indican las extensiones de
  carga útil sin precios en cargo.

El selector se adapta inmediatamente al encabezado:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Las UI y el SDK deben mostrar el tipo de selector:

- `0x00` = dominio por defecto implícito (sin carga útil).
- `0x01` = resumen local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Ejemplos de hex canoniques que les outils de portefeuille peuvent lier ou integrer
documentos/pruebas auxiliares:

| Tipo de selector | Canonique hexagonal |
|---------------|---------------|
| Implícito por defecto | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumen local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Ver `docs/source/references/address_norm_v1.md` para la mesa completa
selecteur/etat et `docs/account_structure.md` para el diagrama completo de
bytes.

## Imponer las formas canónicas

Les operatorurs qui convertissent les encodages Local herites en IH58 canonique
Cualquier cadena comprimida debe seguir el flujo de trabajo CLI documentado en ADDR-5:1. `iroha tools address inspect` emet mantenimiento de una estructura JSON con IH58,
   Comprese et des payloads hex canonices. El currículum incluye además un objeto
   `domain` con los campeones `kind`/`warning` y refleja todos los dominios disponibles a través de
   el campeón `input_domain`. Cuando `kind` va a `local12`, la CLI imprime un
   Anuncio en stderr y el currículum JSON reflejan el meme consignado para que
   Los pipelines CI y el SDK pueden mostrarse. Paso `legacy  suffix`
   Cuando vuelva a activar la codificación convertida bajo la forma `<ih58>@<domain>`.
2. El SDK puede mostrar el aviso de meme/reanudar a través del asistente
   JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  El ayudante conserva el prefijo IH58 y detecta después del literal sauf si vous
  Fournissez explícitamente `networkPrefix`, donc les resumes pour des reseaux
  non defaut ne sont pas re-rendus silencieusement avec le prefixe par defaut.3. Convierta la carga útil canónica en reutilizando los campos `ih58.value` o
   `compressed` del currículum (o solicita otra codificación a través de `--format`). ces
   Chaines sont deja sures a partager en externo.
4. Mettez a jour les manifests, registres et documents orientes client avec la
   forme canonique et notifiez les contraparties que les selecteurs Local seront
   rechaza une fois le cutover termine.
5. Pour les jeux de donnees en masa, ejecutez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. La comando
   lit des literaux separes par nouvelle ligne (les commentaires commencant par
   `#` son ignorados, y `--input -` o ninguna bandera utilizan STDIN), emet una relación
   JSON con currículums canónicos/IH58/compresse pour cada entrada y cuenta
   Los errores de análisis y los anuncios de dominio local. utilizar
   `--allow-errors` Lors de l'audit de dumps herites contenant des lignes
   parásitos, y bloquear la automatización vía `strict CI post-check` cuando les
   Los operadores están bloqueando los selectores locales en CI.
6. Quand vous avez besoin d'une reecriture ligne a ligne, utilisez
  Pour les feuilles de calcul de remediation des selecteurs Local, utilisez
  para exportar un CSV `input,status,format,...` que se encuentra antes de las codificaciones
  canonices, avertissements et echecs de parse en une seule passe.
   El ayudante ignora las líneas no locales por defecto, convierte cada entradarestante en la codificación solicitada (IH58/compresse/hex/JSON), y conserve el
   dominio original cuando `legacy  suffix` está activo. Associaz-le a
   `--allow-errors` para continuar con el análisis de memes cuando un dump contiene contenido
   Literaux mal formes.
7. Automatización CI/lint puede ejecutar `ci/check_address_normalize.sh`, aquí
   Extraiga los selectores locales de `fixtures/account/address_vectors.json`, les
   conviértalo vía `iroha tools address normalize` y rejoue
   `iroha tools address audit` para comprobar que los lanzamientos
   n'emetent plus de digests Local.`torii_address_local8_total{endpoint}` más
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` y el cuadro Grafana
`dashboards/grafana/address_ingest.json` proporciona la señal de aplicación:
une fois que les Dashboards de Production Montrent Zero Soumissions Local
Legítimos y cero colisiones Local-12 colgante 30 días consecutivos, Torii
basculera la porte Local-8 en echec estricto sur mainnet, seguido por Local-12 une
fois que les domaines globaux ont des entrees de registre correspondantes.
Considere la salida CLI como el aviso del operador para este gel - la meme chaine
 El anuncio se utiliza en el SDK de información sobre herramientas y en la automatización para
mantener la partición con los criterios de clasificación de la hoja de ruta. Torii utilizar
Que sur des clusters dev/test lors du diagnostic de regressions. continuarz a
espejo `torii_address_domain_total{domain_kind}` y Grafana
(`dashboards/grafana/address_ingest.json`) para el paquete anterior ADDR-7
puisse montrer que `domain_kind="local12"` est reste a zero colgante la fenetre
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) agregado tres
barreras:- `AddressLocal8Resurgence` page chaque fois qu'un contexte signale une nouvelle
  incremento Local-8. Detenga los lanzamientos en modo estricto, localice la
  Surface SDK está disponible en el tablero y, si es necesario, se define temporalmente
  predeterminado (`true`).
- `AddressLocal12Collision` se declina cuando dos etiquetas Local-12 hashent
  ver le meme digest. Mettez en pausa las promociones de manifiesto, ejecutaz le
  kit de herramientas Local -> Global para auditar el mapeo de resúmenes y coordinaciones
  con la gobernanza Nexus antes de reemettre l'entree de registre ou de
  reenclencher les rollouts en aval.
- `AddressInvalidRatioSlo` evite que la proporción sea inválida en la etapa de la
  flotte (excepto los rechazos Local-8/strict-mode) supera el SLO de 0.1%
  colgante dix minutos. Utilice `torii_address_invalid_total` para el archivo identificador
  contexto/razón de responsabilidad y coordinación con el equipo SDK propietario anterior
  de reenclencher le mode estricto.

### Extrait de note de release (portefeuille et explorateur)

Incluez le bullet suivant dans les notes de release portefeuille/explorateur
lors du cutover:> **Direcciones:** Ajoute le helper `iroha tools address normalize`
> y la rama en CI (`ci/check_address_normalize.sh`) para las tuberías
> portefeuille/explorateur puissent convertir les selecteurs Local herites vers
> des formes canoniques IH58/compressées avant que Local-8/Local-12 soient
> bloques en mainnet. Mettez a jour les exports personalises pour ejecutor la
> commande et joindre la liste normalisee au bundle de preuve de release.