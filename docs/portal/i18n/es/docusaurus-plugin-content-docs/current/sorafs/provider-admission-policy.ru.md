---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado a [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de privacidad e identificación de dispositivos SoraFS (черновик SF-2b)

Estos resultados prácticos de la función física de **SF-2b**: определить и
принудительно применять процесс допуска, требования к идентичности и
аттестационные payload'ы для провайдеров хранения SoraFS. Она расширяет
proceso de creación, descripción de RFC Architects SoraFS y actualización
оставшуюся работу на отслеживаемые инженерные задачи.

## Цели политики

- Garantía, para que los operadores autorizados puedan publicar el mensaje `ProviderAdvertV1`, según lo establecido previamente.
- Haga clic en el enlace de verificación de documentos idénticos, gobernanza total, certificado de endpoint y mínimos. вкладу apuesta.
- Previamente a los instrumentos de determinación de temperatura, como Torii, шлюзы e `sorafs-node` primero проверки.
- Puede realizar modificaciones y anulaciones variables según la configuración o los instrumentos de otros usuarios.

## Требования к идентичности и estaca| Требование | Descripción | Respuesta |
|------------|----------|-----------|
| Происхождение ключа объявления | Los proveedores que registran el código Ed25519, que aparece en el anuncio. Бандл допуска хранит публичный ключ вместе с подписью gobernancia. | Conecte el polo `ProviderAdmissionProposalV1` al `advert_key` (32 bytes) y conectelo a otro (`sorafs_manifest::provider_admission`). |
| Participación de Указатель | Para el grupo de apuestas emergente `StakePointer`, que se encuentra en el grupo de apuestas activo. | Introduzca las validaciones en `sorafs_manifest::provider_advert::StakePointer::validate()` y guarde los elementos en CLI/test. |
| Etiquetas médicas | Провайдеры объявляют юрисдикцию + юридический контакт. | Utilice los polos anteriores `jurisdiction_code` (ISO 3166-1 alfa-2) y el opcional `contact_uri`. |
| Аттестация эндпоинта | Cada dispositivo disponible debe conectar un dispositivo de certificación mTLS o QUIC. | Abra la carga útil Norito `EndpointAttestationV1` y limpie su dispositivo en una banda dual. |

## Процесс допуска1. **Создание предложения**
   - CLI: добавить `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`,
     formulario `ProviderAdmissionProposalV1` + бандл аттестации.
   - Validez: убедиться в наличии обязательных полей, estaca > 0, mango de fragmentador canónico en `profile_id`.
2. **Gobernanza de Одобрение**
   - Совет подписывает `blake3("sorafs-provider-admission-v1" || canonical_bytes)` используя существующие
     sobre de instrumentos (modelo `sorafs_manifest::governance`).
   - Sobre сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Внесение в реестр**
   - Реализовать общий валидатор (`sorafs_manifest::provider_admission::validate_envelope`), который
     переиспользуют Torii/шлюзы/CLI.
   - Desconecte el documento Torii, elimine anuncios, resúmalos o desvíos de sobres.
4. **Novedades y novedades**
   - Agregue `ProviderAdmissionRenewalV1` a los componentes opcionales/stake.
   - Al ejecutar CLI `--revoke`, estos dispositivos físicos se activan y se controlan la gobernanza.

## Задачи реализации

| Oblast | Задача | Propietario(s) | Estado |
|---------|--------|----------|--------|
| Схема | Utilice `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) en `crates/sorafs_manifest/src/provider_admission.rs`. Реализовано в `sorafs_manifest::provider_admission` с помощниками валидации.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Almacenamiento / Gobernanza | ✅ Завершено |
| Instrumentos CLI | Utilice los comandos `sorafs_manifest_stub`: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Grupo de Trabajo sobre Herramientas | ✅ Завершено |CLI поток теперь принимает промежуточные бандлы сертификатов (`--endpoint-attestation-intermediate`),
выдает канонические байты предложения/sobre y проверяет подписи совета во время `sign`/`verify`. Los operadores pueden
передавать тела advert напрямую или переиспользовать подписанные ads, а файлы подписей можно
Asegúrese de conectar `--council-signature-public-key` con `--council-signature-file` para una máquina automática.

### Справочник CLI

Introduzca el comando `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Banderas disponibles: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y cada minuto de `--endpoint=<kind:host>`.
  - Para la certificación del endpoint de los tributos `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, certificado de verificación
    `--endpoint-attestation-leaf=<path>` (más la opción `--endpoint-attestation-intermediate=<path>`
    для каждого элемента цепочки) y любые согласованные ALPN ID
    (`--endpoint-attestation-alpn=<token>`). Los puntos QUIC pueden ser transportados por horas
    `--endpoint-attestation-report[-hex]=...`.
  - Texto: códigos de barras predeterminados Norito (`--proposal-out`) y formato JSON
    (salida estándar según el modelo `--json-out`).
- `sign`
  - Входные данные: предложение (`--proposal`), подписанный ad (`--advert`), опциональное тело ad
    (`--advert-body`), época de retención y cada minuto que pasa. Подписи можно передавать
    inline (`--council-signature=<signer_hex:signature_hex>`) o через файлы, сочетая
    `--council-signature-public-key` con `--council-signature-file=<path>`.
  - Formulario de sobre válido (`--envelope-out`) y formato JSON con resumen de privacidad,
    числом подписантов и входными путями.
-`verify`
  - Проверяет существующий sobre (`--envelope`) y опционально сверяет соответствующее предложение,
    anuncio или тело anuncio. JSON-отчет подсвечивает значения digest, статус проверки подписей
    и какие опциональные артефакты совпали.
- `renewal`- Agregue un nuevo sobre completo a un resumen completo. Требуются
    `--previous-envelope=<path>` y posterior `--envelope=<path>` (carga útil de Norito).
    Los proveedores de CLI, los alias de perfil, las capacidades y la clave de publicidad no disponibles, según esta información
    обновления participación, эндпоинтов и metadatos. Выводит канонические байты `ProviderAdmissionRenewalV1`
    (`--renewal-out`) además del formato JSON.
- `revoke`
  - La banda variada `ProviderAdmissionRevocationV1` para la prueba, que sobre no está disponible.
    Требует `--envelope=<path>`, `--reason=<text>`, como minimo `--council-signature` y opcionales
    `--revoked-at`/`--notes`. CLI muestra y muestra el resumen de datos, muestra la carga útil Norito
    `--revocation-out` y contiene archivos JSON en un resumen y una copia completa.
| Proverka | Realice un validador de datos, utilice Torii, slюзами e `sorafs-node`. Unidad previa + pruebas de integración CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Redes TL / Almacenamiento | ✅ Завершено || Integración Torii | Puede validar anuncios primero en Torii, bloquear anuncios en políticas y teléfonos públicos. | Redes TL | ✅ Завершено | Torii теперь загружает sobres de gobernanza (`torii.sorafs.admission_envelopes_dir`), проверяет совпадение digest/подписи при приеме и публикует телеметрию допуска.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Novedades | Abra las funciones/programas + CLI y publique el ciclo de instalación en los documentos (con el runbook y los comandos de la CLI en `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Almacenamiento / Gobernanza | ✅ Завершено |
| Telemetría | Определить paneles/alertas `provider_admission` (пропущенное обновление, срок действия sobre). | Observabilidad | 🟠 En el proceso | Счетчик `torii_sorafs_admission_total{result,reason}` существует; paneles/alertas en la sección.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook обновления и отзыва#### Плановое обновление (обновления participación/topologiи)
1. Соберите пару последующего предложения/anuncio через `provider-admission proposal` и `provider-admission sign`,
   увеличив `--retention-epoch` и обновив stake/эндпоинты по необходимости.
2. Выполните
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда проверяет неизменность полей capacidad/perfil через
   `AdmissionRecord::apply_renewal`, выпускает `ProviderAdmissionRenewalV1` y печатает resúmenes для
   Gobernanza pública.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Introduzca el sobre anterior en `torii.sorafs.admission_envelopes_dir`, introduzca la configuración Norito/JSON.
   en el gobierno del repositorio y agrega la actualización de hash + época de retención en `docs/source/sorafs/migration_ledger.md`.
4. Solicite al operador que active y active un nuevo sobre
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para la alimentación primaria.
5. Mantenga y ajuste los accesorios canónicos según `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) proporciona estabilidad Norito.#### Аварийный отзыв
1. Utilice sobres compactos y cierres de etiquetas:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI подписывает `ProviderAdmissionRevocationV1`, проверяет набор подписей через
   `verify_revocation_signatures` y сообщает resumen отзыва.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Utilice el sobre `torii.sorafs.admission_envelopes_dir`, envíe el sobre Norito/JSON a las tarjetas de admisión.
   и зафиксируйте hash причины в протоколе gobernancia.
3. Retire `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, чтобы подтвердить,
   что кеши отбросили отозванный anuncio; храните артефакты отзыва в ретроспективах инцидента.

## Testimonio y televisión- Colocar accesorios dorados en el lugar anterior y posterior del sobre.
  `fixtures/sorafs_manifest/provider_admission/`.
- Utilice CI (`ci/check_sorafs_fixtures.sh`) para la impresión de sobres.
- Сгенерированные accesorios включают `metadata.json` с каноническими resúmenes; aguas abajo-pruebas утверждают
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Pruebas de integración previas:
  - Torii отклоняет anuncios с отсутствующими или просроченными sobres de admisión.
  - CLI проходит ida y vuelta предложения → sobre → verificación.
  - Gobernanza actualizada de la certificación del punto final según la identificación del proveedor.
- Требования по телеметрии:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` в Torii. ✅ `torii_sorafs_admission_total{result,reason}` теперь показывает aceptado/rechazado.
  - Haga clic en el botón de inicio de la sesión de instalación (programado el 7 de diciembre).

## Следующие шаги1. ✅ Завершены изменения схем Norito y добавлены помощники валидации в `sorafs_manifest::provider_admission`. Banderas de funciones не нужны.
2. ✅ CLI de archivos (`proposal`, `sign`, `verify`, `renewal`, `revoke`) задокументированы и проверены интеграционными тестами; Puede utilizar la gobernanza de scripts en la sincronización con el runbook.
3. ✅ Torii admisión/descubrimiento принимает sobres и публикует телеметрические счетчики принятия/отклонения.
4. Enfoque en la información: ver paneles/alertas de admisión, novedades, noticias en tecnología de seis días, podólogos предупреждения (`torii_sorafs_admission_total`, indicadores de caducidad).