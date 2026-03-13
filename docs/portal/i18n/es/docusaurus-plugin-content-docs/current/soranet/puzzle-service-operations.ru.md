---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-de-servicio-de-rompecabezas
título: Руководство по эксплуатации Servicio de rompecabezas
sidebar_label: Operaciones de servicio de rompecabezas
descripción: Эксплуатация daemon `soranet-puzzle-service` para boletos de admisión Argon2/ML-DSA.
---

:::nota Канонический источник
:::

# Руководство по эксплуатации Servicio de rompecabezas

Daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) выпускает
Boletos de admisión respaldados por Argon2, según la política `pow.puzzle.*` у retransmisión
Y, por lo general, bloquea los tokens de admisión ML-DSA o los relés de borde.
Cómo configurar puntos finales HTTP:

- `GET /healthz` - sonda de vida.
- `GET /v2/puzzle/config` - возвращает эффективные параметры PoW/puzzle,
  Utilice el relé JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - выпускает billete Argon2; cuerpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запрашивает более короткий TTL (ventana de política sujeta k), привязывает ticket
  к hash de transcripción и возвращает boleto firmado por retransmisión + huella digital de firma
  при наличии firma de claves.
- `GET /v2/token/config` - когда `pow.token.enabled = true`, возвращает активную
  política de token de admisión (huella digital del emisor, límites TTL/desviación del reloj, ID de retransmisión,
  y conjunto de revocación fusionado).
- `POST /v2/token/mint` - token de admisión ML-DSA de выпускает, incluido en el suministro
  reanudar el hash; cuerpo принимает `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.Entradas, servicios exclusivos, pruebas integradas
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, который также
упражняет aceleradores de relé во время volumétricos DoS сценариев.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Настройка выпуска токенов

Задайте поля rele JSON под `pow.token.*` (см.
`tools/soranet-relay/deploy/config/relay.entry.json` (primero), чтобы
включить tokens ML-DSA. Clave pública del emisor mínima previa y opcional
lista de revocación:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

Servicio de rompecabezas повторно использует эти значения автоматически перезагружает
Norito Revocación de archivo JSON en tiempo de ejecución. Utilice CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) чтобы выпускать и
verificar tokens fuera de línea, agregar entradas `token_id_hex` en un archivo de revocación y
Proporcionar credenciales de auditoría para actualizaciones de publicaciones en producción.

Consulte la clave secreta del emisor en el servicio de rompecabezas con las banderas CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` доступен, когда secret управляется herramientas fuera de banda
tubería. Vigilante de archivos de revocación держит `/v2/token/config` актуальным;
координируйте обновления с командой `soranet-admission-token revoke`, чтобы
избежать отставания estado de revocación.Instale `pow.signed_ticket_public_key_hex` en el relé JSON, чтобы объявить
Clave pública ML-DSA-44 para tickets PoW firmados; `/v2/puzzle/config` эхо
llave de voz y huella digital BLAKE3 (`signed_ticket_public_key_fingerprint_hex`),
чтобы clientes могли pin-ить verificador. Boletos firmados проверяются по ID de retransmisión и
enlaces de transcripciones y aplicaciones en el almacén de revocación; tickets PoW sin procesar de 74 bytes
остаются валидными при настроенном verificador de boletos firmados. Передайте secreto del firmante
por `--signed-ticket-secret-hex` o `--signed-ticket-secret-path` por cierre
servicio de rompecabezas; START отклонит несоответствующие keypairs, если secret не валидируется
против `pow.signed_ticket_public_key_hex`. `POST /v2/puzzle/mint` modelo
`"signed": true` (y opcional `"transcript_hash_hex"`) чтобы вернуть
El ticket firmado con codificación Norito contiene bytes del ticket sin procesar; ответы включают
`signed_ticket_b64` y `signed_ticket_fingerprint_hex` para reproducir huellas dactilares.
Запросы с `signed = true` отклоняются, если signer secret не настроен.

## Libro de jugadas ротации ключей1. ** Соберите новый confirmación del descriptor. ** Descriptor de retransmisión de gobernanza pública
   confirmar en paquete de directorio. Скопируйте cadena hexadecimal en `handshake.descriptor_commit_hex`
   внутри retransmisión JSON конфигурации, которой пользуется servicio de rompecabezas.
2. **Acertijo de política de límites.** Подтвердите, что обновленные значения
   `pow.puzzle.{memory_kib,time_cost,lanes}` соответствуют plan de lanzamiento. Operadores
   должны держать Argon2 конфигурацию детерминированной между relés (mínimo 4 MiB
   памяти, 1 <= carriles <= 16).
3. **Reiniciar.** Coloque la unidad systemd o el contenedor después de esto, como
   gobernanza объявит corte de rotación. Сервис не поддерживает hot-reload; для
   применения нового descriptor commit требуется reiniciar.
4. **Провалидируйте.** Compre el billete con `POST /v2/puzzle/mint` y envíelo,
   что `difficulty` e `expires_at` son nuevas políticas. Informe de remojo
   (`docs/source/soranet/reports/pow_resilience.md`) содержит ожидаемые límites de latencia
   для справки. Когда tokens включены, запросите `/v2/token/config`, чтобы убедиться,
   что la huella digital del emisor anunciado y el recuento de revocación соответствуют ожидаемым значениям.

## Procedimiento de desactivación de emergencia1. Instale `pow.puzzle.enabled = false` en otras configuraciones de relé. Оставьте
   `pow.required = true`, otros tickets de reserva de hashcash que están disponibles.
2. Opcionalmente, introduzca las entradas `pow.emergency`, para evitar el uso de usuarios.
   descriptores пока Puerta Argon2 fuera de línea.
3. Перезапустите retransmisión y servicio de rompecabezas, чтобы применить изменение.
4. Monitoree `soranet_handshake_pow_difficulty`, чтобы убедиться, что сложность
   упала до ожидаемого hashcash значения, and проверьте, что `/v2/puzzle/config`
   сообщает `puzzle = null`.

## Monitoreo y alerta- **Latencia SLO:** Seleccione `soranet_handshake_latency_seconds` y deje P95.
  menos de 300 ms. Compensaciones de prueba de remojo de datos de calibración de aceleradores de protección.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Presión de cuota:** Используйте `soranet_guard_capacity_report.py` con métricas de relé
  для настройки `pow.quotas` tiempos de reutilización (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alineación del rompecabezas:** `soranet_handshake_pow_difficulty` должен совпадать с
  dificultad из `/v2/puzzle/config`. Divergente configuración de retransmisión obsoleta
  или неудачный reiniciar.
- **Preparación del token:** Alerta, если `/v2/token/config` неожиданно падает до
  `enabled = false` o `revocation_source` incluyen marcas de tiempo obsoletas. Operadores
  должны ротировать Norito archivo de revocación через CLI при выводе токена, чтобы
  punto final оставался точным.
- **Estado del servicio:** Pruebe `/healthz` en cadencia de vida y alerta,
  o `/v2/puzzle/mint` envía HTTP 500 (desajustado por falta de coincidencia de parámetros de Argon2)
  o fallos del RNG). Ошибки token acuñación проявляются как HTTP 4xx/5xx en
  `/v2/token/mint`; повторяющиеся сбои следует считать condición de paginación.

## Cumplimiento y registro de auditoríaRelés публикуют структурированные `handshake` eventos, включающие razones del acelerador
y duraciones de enfriamiento. Убедитесь, что proceso de cumplimiento из
`docs/source/soranet/relay_audit_pipeline.md` registra estos registros y características
política de rompecabezas оставались auditables. Когда puzzle gate включен, архивируйте
образцы tickets acuñados y instantánea de configuración Norito junto con ticket de implementación
для будущих auditorías. Fichas de admisión, ventanas de mantenimiento previas,
cerrar el archivo de revocación `token_id_hex` y agregarlo al archivo de revocación
истечения или отзыва.