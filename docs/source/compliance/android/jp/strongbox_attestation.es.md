---
lang: es
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Evidencia de certificación StrongBox: implementaciones en Japón

| Campo | Valor |
|-------|-------|
| Ventana de evaluación | 2026-02-10 – 2026-02-12 |
| Ubicación del artefacto | `artifacts/android/attestation/<device-tag>/<date>/` (formato de paquete según `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Herramientas de captura | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Revisores | Líder de laboratorio de hardware, Cumplimiento y Asuntos Legales (JP) |

## 1. Procedimiento de captura

1. En cada dispositivo enumerado en la matriz StrongBox, genere un desafío y capture el paquete de atestación:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Confirme los metadatos del paquete (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) en el árbol de evidencia.
3. Ejecute el asistente de CI para volver a verificar todos los paquetes sin conexión:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Resumen del dispositivo (2026-02-12)

| Etiqueta de dispositivo | Modelo / Caja Fuerte | Ruta del paquete | Resultado | Notas |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Píxel 6/Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Aprobado (respaldado por hardware) | Desafío vinculado, parche del sistema operativo 2025-03-05. |
| `pixel7-strongbox-a` | Píxel 7/Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Aprobado | Candidato al carril primario de CI; temperatura dentro de las especificaciones. |
| `pixel8pro-strongbox-a` | Píxel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Aprobado (nueva prueba) | Se reemplazó el concentrador USB-C; Buildkite `android-strongbox-attestation#221` capturó el paquete que pasaba. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Aprobado | Perfil de certificación de Knox importado el 9 de febrero de 2026. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Aprobado | Perfil de certificación de Knox importado; El carril CI ahora es verde. |

Las etiquetas del dispositivo se asignan a `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Lista de verificación del revisor

- [x] Verifique que `result.json` muestre `strongbox_attestation: true` y la cadena de certificados a la raíz confiable.
- [x] Confirmar que los bytes de desafío coinciden. Buildkite ejecuta `android-strongbox-attestation#219` (barrido inicial) e `#221` (nueva prueba de Pixel 8 Pro + captura S24).
- [x] Vuelva a ejecutar la captura de Pixel 8 Pro después de la reparación del hardware (propietario: líder del laboratorio de hardware, completado el 13 de febrero de 2026).
- [x] Captura completa del Galaxy S24 una vez que llegue la aprobación del perfil de Knox (propietario: Device Lab Ops, completado el 13 de febrero de 2026).

## 4. Distribución

- Adjunte este resumen más el archivo de texto del informe más reciente a los paquetes de cumplimiento de los socios (lista de verificación FISC §Residencia de datos).
- Rutas de paquetes de referencia al responder a las auditorías de los reguladores; no transmita certificados sin procesar fuera de canales cifrados.

## 5. Registro de cambios

| Fecha | Cambiar | Autor |
|------|--------|--------|
| 2026-02-12 | Captura inicial del paquete JP + informe. | Operaciones de laboratorio de dispositivos |