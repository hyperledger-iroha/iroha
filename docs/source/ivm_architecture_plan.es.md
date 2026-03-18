---
lang: es
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T10:21:48.087325+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Plan de refactorización de arquitectura

Este plan captura los hitos a corto plazo para remodelar la máquina virtual Iroha.
(IVM) en capas más claras preservando al mismo tiempo las características de seguridad y rendimiento.
Se centra en aislar responsabilidades, hacer que las integraciones de host sean más seguras y
preparando la pila de idiomas Kotodama para su extracción en una caja independiente.

## Metas

1. **Fachada de tiempo de ejecución en capas**: introduce una interfaz de tiempo de ejecución explícita para que la VM
   El núcleo se puede incrustar detrás de un rasgo estrecho y las interfaces alternativas pueden evolucionar.
   sin tocar los módulos internos.
2. **Refuerzo de límites de host/llamada al sistema**: enrute el envío de llamadas al sistema a través de un
   Adaptador dedicado que aplica la política ABI y la validación del puntero antes que cualquier host.
   el código se ejecuta.
3. **Separación de idioma/herramienta**: mueva el código específico Kotodama a una nueva caja y
   mantenga solo la superficie de ejecución del código de bytes en `ivm`.
4. **Cohesión de configuración**: unifica la aceleración y los cambios de funciones para que sean
   impulsado a través de `iroha_config`, eliminando perillas basadas en el entorno en producción
   caminos.

## Desglose de fases

### Fase 1 – Fachada de ejecución (en curso)
- Agregar un módulo `runtime` que define un rasgo `VmEngine` que describe el ciclo de vida
  operaciones (`load_program`, `execute`, plomería del host).
- Enseñar a `IVM` a implementar el rasgo.  Esto mantiene la estructura existente pero permite
  Los consumidores (y las pruebas futuras) dependerán de la interfaz en lugar de lo concreto.
  tipos.
- Comience a eliminar las reexportaciones directas del módulo desde `lib.rs` para que las personas que llaman importen a través de
  fachada cuando sea posible.

**Impacto en la seguridad/rendimiento**: La fachada restringe el acceso directo al interior
estado; sólo los puntos de entrada seguros están expuestos.  Esto hace que sea más fácil auditar el host.
interacciones y razones sobre el manejo de gas o TLV.

### Fase 2: despachador de llamadas al sistema
- Introducir un componente `SyscallDispatcher` que envuelve `IVMHost` y aplica ABI
  validación de política y puntero una vez, en una ubicación.
- Migrar el host predeterminado y los hosts simulados para usar el despachador, eliminando
  lógica de validación duplicada.
- Haga que el despachador sea conectable para que los hosts puedan suministrar instrumentación personalizada sin
  pasando por alto los controles de seguridad.
- Proporcionar un asistente `SyscallDispatcher::shared(...)` para que las VM clonadas puedan reenviar
  llamadas al sistema a través de un host `Arc<Mutex<..>>` compartido sin que cada trabajador construya
  envoltorios a medida.

**Impacto en la seguridad/rendimiento**: la puerta centralizada protege contra hosts que
olvide llamar a `is_syscall_allowed`, y permite el almacenamiento en caché futuro del puntero
validaciones para llamadas al sistema repetidas.

### Fase 3 – Extracción Kotodama
- Compilador Kotodama extraído a `crates/kotodama_lang` (de `crates/ivm/src/kotodama`).
- Proporcionar una API de código de bytes mínima que consume la VM (`compile_to_ivm_bytecode`).

**Impacto en la seguridad/rendimiento**: el desacoplamiento reduce la superficie de ataque de la VM
Core y permite la innovación del lenguaje sin correr el riesgo de regresiones del intérprete.### Fase 4 – Consolidación de la configuración
- Opciones de aceleración de subprocesos a través de ajustes preestablecidos de `iroha_config` (por ejemplo, habilitar backends de GPU) mientras se mantienen las anulaciones del entorno existente (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) como interruptores de interrupción del tiempo de ejecución.
- Exponer un objeto `RuntimeConfig` a través de la nueva fachada para que los anfitriones seleccionen
  explícitamente políticas de aceleración deterministas.

**Impacto en la seguridad/rendimiento**: eliminar los cambios basados en entorno evita el silencio
deriva de configuración y garantiza un comportamiento determinista en todas las implementaciones.

## Próximos pasos inmediatos

- Termine la Fase 1 agregando el rasgo de la fachada y actualizando los sitios de llamadas de alto nivel para
  Depende de ello.
- Auditar las reexportaciones públicas para garantizar solo la fachada y las API deliberadamente públicas.
  escaparse de la caja.
- Prototipo de la API del despachador de llamadas al sistema en un módulo separado y migrar la
  host predeterminado una vez validado.

El progreso en cada fase se rastreará en `status.md` una vez que se complete la implementación.
en marcha.