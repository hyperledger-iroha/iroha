---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota Канонический источник
Esta página está escrita en `docs/source/sns/address_display_guidelines.md` y en el teclado.
служит канонической копией портала. Исходный файл остается для PR переводов.
:::

Colecciones, actualizaciones y primeros SDK disponibles en las cuentas de dirección
к неизменяемым carga útil. Primer dispositivo Android en
`examples/android/retail-wallet` patrón de demostración de UX:- **Otros botones de copia.** Otros botones de copia anteriores: IH58
  (предпочтительно) y сжатая formato exclusivo de Sora (`sora...`, второй по предпочтению). IH58 всегда безопасно
  делиться наружу он используется в QR payload. Сжатая форма должна включать
  встроенное предупреждение, потому что работает только в Sora-aware приложениях.
  Primeros botones de Android Material e información sobre herramientas en
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, а
  La demostración de iOS SwiftUI se adapta a la experiencia de usuario mediante `AddressPreviewCard` en
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, выбираемый текст.** Отрисовывайте обе строки моноширинным
  шрифтом и с `textIsSelectable="true"`, чтобы пользователи могли проверять
  значения без вызова IME. Cambiar el polo de redacción: IME puede repetirse
  kana или внедрить нулевой ширины кодовые точки.
- **Подсказки для неявного DOMена по умолчанию.** Когда селектор указывает na
  неявный домен `default`, показывайте подпись, напоминающую операторам, что
  суффикс не требуется. Обозреватели также должны выделять канонический доменный
  ярлык, когда селектор кодирует digest.
- **QR para IH58.** QR-коды должны кодировать строку IH58. Если generación QR
  провалилась, покажите явную ошибку вместо пустого изображения.
- **Сообщение буфера обмена.** Después de copiar formas de presentación de tostadas
  или snackbar, напоминающий пользователям, что она Sora-only y подвержена
  искажению IME.Следование этим guardrails предотвращает повреждение Unicode/IME y удовлетворяет
критериям приемки дорожной карты ADDR-6 для UX кошельков/обозревателей.

## Скриншоты для сверки

Utilice algunas configuraciones de configuración localizadas, cuáles son sus características
Los botones, la información sobre herramientas y las instrucciones de configuración muestran las siguientes plataformas:

- Reproductor de Android: `/img/sns/address_copy_android.svg`

  ![Справка Android двойного копирования](/img/sns/address_copy_android.svg)

- Versión iOS: `/img/sns/address_copy_ios.svg`

  ![Справка iOS двойного копирования](/img/sns/address_copy_ios.svg)

## Programas SDK

El SDK de caché contiene un asistente adicional, varios formularios IH58 y dispositivos, y
Esta línea de configuración de interfaz de usuario está configurada de forma predeterminada:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)` возвращает предупреждение о
  сжатой фоорме и добавляет его в `warnings`, когда вызывающие передают literal
  `sora...`, чтобы обозреватели/panelи кошельков могли показывать предупреждение
  Sora-only во время вставки/валидации, а не только при генерации сжатой формы.
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilice estos ayudantes para codificar todas las lógicas implementadas en la interfaz de usuario.
El asistente de JavaScript elimina la carga útil `selector` en `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`), чтобы UIs могли указать, является
El selector Local-12 o el registro de podkreplen no requieren ninguna carga útil.

## Демонстрация инструментирования обозревателя



Las siguientes opciones son colocar teléfonos y dispositivos de descarga:- Pulse `data-copy-mode="ih58|compressed|qr"` en los botones de copia, чтобы
  Los principales fabricantes de interfaces emiten datos de configuración con un sistema métrico Torii.
  `torii_address_format_total`. El componente demostrativo está disponible
  событие `iroha:address-copy` с `{mode,timestamp}` - подключите его к своему
  Аналитическому/телеметрийному пайплайну (por ejemplo, en Segment o NORITO
  коллектор), чтобы paneles de instrumentos могли коррелировать использование форматов
  Dirección del servidor de copia de seguridad del cliente. Также зеркальте счетчики
  доменов Torii (`torii_address_domain_total{domain_kind}`) в том же фиде, чтобы
  проверки вывода Local-12 могли экспортировать 30-дневное доказательство
  `domain_kind="local12"` está conectado a los paneles Grafana `address_ingest`.
- Сопоставляйте каждому контролу отдельные `aria-label`/`aria-describedby`,
  объясняющие, безопасен ли literal для обмена (IH58) или только для Sora
  (сжатый). Включайте подпись неявного домена в описание, чтобы вспомогательные
  технологии показывали тот же контекст.
- Экспонируйте región en vivo (por ejemplo, `<output aria-live="polite">...</output>`),
  который объявляет результаты копирования и предупреждения, совпадая с
  VoiceOver/TalkBack está disponible en Swift/Android.

Este instrumento de medición del ADDR-6b puede ser utilizado por los operadores
conecte y utilice Torii y realice copias de seguridad del cliente local
selectores.

## Набор миграции Local -> GlobalИспользуйте [Local -> Kit de herramientas global](local-to-global-toolkit.md) para
автоматизации аудита и конверсии устаревших Selectores locales. Ayudante выводит и
Auditoría JSON y convertidor IH58/convertidor de texto original
Los operadores aplican los tickets de preparación, un runbook compatible.
Los paneles Grafana y el Alertmanager, que realizan una transición estricta.

## Быстрый справочник бинарной раскладки (ADDR-1a)

Когда SDK показывают продвинутые инструменты адресов (inspectores, подсказки
валидации, constructores de manifiestos), направляйте разработчиков к каноническому
formato de cable es `docs/account_structure.md`. Раскладка всегда
`header · selector · controller`, encabezado de bits de gran tamaño:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) сегодня; ненулевые значения зарезервированы и
  должны приводить к `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` combina controladores individuales (`0`) y multifirma (`1`).
- `norm_version = 1` кодирует правила селектора Norm v1. Будущие нормы будут
  Utilice el polo de 2 bits.
- `ext_flag` всегда `0`; установленные биты означают неподдерживаемые расширения
  carga útil.

El selector selecciona el siguiente encabezado:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

La interfaz de usuario y el SDK se pueden seleccionar mediante este selector:- `0x00` = неявный домен по умолчанию (без payload).
- `0x01` = resumen local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = запись глобального registro (`registry_id:u32` big-endian).

Los términos hexadecimales y los instrumentos de programación pueden utilizarse o
встраивать в documentos/pruebas:

| Selector de puntas | Hexágono canónico |
|---------------|---------------|
| Неявный по умолчанию | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumen local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

См. `docs/source/references/address_norm_v1.md` para selector/estado de múltiples tablas
y `docs/account_structure.md` para los diagramas de polos.

## Принуждение канонических форм

Operadores, convertidores de frecuencia, codificadores locales en canales canónicos IH58 o
Para obtener más información, utilice la línea CLI en ADDR-5:

1. `iroha tools address inspect` escribe la estructura JSON en IH58,
   carga útil hexadecimal comprimida y canónica. Резюме также включает объект `domain`
   с полями `kind`/`warning` y отражает любой переданный домен через
   `input_domain`. Desde `kind` a partir de `local12`, la CLI está configurada previamente en
   stderr, un archivo JSON integrado en el archivo, todos los archivos CI y SDK
   ее показывать. Mantenga `--append-domain`, o хотите воспроизвести
   Codificación convertible como `<ih58>@<domain>`.
2. El SDK puede insertar la advertencia/resumen del asistente de JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Ayudante сохраняет IH58 префикс, извлеченный из literal, если только вы явно не
  указали `networkPrefix`, la opción de configuración no predeterminada no es necesaria
  тихо с дефолтным префиксом.3. Conversión de carga útil canónica, versión inicial `ih58.value` o
   `compressed` es una respuesta (o se utiliza una codificación diferente para `--format`). Эти
   строки уже безопасны для внешнего обмена.
4. Обновите manifiestos, registros y documentos de clientes formularios canónicos y
   Utilice los controles locales para seleccionar los selectores locales después de la transición.
5. Для больших наборов данных запустите
   `iroha tools address audit --input addresses.txt --network-prefix 753`. comando
   читает literales, разделенные переводом строки (коментарии с `#` игнорируются,
   (A `--input -` o una bandera externa implementada por STDIN), contiene archivos JSON
   каноническими/IH58/сжатыми резюме для каждой записи и считает ошибки парсинга
   и предупреждения Local домена. Utilice `--allow-errors` antes de una auditoría
   `--fail-on-warning`, cuando los operadores activan el selector local en CI.
6. Когда нужна построчная перепись, используйте
  Для таблиц remediation Local селекторов используйте
  Para exportar CSV `input,status,format,...`, varios archivos canónicos
  кодировки, предупреждения и ошибки парсинга за один проход.
   Ayudante para la aplicación de aplicaciones locales, convertidor de archivos de configuración
   запись в запрошенный codificación (IH58/сжатый/hex/JSON) y сохраняет исходный DOMен
   por `--append-domain`. Совмещайте с `--allow-errors`, чтобы продолжать скан даже
   если dump содержит поврежденные literales.7. La automatización de CI/lint puede eliminar `ci/check_address_normalize.sh`, código
   Utilice selectores locales como `fixtures/account/address_vectors.json`,
   Convertidor con `iroha tools address normalize`, y un teclado portátil
   `iroha tools address audit --fail-on-warning`, чтобы доказать, что релизы больше не
   эмитят Resúmenes locales.

`torii_address_local8_total{endpoint}` todo el mundo
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` y panel Grafana
`dashboards/grafana/address_ingest.json` Aplicación de la señal de entrada: código
продакшн-дашборды показывают ноль легитимных Local отправок и ноль Local-12
En una edición de 30 días, Torii mantiene la puerta Local-8 en hard-fail
mainnet, un sitio Local-12, una gran red de dominios globales que publican mensajes de texto
registro. Считайте вывод CLI операторским уведомлением об этом замораживании -
та же строка предупреждения используется в SDK tooltips and автоматизации, чтобы
сохранять паритет с критериями выхода дорожной карты. Torii теперь по умолчанию
кластерах при диагностике регрессий. Продолжайте зеркалировать
`torii_address_domain_total{domain_kind}` en Grafana
(`dashboards/grafana/address_ingest.json`), чтобы пакет доказательств ADDR-7
Más información, como `domain_kind="local12"` está instalada en la tecnología
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) добавляет три
barandillas:- `AddressLocal8Resurgence` пейджит, когда контекст сообщает о свежем инкременте
  Local-8. Остановите implementaciones en modo estricto, найдите проблемный SDK в дашборде и,
  возврата сигнала к нулю - затем восстановите dефолт (`true`).
- `AddressLocal12Collision` срабатывает, когда два Local-12 лейбла хэшируются в
  resumen de odin. Приостановите manifiestos de promociones, запустите Local -> Global
  kit de herramientas para auditar resúmenes de mapeo y coordinar con la gobernanza Nexus antes
  перевыпуском entrada de registro или повторным включением implementaciones posteriores.
- `AddressInvalidRatioSlo` предупреждает, когда флотский relación no válida (исключая
  отказы Local-8/strict-mode) prevышает SLO 0.1% в течение десяти minут. Используйте
  `torii_address_invalid_total` para cualquier contacto/personaje y
  Configure el SDK antes de activar el modo estricto.

### Фрагмент релизной заметки (кошелек и обозреватель)

Включите следующий viñeta en notas de la versión кошелька/обозревателя при cutover:

> **Адреса:** Добавлен ayudante `iroha tools address normalize --only-local --append-domain`
> и подключен в CI (`ci/check_address_normalize.sh`), чтобы пайплайны кошелька/
> обозревателя могли конвертировать устаревшие Selector local de canales
> IH58/formatos para bloques Local-8/Local-12 en mainnet. Обновите любые
> кастомные экспорты, чтобы запускать команду и прикладывать нормализованный
> список к paquete de pruebas релиза.