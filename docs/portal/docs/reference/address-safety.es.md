<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
translator: manual
source_hash: 582a75b7b68e86acd82b36ccacec2691d6552d45bb00e2f6fe5bed1424f2842a
source_last_modified: "2025-11-06T19:39:18.688464+00:00"
translation_last_reviewed: 2025-11-14
---

---
title: Seguridad y accesibilidad de direcciones
description: Requisitos de UX para mostrar y compartir direcciones de Iroha de forma segura (ADDR-6c).
---

Esta página recoge el entregable de documentación ADDR‑6c. Aplica estas
restricciones a wallets, exploradores, tooling de SDK y cualquier superficie del portal
que muestre o acepte direcciones orientadas a personas. El modelo de datos canónico vive
en `docs/account_structure.md`; la checklist siguiente explica cómo exponer esos
formatos sin comprometer la seguridad ni la accesibilidad.

## Flujos seguros para compartir

- Haz que cualquier acción de copiar/compartir use por defecto la dirección IH‑B32.
  Muestra el dominio resuelto como contexto de soporte para que la cadena con checksum
  quede en primer plano.
- Ofrece una acción de “Compartir” que agrupe la dirección en texto plano y un código QR
  derivados del mismo payload. Deja que la persona usuaria inspeccione ambos antes de
  confirmar.
- Cuando el espacio requiera truncar (tarjetas pequeñas, notificaciones), conserva el
  prefijo legible inicial, muestra puntos suspensivos y mantiene los últimos 4–6
  caracteres de forma que el ancla de checksum sobreviva. Proporciona un gesto de
  toque/atajo de teclado para copiar la cadena completa sin truncar.
- Evita desincronizaciones con el portapapeles mostrando un toast de confirmación que
  previsualice exactamente la cadena IH‑B32 copiada. Cuando tengas telemetría, cuenta
  intentos de copia frente a acciones de compartir para detectar regresiones de UX con
  rapidez.

## Salvaguardas para IME y entrada

- Rechaza entrada non‑ASCII en campos de dirección. Cuando aparezcan artefactos de IME
  (full width, Kana, marcas de tono), muestra un aviso inline que explique cómo cambiar
  el teclado a entrada latina antes de reintentar.
- Proporciona una zona de pegado en texto plano que elimine marcas combinadas y reemplace
  espacio en blanco por espacios ASCII antes de la validación. Esto evita que la persona
  pierda el progreso cuando desactiva el IME a mitad del flujo.
- Refuerza la validación frente a joiners de ancho cero, variation selectors y otros
  code points Unicode de tipo “sigiloso”. Registra la categoría de code point rechazada
  para que las suites de fuzzing puedan importar la telemetría.

## Expectativas para tecnologías asistivas

- Anota cada bloque de dirección con `aria-label` o `aria-describedby` que
  deletree el prefijo legible y agrupe el payload en bloques de 4–8 caracteres
  (“ih dash b three two …”). Esto evita que los lectores de pantalla generen una
  secuencia ininteligible de caracteres.
- Anuncia los eventos de copia/compartido satisfactorios mediante una live region con
  prioridad “polite”. Incluye el destino (portapapeles, hoja de compartir, QR) para que
  la persona sepa que la acción se completó sin mover el foco.
- Proporciona texto descriptivo `alt` para las previsualizaciones de QR (por ejemplo,
  “Dirección IH‑B32 para `<alias>@<domain>` en la cadena `0x1234`”). Acompaña el lienzo
  QR con una acción “Copiar dirección como texto” para personas con baja visión.

## Direcciones comprimidas solo para Sora

- Gating: oculta la cadena comprimida `snx1…` detrás de una confirmación explícita. La
  confirmación debe reiterar que esa forma solo funciona en cadenas Sora Nexus.
- Etiquetado: cada aparición debe mostrar una insignia visible “Solo Sora” y un tooltip
  que explique por qué otras redes requieren la forma IH‑B32.
- Guardarraíles: si el discriminante de cadena activo no es la asignación Nexus, rehúsa
  generar la dirección comprimida por completo y redirige a la persona de vuelta a
  IH‑B32.
- Telemetría: registra con qué frecuencia se solicita y copia la forma comprimida para
  que el playbook de incidentes detecte picos accidentales de compartido.

## Puertas de calidad

- Extiende los tests de UI automatizados (o suites de accesibilidad tipo Storybook) para
  asegurar que los componentes de dirección exponen el metadata ARIA requerido y que los
  mensajes de rechazo de IME aparecen.
- Incluye escenarios de QA manual para entrada IME (kana, pinyin), pasada con lector de
  pantalla (VoiceOver/NVDA) y copia de QR en temas de alto contraste antes de lanzar.
- Superficie estos checks en las checklists de release junto con las pruebas de paridad
  IH‑B32 para que las regresiones sigan bloqueadas hasta corregirse.
