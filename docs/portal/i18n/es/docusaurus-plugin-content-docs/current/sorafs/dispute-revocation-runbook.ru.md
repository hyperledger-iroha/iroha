---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: disputa-revocación-runbook
título: Ранбук споров и отзывов SoraFS
sidebar_label: Ранбук споров y отзывов
descripción: Gobernanza del proceso para los empleados de las empresas SoraFS, las coordinaciones de los equipos y los evacuadores de aire acondicionado.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/dispute_revocation_runbook.md`. Deje copias sincronizadas de la documentación actual de Sphinx y no la guarde.
:::

## Назначение

Este rango proporciona la gobernanza de los operadores que pueden proporcionar apoyo a las empresas SoraFS, coordinaciones y observaciones. детерминированной эвакуации данных.

## 1. Оценить инцидент

- **Dispositivo de activación:** reduce el SLA (tiempo de actividad/PoR), elimina las replicaciones o las actualizaciones de facturación.
- **Подтвердить телеметрию:** зафиксируйте instantáneas `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` para el usuario.
- **Уведомить стейкхолдеров:** Equipo de almacenamiento (операции провайдера), Consejo de gobierno (орган решения), Observabilidad (обновления дашбордов).

## 2. Подготовить пакет доказательств1. Soberite сырые артефакты (telemetría JSON, logotipos CLI, заметки аудитора).
2. Normalizar los archivos predeterminados (por ejemplo, tarball); зафиксируйте:
   - digerir BLAKE3-256 (`evidence_digest`)
   - punta del medio (`application/zip`, `application/jsonl` y т.д.)
   - Configuración de URI (almacenamiento de objetos, pin SoraFS o punto final, código de acceso Torii)
3. Coloque el paquete en el depósito de evidencia de gobernanza con un paquete de escritura única.

## 3. Подать спор

1. Utilice la especificación JSON para `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Presione CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Проверьте `dispute_summary.json` (подтвердите тип, resumen доказательств и временные метки).
4. Introduzca el archivo JSON en Torii `/v1/sorafs/capacity/dispute` para ver el flujo de trabajo de gobernanza. Зафиксируйте значение ответа `dispute_id_hex`; оно якорит последующие действия по отзыву и аудиторские отчеты.

## 4. Эвакуация и отзыв1. **Окно льготы:** уведомите провайдера о грядущем отзыве; разрешите эвакуацию закрепленных данных, когда это допускает политика.
2. **Generador `ProviderAdmissionRevocationV1`:**
   - Utilice `sorafs_manifest_stub provider-admission revoke` para una conexión completa.
   - Проверьте подписи и digerir отзыва.
3. **Опубликуйте отзыв:**
   - Haga clic en el botón Torii.
   - Убедитесь, что провайдера заблокированы (ожидайте роста `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Observe el tablero:** omete la contraseña, introduzca el ID y regístrelo en el paquete de documentos.

## 5. Post-mortem y последующие действия

- Зафиксируйте таймлайн, корневую причину и меры ремедиации в трекере инцидентов gobernabilidad.
- Определите реституцию (participación de reducción, clawbacks комиссий, возвраты клиентам).
- Документируйте выводы; Puede desactivar SLA y monitorear alertas de personas no identificadas.

## 6. Materiales usados

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел споров)
- `docs/source/sorafs/provider_admission_policy.md` (flujo de trabajo отзыва)
- Дашборд наблюдаемости: `SoraFS / Capacity Providers`

## Cheklist

- [ ] Пакет доказательств собRAN и захеширован.
- [ ] Carga útil спора валидирован локально.
- [ ] Torii-transmisión específica.
- [ ] Отзыв выполнен (если одобрен).
- [ ] Дашборды/ранбуки обновлены.
- [ ] Post-mortem оформлен в совете gobernancia.