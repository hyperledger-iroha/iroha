---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Безопасность и доступность адресов
descripción: UX-требования для безопасного отображения и передачи адресов Iroha (ADDR-6c).
---

Esta página contiene el documento ADDR-6c. Применяйте эти ограничения к кошелькам, explorers, инструментам SDK and любым поверхностям портала, которые отображают или принимают адреса для людей. Каноническая модель данных находится в `docs/account_structure.md`; La lista de verificación no está permitida, ya que puede colocar estos formatos en el usuario para el almacenamiento y la descarga.

## Безопасные сценарии обмена

- Para realizar una copia de diseño/uso en la dirección IH58. Показывайте разрешенный домен как поддерживающий контекст, чтобы строка с checksum была на виду.
- Antes del diseño "Compartir", incluye direcciones de texto sin formato y códigos QR, así como la carga útil. Дайте пользователям проверить оба перед подтверждением.
- Если места мало (маленькие карточки, уведомления), сохраняйте читаемый префикс, показывайте многоточие и оставляйте последние 4–6 símbolos, чтобы сохранить якорь suma de comprobación. Pulse/haga clic en el botón para copiar varias veces antes de su uso.
- Antes de realizar la sincronización, coloque una tostada en la lámpara IH58 o una tostada. Además, cuando se instala telemetría, se realizan copias de seguridad y dispositivos “podidos” y se busca una regresión de UX.

## IME y защита ввода- Utilice la dirección не-ASCII en la dirección correcta. Когда появляются артефакты IME (ancho completo, Kana, знаки тона), показывайте предупреждение inline, объясняющее, как переключить клавиатуру на латинский ввод перед повтором.
- Inserte archivos de texto sin formato, combine archivos de texto sin formato y conecte archivos ASCII antes de su validación. Esto no puede afectar el progreso de la configuración de IME en la sesión de fotos.
- Усильте валидацию против de ancho cero, selectores de variación y más puntos de código Unicode. Al iniciar sesión en una categoría de código abierto, las suites fuzzing pueden importar telemetría.

## Ожидания для asistencia técnica

- Anótese el bloque de direcciones con `aria-label` o `aria-describedby`, según las preferencias de configuración y la carga útil eliminada. группы по 4–8 символов (“ih guión b tres dos…”). Estos lectores de pantalla no utilizan símbolos de pantalla pequeños.
- Сообщайте об успешных событиях копирования/поделиться через educada actualización de la región en vivo. Указывайте пункт назначения (portapapeles, hoja para compartir, QR), чтобы пользователь понимал, что действие завершено без переноса фокуса.
- Utilice el texto original `alt` para la versión QR anterior (por ejemplo, “Dirección IH58 para `<account>` en la cadena `0x1234`”). El espacio con QR-kanvas activa el respaldo “Copiar dirección como texto” para buscar en ningún sitio.

## Сжатые адреса только для Sora- Puerta: скрывайте сжатую строку `sora…` за явным подтверждением. Para poder realizar la instalación, el formato del robot se puede realizar en Sora Nexus.
- Etiquetado: каждое появление должно включать видимый бейдж “Sora-only” e información sobre herramientas con объяснением, почему другим сетям требуется форма IH58.
- Guardrails: если активный discriminant цепочки не соответствует выделению Nexus, полностью отказывайтесь генерировать сжатый адрес и направляйте пользователя обратно к IH58.
- Telemetría: записывайте частоту запросов and копирования сжатой формы, чтобы playbook инцидентов мог обнаруживать всплески случайного шаринга.

## Качество

- Pruebas de interfaz de usuario automáticas (o storybook a11y suites), que permiten nuevas metadanas y actualizaciones de ARIA сообщений об отказе IME.
- Agregue aplicaciones de control de calidad para archivos IME (kana, pinyin), lector de pantalla de prueba (VoiceOver/NVDA) y copia de QR en temas de contacto antes del lanzamiento.
- Tenga en cuenta estas pruebas en la lista de verificación de versiones relacionadas con los test de paridad del IH58, ya que se han producido regresiones en el dispositivo.