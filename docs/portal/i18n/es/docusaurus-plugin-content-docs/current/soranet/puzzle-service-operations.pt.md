---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-de-servicio-de-rompecabezas
título: Guía de operaciones del Servicio Puzzle
sidebar_label: Operaciones del servicio de rompecabezas
descripción: Operacao do daemon `soranet-puzzle-service` para entradas de admisión Argon2/ML-DSA.
---

:::nota Fuente canónica
España `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas como copias sincronizadas.
:::

# Guía de operaciones del servicio Puzzle

O demonio `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
boletos de admisión respaldados por Argon2 que reflejan una póliza `pow.puzzle.*`
hacer retransmisión e, cuando se configura, faz broker de tokens de admisión ML-DSA en nombre
Relés de borde dos. Ele expone cinco puntos finales HTTP:

- `GET /healthz` - sonda de vida.
- `GET /v1/puzzle/config` - retorna los parámetros efectivos de PoW/puzzle extraidos
hacer JSON hacer relé (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emite un billete Argon2; un cuerpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  pede um TTL menor (clamp na janela de Policy), vincula o ticket a um transcript
  hash e retorna um ticket assinado pelo relevo + huella digital da assinatura quando
  as chaves de assinatura estao configuradas.
- `GET /v1/token/config` - cuando `pow.token.enabled = true`, retorna una póliza
  activa de token de admisión (huella digital del emisor, límites de TTL/desviación del reloj, ID de retransmisión)
  e o conjunto de revocación mesclado).
- `POST /v1/token/mint` - emite un token de admisión ML-DSA vinculado al hash del currículum
  fornecido; o cuerpo aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.Os tickets produzidos pelo servico sao verificados no teste de integracao
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que también ejercita el sistema
Los aceleradores se retransmiten durante escenarios de DoS volumétrico.
(herramientas/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## Configurar emisión de tokens

Defina los campos JSON del relé en `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` como ejemplo) para habilitar
Fichas ML-DSA. No minimo, forneca a chave publica do emisor e uma lista de revocación
opcional:

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

O puzzle service reutiliza esos valores y recarrega automáticamente el archivo
Norito JSON revocado en tiempo de ejecución. Utilice o CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emitir y
inspeccionar tokens fuera de línea, anexar entradas `token_id_hex` al archivo de revogacao
e auditar credenciales existentes antes de publicar actualizaciones en producción.

Pase una clave secreta del emisor para el servicio de rompecabezas a través de la CLI de banderas:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` también está disponible cuando el secreto y gerenciado por um
tubería de herramientas fora de banda. Oh observador del archivo de revogacao mantem
`/v1/token/config` actualizado; actualizaciones de coordenadas con el comando
`soranet-admission-token revoke` para evitar estado de revogacao defasado.Defina `pow.signed_ticket_public_key_hex` sin JSON del relé para anunciar a chave
publica ML-DSA-44 usada para verificar entradas PoW asesinadas; `/v1/puzzle/config`
reproducción de chave y su huella digital BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
para que los clientes puedan arreglar o verificador. Entradas assinados sao validados contra
o ID de retransmisión y enlaces de transcripción y compartilham o almacén de revocación mesmo; prisionero de guerra
tickets brutos de 74 bytes permanecen válidos cuando el verificador de ticket firmado
Todavía está configurado. Pase el secreto del firmante a través de `--signed-ticket-secret-hex` o
`--signed-ticket-secret-path` para iniciar el servicio de rompecabezas; o startup rejeita
pares de claves divergentes se o secret nao valida contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` aceita `"signed": true` (y opcional `"transcript_hash_hex"`) para
retornar un ticket firmado Norito junto con los bytes del ticket bruto; como respuestas
incluya `signed_ticket_b64` e `signed_ticket_fingerprint_hex` para ayudar a rastrear
huellas dactilares de repetición. Solicitudes com `signed = true` sao rejeitadas se o firmante secreto
nao estiver configurado.

## Playbook de rotacao de chaves1. **Coletar o novo descriptor commit.** Una gobernanza pública o retransmisión
   El descriptor no confirma ningún paquete de directorio. Copiar una cadena hexadecimal para
   `handshake.descriptor_commit_hex` dentro de la configuración JSON del relé compartido
   com o servicio de rompecabezas.
2. **Revisar los límites de la política de rompecabezas.** Confirme que os valores atualizados
   `pow.puzzle.{memory_kib,time_cost,lanes}` estejam alinhados ao plano de liberación.
   Los operadores deben mantener la configuración determinística de Argon2 entre relés.
   (mínimo 4 MiB de memoria, 1 <= carriles <= 16).
3. **Preparar o reiniciar.** Vuelva a cargar la unidad del sistema o el contenedor cuando
   Governance anunciar o cutover de rotacao. O servicio nao soporte hot-reload;
   Para reiniciar es necesario pegar el nuevo descriptor commit.
4. **Validar.** Emita un boleto vía `POST /v1/puzzle/mint` y confirme que
   `difficulty` e `expires_at` corresponden a una política nova. Oh informe de remojo
   (`docs/source/soranet/reports/pow_resilience.md`) captura límites de latencia
   esperados para referencia. Quando tokens estiverem habilitados, busque
   `/v1/token/config` para garantizar que la huella digital del emisor anunciada y a
   contagio de revocaciones correspondientes a los valores esperados.

## Procedimiento de desativacao de emergencia1. Defina `pow.puzzle.enabled = false` en la configuración compartida del relé.
   Mantenha `pow.required = true` se necesita el respaldo de hashcash de tickets
   permanecer obrigatorios.
2. Opcionalmente, fuerce las entradas `pow.emergency` para rejeitar descriptores
   antiguo enquanto o gate Argon2 estiver offline.
3. Reinicia el servicio de retransmisión y rompecabezas para aplicar a mudanca.
4. Monitoree `soranet_handshake_pow_difficulty` para garantizar que hay dificultades
   caia para el valor hashcash esperado e verifique `/v1/puzzle/config` reportando
   `puzzle = null`.

## Monitoreo y alertas- **Latencia SLO:** Acompanhe `soranet_handshake_latency_seconds` y mantenha o P95
  bajo de 300 ms. Las compensaciones hacen la prueba de remojo para fornecem dados de calibracao para
  aceleradores de guardia. (docs/source/soranet/reports/pow_resilience.md:1)
- **Presión de cuota:** Utilice métricas de relé de comunicación `soranet_guard_capacity_report.py`
  para ajustar los tiempos de reutilización de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **Alineación del rompecabezas:** `soranet_handshake_pow_difficulty` deve corresponsal a
  dificultad retornada por `/v1/puzzle/config`. Configuración del relé divergencia indica
  rancio o reinicie falho.
- **Disponibilidad del token:** Alerta se `/v1/token/config` cair para `enabled = false`
  Inesperadamente, `revocation_source` informa que las marcas de tiempo están obsoletas. Operadores
  Debe rotar el archivo Norito de revogacao a través de CLI cuando un token para
  retirado para mantener ese punto final preciso.
- **Estado del servicio:** Sondeo `/healthz` na cadencia habitual de liveness e alertar
  Si `/v1/puzzle/mint` retorna HTTP 500 (indica discrepancia en los parámetros Argon2 o
  falhas de RNG). Errores de token minting aparecen como respuestas HTTP 4xx/5xx en
  `/v1/token/mint`; Trate falhas repetidas como indicación de paginación.

## Registro de auditoría y cumplimientoLos relés emiten eventos `handshake` estructurados que incluyen motivos de aceleración e
duraciones de enfriamiento. Garantía que o tubería de cumplimiento descrita en
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudancas
da política de rompecabezas fiquem auditaveis. Cuando la puerta del rompecabezas estiver habilitada,
Archivamos nuestras entradas emitidas y la instantánea de configuración Norito com o
entrada de lanzamiento para futuros auditorios. Tokens de admisión emitidos antes de janelas
de manutencao devem ser rastreados pelos valores `token_id_hex` e insertados no
arquivo de revogaçao quando expirarem ou forem revogados.