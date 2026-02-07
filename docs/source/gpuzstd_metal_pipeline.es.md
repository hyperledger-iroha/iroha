---
lang: es
direction: ltr
source: docs/source/gpuzstd_metal_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019b3aa25ae224c1595467ac809f2c53290813e91a78b78b94ca71c3dd950264
source_last_modified: "2026-01-31T19:25:45.072449+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Tubería GPU Zstd (metal)

Este documento describe la canalización de GPU determinista utilizada por Metal helper.
para compresión zstd. Es una guía de diseño e implementación para el
Ayudante `gpuzstd_metal` que emite tramas zstd estándar y bytes deterministas
para un flujo de secuencia dado. Las salidas deben circular con decodificadores de CPU; byte por
La paridad de bytes con el compresor de la CPU no es necesaria porque la generación de secuencias
difiere.

## Metas

- Emitir tramas zstd estándar que se decodifican de manera idéntica con CPU zstd; paridad de bytes
  con el compresor de la CPU no es necesario.
- Salidas deterministas en hardware, controladores y programación de subprocesos.
- Comprobaciones de límites explícitas y vidas útiles de buffer predecibles.

## Nota de implementación actual

- La búsqueda de coincidencias y la generación de secuencias se ejecutan en GPU.
- Ensamblaje de marcos y codificación de entropía (Huffman/FSE) actualmente ejecutados en el host
  usando el codificador integrado en la caja; Los núcleos GPU Huffman/FSE se prueban con paridad, pero no
  aún conectado en la ruta de fotograma completo.
- Decode utiliza el decodificador de cuadros integrado con un respaldo zstd de CPU para cuadros no compatibles;
  La descodificación completa del bloque de GPU sigue en curso.

## Canalización de codificación (nivel alto)

1. Puesta en escena de entrada
   - Copie la entrada en un búfer del dispositivo.
   - Partición en fragmentos de tamaño fijo (para generación de secuencias) y bloques (para
     conjunto de marco zstd).
2. Búsqueda de coincidencias y emisión de secuencias.
   - Los núcleos de GPU escanean cada fragmento y emiten secuencias (longitud literal, coincidencia
     longitud, desplazamiento).
   - El orden de secuencia es estable y determinista.
3. Preparación literal
   - Recopilar literales referenciados por secuencias.
   - Cree histogramas literales y seleccione el modo de bloque literal (sin formato, RLE o
     Huffman) de manera determinista.
4. Tablas de Huffman (literales)
   - Generar longitudes de código a partir del histograma.
   - Cree tablas canónicas con desempate determinista que coincida con la CPU
     salida zstd.
5. Tablas FSE (LL/ML/OF)
   - Normalizar los recuentos de frecuencia.
   - Construir tablas de codificación/decodificación FSE de forma determinista.
6. Escritor de flujo de bits
   - Paquete de bits little-endian (LSB-first).
   - Vaciar los límites de bytes; pad con ceros solamente.
   - Enmascarar valores a anchos de bits declarados y hacer cumplir controles de capacidad.
7. Montaje de bloque y marco.
   - Emitir encabezados de bloque (tipo, tamaño, bandera del último bloque).
   - Serializar literales y secuencias en bloques comprimidos.
   - Emite encabezados de trama zstd estándar y sumas de verificación opcionales.

## Tubería de decodificación (nivel alto)

1. Análisis de cuadros
   - Validar bytes mágicos, configuraciones de ventanas y campos de encabezado de marco.
2. Lector de flujo de bits
   - Leer secuencias de primer bit LSB con controles de límites estrictos.
3. Decodificación literal
   - Decodificar bloques literales (sin formato, RLE o Huffman) en el búfer literal.
4. Decodificación de secuencia
   - Decodificar valores LL/ML/OF usando tablas FSE.
   - Reconstruir coincidencias usando la ventana deslizante.
5. Salida y suma de comprobación
   - Escribe bytes reconstruidos en el búfer de salida.
   - Verificar sumas de verificación opcionales cuando esté habilitado.

## Duración y propiedad del buffer- Búfer de entrada: host -> dispositivo, solo lectura.
- Búfer de secuencia: dispositivo, producido por búsqueda de coincidencias y consumido por entropía
  codificación; sin reutilización entre bloques.
- Buffer literal: dispositivo, producido para cada bloque y liberado después del bloque.
  emisión.
- Búfer de salida: dispositivo, mantiene los bytes de la trama final hasta que el host los copia
  fuera.
- Buffers de scratch: reutilizados en todos los kernels, pero siempre sobrescritos de forma determinista.

## Responsabilidades del núcleo

- Núcleos de búsqueda de coincidencias: busque coincidencias y emita secuencias (LL/ML/OF + literales).
- Huffman construye núcleos: deriva longitudes de código y tablas canónicas.
- Núcleos de compilación FSE: construye tablas LL/ML/OF y máquinas de estado.
- Bloquear núcleos de codificación: serializar literales y secuencias en el flujo de bits.
- Bloquear núcleos de decodificación: analizar flujo de bits y reconstruir literales/secuencias.

## Determinismo y restricciones de paridad

- Las compilaciones de tablas canónicas deben utilizar el mismo orden y desempate que la CPU.
  zstd.
- No hay atómicos ni reducciones que dependan de la programación de subprocesos para ningún byte de salida.
- El empaquetado de Bitstream es little-endian, LSB-first; almohadillas de alineación de bytes con ceros.
- Todas las comprobaciones de límites son explícitas; las entradas no válidas fallan de forma determinista.

## Validación

- Vectores dorados de CPU para el escritor/lector de flujo de bits.
- Pruebas de paridad de corpus que comparan las salidas de GPU y CPU.
- Cobertura difusa para marcos mal formados y condiciones de contorno.

## Evaluación comparativa

Ejecute `cargo test -p gpuzstd_metal gpu_vs_cpu_benchmark -- --ignored --nocapture` para
compare la latencia de codificación de CPU y GPU en todos los tamaños de carga útil. La prueba se salta en los hosts
sin dispositivo compatible con metal; capturar la salida junto con los detalles del hardware
al ajustar los umbrales de descarga de GPU. Norito aplica el mismo límite en
`gpu_zstd::encode_all`, por lo que las personas que llaman directamente coinciden con la puerta heurística.