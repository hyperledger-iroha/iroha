---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 054720d566baf0d74651c4fda8bac5f0ced70259dce78bb8032ab1861c8225a2
source_last_modified: "2025-12-05T17:03:20.752240+00:00"
translation_last_reviewed: "2026-01-30"
---

# Harness de conformidad del gateway SoraFS

Esta nota describe la arquitectura propuesta para el harness de conformidad SF-5a.
Es un plan de implementación, no el entregable final. La intención es dar a QA y
Tooling un blueprint compartido antes de iniciar la integración.

## Objetivos

1. **Validación por replay:** alimentar fixtures CAR canónicos en un gateway y
   verificar que las pruebas BLAKE3 + PoR coinciden con los manifiestos publicados en los fixtures SoraFS.
2. **Cobertura negativa:** asegurar que los gateways rechacen correctamente chunker handles no soportados, pruebas malformadas, desajustes de admisión e intentos de downgrade.
3. **Pruebas de carga:** sostener ≥1.000 streams de rango concurrentes contra un set de payloads con semilla y verificar latencia, throughput y rechazos deterministas.
4. **Atestación:** producir reportes estructurados de ejecución que los operadores puedan firmar al auto-certificar gateways.

## Integración con CI

La suite determinista de replay ya corre como parte de los helpers de CI del workspace vía
`ci/check_sorafs_gateway_conformance.sh`. El script ejecuta
`cargo test --locked -p integration_tests sorafs_gateway_conformance -- --nocapture`
para reproducir la matriz canónica de fixtures y falla rápido ante regresiones.
Los pipelines nightly deben invocar el script junto con `ci/check_sorafs_fixtures.sh`
para mantener la conformidad ligada a las mismas pruebas, manifiestos y escenarios de rechazo
usados para las atestaciones de operadores.

El mismo binario de tests ahora ejecuta el harness de carga determinista
(`run_deterministic_load_test`) que lanza ≥1.000 solicitudes concurrentes sobre la mezcla
de éxito/rechazo definida por el plan SF-5a. Los resultados (percentiles de latencia y
contadores de rechazos/errores) fluyen a `SuiteReport::to_json_value` para que los bundles
de atestación incluyan evidencia tanto de replay como de carga.

## Boceto de arquitectura

```
┌────────────────────────────┐
│  Registro de fixtures      │
│  (Norito)                  │
│   - Manifiestos            │
│   - Bundles de pruebas     │
│   - Casos negativos        │
└────────────┬───────────────┘
             │
┌────────────▼─────────────┐      ┌──────────────────────────┐
│   Controlador de replay  │      │   Generador de carga     │
│ (Rust, orquestador Tokio │◀────▶│ (Rust + workers          │
│  + validadores Norito)   │      │  configurables)          │
└────────────┬─────────────┘      └──────────────────────────┘
             │
┌────────────▼─────────────┐
│ Capa de adaptadores      │
│  de gateway              │
│  - shim HTTP             │
│  - inyección de headers  │
│  - extracción de pruebas │
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Pipeline de verificación │
│  - check digest manifest │
│  - validación chunk plan │
│  - verificación PoR      │
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Reporte y atestación     │
│  - resumen JSON          │
│  - artefactos de fallos  │
│  - hook de firma (sobre  │
│    de atestación Norito) │
└──────────────────────────┘
```

## Hook de firma de atestación

El harness de conformidad emite resultados legibles por máquina _y_ una atestación Norito
firmada para que los operadores presenten evidencia a gobernanza. El flujo de firma es
determinista y multiplataforma; no depende de tooling ad hoc.

1. **Canonizar el payload del reporte.** Serializar la estructura del reporte con
   `norito::json::to_vec(&report)` para obtener la secuencia canónica de bytes (guardarla
   como `payload_bytes`). El writer de Norito ya fuerza el orden determinista de claves.
   La estructura del reporte debe corresponder al esquema descrito en
   [Formato de reporte](#formato-de-reporte) y ya debe contener `fixtures_commit`, `run_id`
   y el array `scenarios`.
2. **Hash para auditabilidad.** Calcular `let digest = blake3::hash(&payload_bytes);`
   y almacenar el resultado de 32 bytes en hex y multibase.
   Este digest se convierte en `attestation.payload_hash`.
3. **Firmar con material de claves configurado.** Cargar el par de claves del operador desde la
   configuración del harness (por ejemplo, `gateway_attestor.key_path`). Las claves deben usar
   el esquema que exigen los manifiestos de gobernanza — hoy `Ed25519`. El harness
   llama `iroha_crypto::Signature::new(key_pair.private_key(), &payload_bytes)` y registra
   la firma y el identificador de algoritmo.
4. **Envolver en un sobre Norito.** Construir una estructura `SignedGatewayReport`.

   Ejemplo de snippet del harness:

   ```rust
   use blake3;
   use hex;
   use multibase::{encode, Base};
   use iroha_crypto::{Algorithm, KeyPair, Signature};
   use norito::json::{json, Value};

   let key_pair: KeyPair = load_key_pair_from_disk(&config.gateway_attestor)?;
   let payload_bytes = norito::json::to_vec(&report)?;
   let report_json_value: Value = norito::json::from_slice(&payload_bytes)?;
   let digest = blake3::hash(&payload_bytes);
   let signature = Signature::new(key_pair.private_key(), &payload_bytes);
   let (pk_alg, pk_bytes) = key_pair.public_key().to_bytes();
   let envelope = json!({
       "attestation": {
           "payload_hash": {
               "blake3_hex": format!("{digest:x}"),
               "blake3_multibase": format!("z{}", encode(Base::Z, digest.as_bytes())),
           },
           "signer": {
               "account_id": config.operator_account.as_ref(),
               "public_key_hex": hex::encode(pk_bytes),
               "algorithm": pk_alg.as_static_str(),
           },
           "signature_hex": hex::encode(signature.payload()),
           "signed_at_unix": std::time::SystemTime::now()
               .duration_since(std::time::UNIX_EPOCH)
               .expect("system clock before UNIX_EPOCH")
               .as_secs(),
       },
       "report": report_json_value, // idéntico al payload del reporte
   });
   ```

   Codifica el sobre con `norito::json::to_vec(&envelope)` para archivarlo.
   El helper `load_key_pair_from_disk` es un wrapper delgado sobre
   `iroha_crypto::KeyPair::from_private_key` que hace cumplir ACLs de filesystem y
   soporta adaptadores con hardware-backed.
5. **Persistir artefactos.** Escribe tres archivos por ejecución:
   - `report.json` — reporte JSON sin firmar (formato canónico).
   - `report.attestation.to` — sobre de atestación Norito.
   - `report.attestation.txt` — resumen legible (hash + firmante).

6. **CLI de verificación.** Publicar `sorafs-gateway-cert verify` (parte del crate planeado
   `sorafs-gateway-cert`) que realice la operación inversa:
   - Parsear la atestación Norito.
   - Recalcular BLAKE3 sobre el reporte embebido.
   - Verificar la firma con `iroha_crypto::Signature::verify`.
   - Emitir éxito/fallo con exit code no cero si hay mismatch.

Puedes generar el bundle de reporte/atestación localmente con `cargo xtask sorafs-gateway-attest`.
El comando acepta `--signing-key <path>` (private key en hex),
`--signer-account <soraカタカナ...>` (AccountId codificado sin dominio; sufijo `@domain` rechazado), y `--gateway <url>` más `--out <dir>` opcional.
Los artefactos se guardan por defecto en `artifacts/sorafs_gateway_attest/`.

Al mantener el hook de firma dentro del binario del harness, los runs nightly de CI pueden
publicar artefactos firmados automáticamente, y los operadores solo necesitan proporcionar
una ruta de keypair o un signer hardware-backed vía el mismo trait.

## Matriz de pruebas

| ID | Escenario | Resultado esperado |
|----|-----------|-------------------|
| A1 | Replay CAR completo (perfil sf1) | 200 OK, verificación chunk/PoR pasa |
| A2 | Byte range (alineado a límites de chunk) | 206 Partial Content, pruebas limitadas al rango solicitado |
| A3 | Byte range (solicitud desalineada) | 416 Range Not Satisfiable |
| A4 | Replay multi-range (parcial + chunk completo) | 206 Partial Content con segmentos multipart ordenados |
| B1 | Handle de chunker no soportado | 406 con body `unsupported_chunker` |
| B2 | Sobre de manifiesto ausente | 428 fallo de admisión |
| B3 | Prueba PoR corrupta | 422 fallo de prueba |
| B4 | Payload CAR corrupto (digest mismatch) | 422 rechazo (payload digest mismatch) |
| B5 | Proveedor no admitido | 412 precondition failure con `provider_not_admitted` |
| B6 | Cliente excede ventana de rate limit | 429 `rate_limited` con header `Retry-After` |
| C1 | 1k concurrentes streaming de rango (caché warm) | P95 < objetivo, sin fallos de prueba |
| C2 | 1k concurrentes con 1% corrupción inyectada | Todas las respuestas corruptas rechazadas, gateway devuelve 422 |
| D1 | Carga con trigger de denylist GAR | 451 Unavailable For Legal Reasons |

El harness en Rust ya ejerce los escenarios A1, A2, A3, A4, B1, B2, B3, B4, B5 y B6
contra fixtures deterministas, afirmando digests canónicos, alineación de byte-range,
semántica de rechazo y enforcement de políticas.

## Denylist de ejemplo

Para ejercitar enforcement de políticas sin cablear feeds de gobernanza, apunta la configuración del nodo
`torii.sorafs_gateway.denylist.path` a `docs/source/sorafs_gateway_denylist_sample.json`. El fixture
contiene:
- entradas `provider` y `manifest_digest` que demuestran identificadores hex de ancho fijo con ventanas de jurisdicción opcionales.
- una entrada `cid` codificada en base64 junto a una ventana de expiración.
- una entrada `url` para bloqueo a nivel de URL.
- una entrada `account_id` usando el encoding hex canónico de AccountAddress para reflejar suspensiones de gobernanza.
- una entrada `account_alias` que bloquea un alias de ruteo (`name@dataspace` or `name@domain.dataspace`).
- una entrada `perceptual_family` que empareja un UUID de familia/variante con metadata de hash perceptual (`perceptual_hash_hex`, `perceptual_hamming_radius`) para que los gateways bloqueen clusters de contenido casi duplicado.

Cada registro sigue el mismo layout Norito JSON que usa el loader, incluyendo campos opcionales
`issued_at` y `expires_at`. Los operadores pueden copiar el archivo como punto de partida para
sus feeds de gobernanza o tests de integración.

## Formato de reporte

El harness debe emitir un documento JSON firmado:

```json
{
  "profile_version": "sf1",
  "gateway_target": "https://gateway.example.com",
  "fixtures_commit": "abc123",
  "scenarios": [
    { "id": "A1", "status": "pass", "duration_ms": 420 },
    { "id": "B3", "status": "pass" },
    { "id": "C1", "status": "pass", "p95_latency_ms": 84 }
  ],
  "failures": [],
  "timestamp": "2025-02-14T12:34:56Z",
  "run_id": "uuid"
}
```

Los operadores pueden adjuntar su firma (sobre Norito firmado) para auto-certificación.

## Smoke test manual de gateway (`sorafs-fetch`)

Para validación ligera fuera del harness completo, los operadores pueden apuntar el CLI multi-source
directamente a un gateway Torii usando el nuevo flag `--gateway-provider`:

```
sorafs-fetch \
  --plan=fixtures/chunk_fetch_plan.json \
  --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --gateway-client-id=ops-orchestrator \
  --json-out=reports/gateway_smoke.json
```

El reporte resultante refleja la salida de conformidad (incluyendo `provider_reports[].metadata`)
y puede adjuntarse a tickets de cambio durante rollouts blue/green.

## Preguntas abiertas / próximos pasos

- Determinar layout del repositorio de fixtures (probablemente `fixtures/sorafs_gateway/`).
- Decidir los defaults de muestreo PoR para pruebas de carga (relacionado con SF-13).
- Seleccionar el driver de concurrencia (Tokio vs generador externo).
- Integrar con CI (GitHub Actions e internos Jenkins).

La cobertura de rechazo por downgrade/headers faltantes y el probe HEAD ya viven en el
harness de integración (`integration_tests/tests/sorafs_gateway_conformance.rs:295`
e `integration_tests/tests/sorafs_gateway_conformance.rs:442`).

Se agradece el feedback — registrar comentarios bajo SF-5a en el roadmap.
