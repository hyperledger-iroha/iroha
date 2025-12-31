<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Seguridad y accesibilidad de direcciones
description: Requisitos de UX para presentar y compartir direcciones de Iroha con seguridad (ADDR-6c).
---

Esta página captura el entregable de documentación ADDR-6c. Aplica estas restricciones a wallets, explorers, herramientas de SDK y cualquier superficie del portal que renderice o acepte direcciones orientadas a personas. El modelo de datos canónico vive en `docs/account_structure.md`; la checklist de abajo explica cómo exponer esos formatos sin comprometer seguridad o accesibilidad.

## Flujos seguros de compartición

- Por defecto, cada acción de copiar/compartir debe usar la dirección IH-B32. Muestra el dominio resuelto como contexto de apoyo para que la cadena con checksum permanezca al frente.
- Ofrece una acción “Compartir” que incluya la dirección en texto plano y un QR derivado del mismo payload. Permite que las personas inspeccionen ambos antes de confirmar.
- Cuando el espacio obligue a truncar (tarjetas pequeñas, notificaciones), conserva el prefijo legible, muestra puntos suspensivos y retén los últimos 4–6 caracteres para que sobreviva el ancla del checksum. Provee un toque/atajo de teclado para copiar la cadena completa sin truncamiento.
- Evita la desincronización del portapapeles emitiendo un toast de confirmación que previsualice la cadena IH-B32 exacta que se copió. Donde haya telemetría, cuenta intentos de copia versus acciones de compartir para detectar regresiones de UX rápido.

## IME y salvaguardas de entrada

- Rechaza entradas no ASCII en campos de dirección. Cuando aparezcan artefactos de composición IME (full width, Kana, marcas de tono), muestra una advertencia inline que explique cómo cambiar el teclado a entrada en latín antes de reintentar.
- Provee una zona de pegado en texto plano que elimine marcas combinantes y reemplace espacios en blanco por espacios ASCII antes de validar. Esto evita que la persona pierda progreso cuando desactiva el IME a mitad de flujo.
- Endurece la validación contra zero-width joiners, variation selectors y otros puntos de código Unicode sigilosos. Registra la categoría del punto de código rechazado para que los fuzzing suites puedan importar la telemetría.

## Expectativas de tecnología asistiva

- Anota cada bloque de dirección con `aria-label` o `aria-describedby` que deletree el prefijo legible y agrupe el payload en bloques de 4–8 caracteres (“ih dash b three two …”). Esto evita que los lectores de pantalla produzcan un flujo ininteligible de caracteres.
- Anuncia los eventos de copia/compartición exitosos mediante una actualización de live region en modo polite. Incluye el destino (portapapeles, hoja de compartir, QR) para que la persona sepa que la acción se completó sin mover el foco.
- Provee texto `alt` descriptivo para las vistas previas de QR (p. ej., “Dirección IH-B32 para `<alias>@<domain>` en la cadena `0x1234`”). Incluye un fallback “Copiar dirección como texto” junto al canvas de QR para personas con baja visión.

## Direcciones comprimidas solo Sora

- Gating: oculta la cadena comprimida `snx1…` detrás de una confirmación explícita. La confirmación debe reiterar que el formato solo funciona en cadenas Sora Nexus.
- Etiquetado: cada aparición debe incluir una insignia visible “Solo Sora” y un tooltip que explique por qué otras redes requieren la forma IH-B32.
- Guardrails: si el discriminante de cadena activo no es la asignación de Nexus, rechaza generar la dirección comprimida y dirige a la persona de vuelta a IH-B32.
- Telemetría: registra con qué frecuencia se solicita y se copia la forma comprimida para que el playbook de incidentes detecte picos de compartición accidental.

## Quality gates

- Extiende las pruebas UI automatizadas (o suites de a11y en storybook) para afirmar que los componentes de direcciones exponen la metadata ARIA requerida y que los mensajes de rechazo por IME aparecen.
- Incluye escenarios de QA manual para entrada IME (kana, pinyin), pase de lector de pantalla (VoiceOver/NVDA) y copia de QR en temas de alto contraste antes del release.
- Refleja estas comprobaciones en las checklists de release junto a las pruebas de paridad IH-B32 para que las regresiones sigan bloqueadas hasta corregirse.
