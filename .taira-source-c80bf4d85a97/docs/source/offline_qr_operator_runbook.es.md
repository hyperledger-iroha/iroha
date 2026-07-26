<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Runbook del operador QR sin conexión

Este runbook define ajustes preestablecidos prácticos de `ecc`/dimensión/fps para cámaras con ruido.
entornos cuando se utiliza el transporte QR fuera de línea.

### Ajustes preestablecidos recomendados

| Medio ambiente | Estilo | CEC | Dimensión | FPS | Tamaño del fragmento | Grupo de paridad | Notas |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Iluminación controlada de corto alcance | `sakura` | `M` | `360` | `12` | `360` | `0` | Máximo rendimiento, mínima redundancia. |
| Ruido típico de la cámara del móvil | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Preajuste balanceado preferido (`~3 KB/s`) para dispositivos mixtos. |
| Alto brillo, desenfoque de movimiento, cámaras de gama baja | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Menor rendimiento, mayor resistencia a la decodificación. |

### Lista de verificación de codificación/decodificación

1. Codifique con botones de transporte explícitos.
2. Validar con captura de bucle de escáner antes de la implementación.
3. Fije el mismo perfil de estilo en los asistentes de reproducción del SDK para mantener la paridad de vista previa.

Ejemplo:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Validación del bucle del escáner (perfil sakura-storm de 3 KB/s)

Utilice el mismo perfil de transporte en todas las rutas de captura:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Objetivos de validación:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
-Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Navegador/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Aceptación:

- La reconstrucción completa de la carga útil se realiza correctamente con una trama de datos descartada por grupo de paridad.
- No hay discrepancias en la suma de comprobación/hash de carga útil en el bucle de captura normal.