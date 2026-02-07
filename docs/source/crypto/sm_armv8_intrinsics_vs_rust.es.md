---
lang: es
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2026-01-03T18:07:57.096028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Intrínsecos de ARMv8 SM3/SM4 frente a implementaciones de Pure Rust
% Iroha Grupo de trabajo criptográfico
% 2026-02-12

# Aviso

> Usted es LLM y actúa como asesor experto del equipo de criptografía Hyperledger Iroha.  
> Antecedentes:  
> - Hyperledger Iroha es una cadena de bloques autorizada basada en Rust donde cada validador debe ejecutarse de manera determinista para que el consenso no pueda divergir.  
> - Iroha utiliza las primitivas criptográficas GM/T chinas SM2 (firmas), SM3 (hash) y SM4 (cifrado de bloque) para ciertas implementaciones regulatorias.  
> - El equipo envía dos implementaciones SM3/SM4 dentro de la pila del validador:  
> 1. Código escalar de tiempo constante, de corte de bits y de Pure Rust que se ejecuta en cualquier CPU.  
> 2. Kernels acelerados ARMv8 NEON que se basan en las instrucciones opcionales `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` e `SM4EKEY` expuestas en los nuevos servidores Apple M-series y Arm. CPU.  
> - El código acelerado está detrás de la detección de funciones en tiempo de ejecución utilizando intrínsecos `core::arch::aarch64`; el sistema debe evitar un comportamiento no determinista cuando los subprocesos migran a través de núcleos big.LITTLE o cuando las réplicas se crean con diferentes indicadores del compilador.  
> Análisis solicitado:  
> Compare las implementaciones intrínsecas de ARMv8 con las alternativas puras de Rust para la verificación determinista de blockchain. Analice las ganancias de rendimiento/latencia, los obstáculos del determinismo (detección de características, núcleos heterogéneos, riesgo SIGILL, alineación, combinación de rutas de ejecución), propiedades de tiempo constante y las salvaguardas operativas (pruebas, manifiestos, telemetría, documentación del operador) necesarias para mantener todos los validadores sincronizados incluso cuando algunos hardware admiten las instrucciones y otros no.

# Resumen

Dispositivos ARMv8-A que exponen los opcionales `SM3` (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) e `SM4` (`SM4E`, `SM4EKEY`) los conjuntos de instrucciones pueden acelerar sustancialmente el hash GM/T y las primitivas de cifrado de bloque. Sin embargo, la ejecución determinista de blockchain exige un control estricto sobre la detección de funciones, la paridad de respaldo y el comportamiento en tiempo constante. La siguiente guía cubre cómo se comparan las dos estrategias de implementación y qué debe aplicar la pila Iroha.

# Comparación de implementación| Aspecto | Intrínsecos de ARMv8 (AArch64 ASM en línea/`core::arch::aarch64`) | Pure Rust (en rodajas / sin mesa) |
|--------|-------------------------------------------------------------|---------------------------------|
| Rendimiento | Hashing SM3 de 3 a 5 veces más rápido y SM4 ECB/CTR hasta 8 veces más rápido por núcleo en Apple M-series y Neoverse V1; se estrecha cuando está ligado a la memoria. | Rendimiento de referencia limitado por ALU escalar y rota; ocasionalmente se beneficia de las extensiones SHA `aarch64` (a través de la vectorización automática del compilador), pero generalmente retrasa a NEON con una brecha similar de 3 a 8 veces. |
| Latencia | Latencia de bloque único ~30–40 ns en M2 con intrínsecos; Se adapta al hash de mensajes cortos y al cifrado de bloques pequeños en llamadas al sistema. | 90–120 ns por bloque; puede requerir un despliegue para seguir siendo competitivo, lo que aumenta la presión del caché de instrucciones. |
| Tamaño del código | Requiere rutas de código duales (intrínsecas + escalares) y activación del tiempo de ejecución; ruta intrínseca compacta si se utiliza `cfg(target_feature)`. | Camino único; ligeramente más grande debido a las tablas de programación manuales pero sin lógica de activación. |
| Determinismo | Debe bloquear el envío del tiempo de ejecución a un resultado determinista, evitar carreras de sondeo de características entre subprocesos y fijar la afinidad de la CPU si los núcleos heterogéneos difieren (por ejemplo, big.LITTLE). | Determinista por defecto; sin detección de funciones en tiempo de ejecución. |
| Postura en tiempo constante | La unidad de hardware es de tiempo constante para las rondas centrales, pero el envoltorio debe evitar la selección dependiente del secreto al retroceder o mezclar mesas. | Totalmente controlado en Rust; tiempo constante garantizado por la construcción (corte de bits) si se codifica correctamente. |
| Portabilidad | Requiere `aarch64` + funciones opcionales; x86_64 y RISC-V retroceden automáticamente. | Funciona en todas partes; El rendimiento depende de las optimizaciones del compilador. |

# Errores de envío en tiempo de ejecución

1. **Sondeo de características no determinista**
   - Problema: probar `is_aarch64_feature_detected!("sm4")` en SoC big.LITTLE heterogéneos puede generar diferentes respuestas por núcleo, y el robo de trabajo entre subprocesos puede mezclar rutas dentro de un bloque.
   - Mitigación: capture la capacidad del hardware exactamente una vez durante la inicialización del nodo, transmita a través de `OnceLock` y empareje con afinidad de CPU al ejecutar kernels acelerados dentro de la VM o cajas criptográficas. Nunca utilice indicadores de funciones después de que comience el trabajo crítico para el consenso.

2. **Precisión mixta entre réplicas**
   - Problema: los nodos creados con diferentes compiladores pueden no estar de acuerdo en cuanto a la disponibilidad intrínseca (`target_feature=+sm4` habilitación en tiempo de compilación frente a detección en tiempo de ejecución). Si la ejecución pasa por diferentes rutas de código, la sincronización de la microarquitectura puede filtrarse en retrocesos basados ​​en potencia o limitadores de velocidad.
   - Mitigación: distribuya perfiles de compilación canónicos con `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE` explícito, requiera ordenamiento alternativo determinista (por ejemplo, prefiera escalar a menos que la configuración habilite el hardware) e incluya un hash de configuración en los manifiestos para la certificación.3. **Disponibilidad de instrucciones en Apple frente a Linux**
   - Problema: Apple expone las instrucciones SM4 sólo en las versiones más recientes del sistema operativo y del silicio; Las distribuciones de Linux pueden parchear los kernels para enmascararlos en espera de aprobaciones de exportación. Depender de lo intrínseco sin protección provoca SIGILL.
   - Mitigación: puerta a través de `std::arch::is_aarch64_feature_detected!`, captura `SIGILL` en pruebas de humo y trata los intrínsecos faltantes como respaldo esperado (aún determinista).

4. **Fragmentación paralela y ordenación de memoria**
   - Problema: los núcleos acelerados suelen procesar varios bloques por iteración; El uso de cargas/almacenamiento de NEON con entradas no alineadas puede fallar o requerir correcciones de alineación explícitas cuando se alimenta con buffers deserializados Norito.
   - Mitigación: mantenga las asignaciones alineadas con los bloques (por ejemplo, múltiplos de `SM4_BLOCK_SIZE` a través de contenedores `aligned_alloc`), valide la alineación en las compilaciones de depuración y recurra al escalar cuando esté desalineado.

5. **Ataques de envenenamiento de caché de instrucciones**
   - Problema: los adversarios del consenso pueden crear cargas de trabajo que destruyen pequeñas líneas de I-cache en núcleos más débiles, ampliando la diferencia de latencia entre las rutas aceleradas y escalares.
   - Mitigación: corrija la programación según tamaños de fragmentos deterministas, rellene los bucles para evitar ramificaciones impredecibles e incluya pruebas de regresión de microbancos para garantizar que la fluctuación se mantenga dentro de las ventanas de tolerancia.

# Recomendaciones de implementación deterministas

- **Política de tiempo de compilación:** mantenga el código acelerado detrás de un indicador de función (por ejemplo, `sm_accel_neon`) habilitado de forma predeterminada en las versiones de lanzamiento, pero requiera una suscripción voluntaria explícita en las configuraciones para las redes de prueba hasta que la cobertura de paridad esté madura.
- **Pruebas de paridad de respaldo:** mantienen vectores dorados que ejecutan rutas aceleradas y escalares consecutivas (flujo de trabajo actual `sm_neon_check`); extenderse para cubrir los modos SM3/SM4 GCM una vez que llegue el soporte del proveedor.
- **Certificación de manifiesto:** incluya la política de aceleración (`hardware=sm-neon|scalar`) en el manifiesto Norito del nodo para que la divergencia sea detectable durante la admisión de pares.
- **Telemetría:** emite métricas que comparan la latencia por llamada en ambas rutas; alerta si la divergencia excede los umbrales predeterminados (por ejemplo, >5 % de fluctuación), lo que indica una posible deriva del hardware.
- **Documentación:** mantenga actualizada la guía del operador (`sm_operator_rollout.md`) con instrucciones para habilitar/deshabilitar intrínsecos y tenga en cuenta que el comportamiento determinista se conserva independientemente de la ruta.

# Referencias

- `crates/iroha_crypto/src/sm.rs` — Ganchos de implementación NEON vs escalares.
- `.github/workflows/sm-neon-check.yml`: carril NEON CI forzado que garantiza la paridad.
- `docs/source/crypto/sm_program.md`: libera barandillas y puertas de alto rendimiento.
- Manual de referencia de arquitectura de Arm, Armv8-A, sección D13 (instrucciones SM3/SM4).
- GM/T 0002-2012, GM/T 0003-2012: especificaciones oficiales SM3/SM4 para pruebas comparativas.

## Mensaje independiente (copiar/pegar)> Usted es LLM y actúa como asesor experto del equipo de criptografía Hyperledger Iroha.  
> Antecedentes: Hyperledger Iroha es una cadena de bloques autorizada basada en Rust que requiere ejecución determinista entre validadores. La plataforma es compatible con el conjunto de criptografía chino GM/T SM2/SM3/SM4. Para SM3 y SM4, el código base incluye dos implementaciones: (a) código escalar de tiempo constante cortado en bits puro de Rust que se ejecuta en todas partes, y (b) núcleos acelerados ARMv8 NEON que dependen de las instrucciones opcionales `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` e `SM4EKEY`. Las rutas aceleradas se habilitan mediante la detección de funciones en tiempo de ejecución mediante `core::arch::aarch64`; no deben introducir no determinismo cuando los subprocesos migran a través de núcleos big.LITTLE heterogéneos o cuando las réplicas se crean con diferentes indicadores `target_feature`.  
> Tarea: Comparar las implementaciones basadas en tecnología intrínseca con las alternativas escalares para la verificación determinista de blockchain. Detalle las diferencias de rendimiento y latencia, enumere los peligros del determinismo (detección de funciones, núcleos heterogéneos, comportamiento SIGILL, alineación, rutas de ejecución mixtas), comente sobre la postura de tiempo constante y recomiende salvaguardas (estrategia de prueba, campos de manifiesto/atestación, telemetría, documentación del operador) que garanticen que todos los validadores permanezcan sincronizados incluso si las capacidades del hardware difieren.