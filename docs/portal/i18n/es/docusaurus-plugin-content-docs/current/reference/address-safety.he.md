---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/reference/address-safety.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 689f36e02c2ddca55c2198d2e03895ff6656c0fd7505b1c7b471cdb0d943b90e
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Seguridad y accesibilidad de direcciones
description: Requisitos de UX para presentar y compartir direcciones de Iroha con seguridad (ADDR-6c).
---

Esta pagina captura el entregable de documentacion ADDR-6c. Aplica estas restricciones a wallets, explorers, herramientas de SDK y cualquier superficie del portal que renderice o acepte direcciones orientadas a personas. El modelo de datos canonico vive en `docs/account_structure.md`; la checklist de abajo explica como exponer esos formatos sin comprometer seguridad o accesibilidad.

## Flujos seguros de comparticion

- Por defecto, cada accion de copiar/compartir debe usar la direccion IH58. Muestra el dominio resuelto como contexto de apoyo para que la cadena con checksum permanezca al frente.
- Ofrece una accion "Compartir" que incluya la direccion en texto plano y un QR derivado del mismo payload. Permite que las personas inspeccionen ambos antes de confirmar.
- Cuando el espacio obligue a truncar (tarjetas pequenas, notificaciones), conserva el prefijo legible, muestra puntos suspensivos y reten los ultimos 4-6 caracteres para que sobreviva el ancla del checksum. Provee un toque/atajo de teclado para copiar la cadena completa sin truncamiento.
- Evita la desincronizacion del portapapeles emitiendo un toast de confirmacion que previsualice la cadena IH58 exacta que se copio. Donde haya telemetria, cuenta intentos de copia versus acciones de compartir para detectar regresiones de UX rapido.

## IME y salvaguardas de entrada

- Rechaza entradas no ASCII en campos de direccion. Cuando aparezcan artefactos de composicion IME (full width, Kana, marcas de tono), muestra una advertencia inline que explique como cambiar el teclado a entrada en latin antes de reintentar.
- Provee una zona de pegado en texto plano que elimine marcas combinantes y reemplace espacios en blanco por espacios ASCII antes de validar. Esto evita que la persona pierda progreso cuando desactiva el IME a mitad de flujo.
- Endurece la validacion contra zero-width joiners, variation selectors y otros puntos de codigo Unicode sigilosos. Registra la categoria del punto de codigo rechazado para que los fuzzing suites puedan importar la telemetria.

## Expectativas de tecnologia asistiva

- Anota cada bloque de direccion con `aria-label` o `aria-describedby` que deletree el prefijo legible y agrupe el payload en bloques de 4-8 caracteres ("ih dash b three two ..."). Esto evita que los lectores de pantalla produzcan un flujo ininteligible de caracteres.
- Anuncia los eventos de copia/comparticion exitosos mediante una actualizacion de live region en modo polite. Incluye el destino (portapapeles, hoja de compartir, QR) para que la persona sepa que la accion se completo sin mover el foco.
- Provee texto `alt` descriptivo para las vistas previas de QR (p. ej., "Direccion IH58 para `<account>` en la cadena `0x1234`"). Incluye un fallback "Copiar direccion como texto" junto al canvas de QR para personas con baja vision.

## Direcciones comprimidas solo Sora

- Gating: oculta la cadena comprimida `sora...` detras de una confirmacion explicita. La confirmacion debe reiterar que el formato solo funciona en cadenas Sora Nexus.
- Etiquetado: cada aparicion debe incluir una insignia visible "Solo Sora" y un tooltip que explique por que otras redes requieren la forma IH58.
- Guardrails: si el discriminante de cadena activo no es la asignacion de Nexus, rechaza generar la direccion comprimida y dirige a la persona de vuelta a IH58.
- Telemetria: registra con que frecuencia se solicita y se copia la forma comprimida para que el playbook de incidentes detecte picos de comparticion accidental.

## Quality gates

- Extiende las pruebas UI automatizadas (o suites de a11y en storybook) para afirmar que los componentes de direcciones exponen la metadata ARIA requerida y que los mensajes de rechazo por IME aparecen.
- Incluye escenarios de QA manual para entrada IME (kana, pinyin), pase de lector de pantalla (VoiceOver/NVDA) y copia de QR en temas de alto contraste antes del release.
- Refleja estas comprobaciones en las checklists de release junto a las pruebas de paridad IH58 para que las regresiones sigan bloqueadas hasta corregirse.
