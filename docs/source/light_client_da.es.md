---
lang: es
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Muestreo de disponibilidad de datos de cliente ligero

La API Light Client Sampling permite a los operadores autenticados recuperar
Muestras de fragmentos de RBC autenticadas por Merkle para un bloque en vuelo. Clientes ligeros
puede emitir solicitudes de muestreo aleatorio, verificar las pruebas devueltas con las
raíz de fragmento anunciada y generar confianza en que los datos están disponibles sin
recuperando toda la carga útil.

## Punto final

```
POST /v1/sumeragi/rbc/sample
```

El punto final requiere un encabezado `X-API-Token` que coincida con uno de los configurados
Tokens API Torii. Las solicitudes también tienen una tarifa limitada y están sujetas a una tarifa diaria.
presupuesto de bytes por persona que llama; exceder cualquiera de ellos devuelve HTTP 429.

### Cuerpo de solicitud

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – hash del bloque objetivo en hexadecimal.
* `height`, `view` – tupla de identificación para la sesión RBC.
* `count`: número deseado de muestras (el valor predeterminado es 1, limitado por la configuración).
* `seed`: semilla RNG determinista opcional para muestreo reproducible.

### Cuerpo de respuesta

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

Cada entrada de muestra contiene el índice del fragmento, los bytes de carga útil (hexadecimales) y la hoja SHA-256.
resumen y una prueba de inclusión de Merkle (con hermanos opcionales codificados como hexadecimal
cuerdas). Los clientes pueden verificar pruebas utilizando el campo `chunk_root`.

## Límites y Presupuestos

* **Muestras máximas por solicitud** – configurable a través de `torii.rbc_sampling.max_samples_per_request`.
* **Bytes máximos por solicitud**: aplicado mediante `torii.rbc_sampling.max_bytes_per_request`.
* **Presupuesto de bytes diario**: seguimiento por persona que llama a través de `torii.rbc_sampling.daily_byte_budget`.
* **Limitación de tasa**: se aplica mediante un depósito de tokens dedicado (`torii.rbc_sampling.rate_per_minute`).

Las solicitudes que exceden cualquier límite devuelven HTTP 429 (CapacityLimit). cuando el trozo
el almacén no está disponible o a la sesión le faltan bytes de carga útil en el punto final
devuelve HTTP 404.

## Integración del SDK

### JavaScript

`@iroha/iroha-js` expone el asistente `ToriiClient.sampleRbcChunks` para que los datos
Los verificadores de disponibilidad pueden llamar al punto final sin realizar su propia búsqueda.
lógica. El asistente valida las cargas útiles hexadecimales, normaliza los números enteros y devuelve
objetos escritos que reflejan el esquema de respuesta anterior:

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

El asistente se activa cuando el servidor devuelve datos con formato incorrecto, lo que ayuda a la paridad JS-04.
Las pruebas detectan regresiones junto con los SDK de Rust y Python. óxido
(`iroha_client::ToriiClient::sample_rbc_chunks`) y Python
(`IrohaToriiClient.sample_rbc_chunks`) ayudantes equivalentes de barco; usar lo que sea
coincide con su arnés de muestreo.