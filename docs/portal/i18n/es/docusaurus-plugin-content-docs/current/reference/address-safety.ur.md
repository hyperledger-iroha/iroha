---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Dirección de seguridad y accesibilidad
descripción: Direcciones Iroha کو محفوظ انداز میں پیش کرنے اور شیئر کرنے کیلئے Requisitos de UX (ADDR-6c).
---

یہ صفحہ Entrega de documentación ADDR-6c کو ظاہر کرتا ہے۔ ان پابندیوں کو کو billeteras, exploradores, herramientas SDK, اور کسی بھی superficie del portal پر لاگو کریں جو direcciones de cara humana کو render یا aceptar کرتا ہو۔ modelo de datos canónico `docs/account_structure.md` میں ہے؛ نیچے دیا گیا lista de verificación بتاتا ہے کہ ان formatos کو seguridad یا accesibilidad کو متاثر کئے بغیر کیسے exponer کیا جائے۔

## Flujos de intercambio seguro

- ہر acción de copiar/compartir کی dirección predeterminada کو I105 بنائیں۔ dominio resuelto کو contexto de soporte کے طور پر دکھائیں تاکہ cadena de suma de verificación سامنے رہے۔
- Capacidad de “Compartir” فراہم کریں جو dirección completa en texto plano اور اسی carga útil سے بنے Código QR کو paquete کرے۔ los usuarios confirman کرنے سے پہلے دونوں inspeccionan کرنے دیں۔
- جب جگہ کم ہو (tarjetas pequeñas, notificaciones), prefijo legible por humanos رکھیں، elipses دکھائیں، اور آخری 4–6 caracteres برقرار رکھیں تاکہ ancla de suma de comprobación قائم رہے۔ truncamiento کے بغیر copia de cadena completa کرنے کیلئے toque/atajo de teclado دیں۔
- desincronización del portapapeles کو روکنے کیلئے brindis de confirmación دیں جو بالکل وہی Vista previa de cadena I105 کرے جو copia ہوئی۔ Telemetría, intentos de copia, acciones de compartir, recuento, regresiones de UX, etc.

## IME y salvaguardias de entrada- campos de dirección میں entrada no ASCII rechazar کریں۔ جب Artefactos de composición IME (ancho completo, Kana, marcas de tono) ظاہر ہوں تو advertencia en línea دکھائیں جو بتائے کہ دوبارہ کوشش سے پہلے teclado کو entrada latina پر کیسے لایا جائے۔
- Zona de pegado de texto sin formato, combinación de marcas, espacios en blanco, espacios ASCII, validación de validación. اس سے usuarios IME بند کرنے پر بھی progreso نہیں کھوتے۔
- ensambladores de ancho cero, selectores de variación, puntos de código Unicode sigilosos y validación de puntos registro de categoría de punto de código rechazado کریں تاکہ fuzzing suites importación de telemetría کر سکیں۔

## Expectativas de tecnología de asistencia

- ہر bloque de direcciones کو `aria-label` یا `aria-describedby` سے anotar کریں جو prefijo legible por humanos کرے اور carga útil کو 4–8 grupos de caracteres میں trozo کرے (“ih guión b tres dos…”). اس سے lectores de pantalla بے معنی کرداروں کی لڑی نہیں بولتے۔
- eventos exitosos de copiar/compartir کو actualización educada de la región en vivo کے ذریعے anunciar کریں۔ destino (portapapeles, hoja para compartir, QR) شامل کریں تاکہ usuario کو foco بدلے بغیر acción مکمل ہونے کا پتا ہو۔
- Vistas previas QR del texto descriptivo `alt` دیں (مثال: “Dirección I105 para `<account>` en la cadena `0x1234`”). کم بصارت والے usuarios کیلئے Lienzo QR کے ساتھ Reserva “Copiar dirección como texto” دیں۔

## Direcciones comprimidas solo de Sora- Puerta: cadena comprimida `i105` کو confirmación explícita کے پیچھے چھپائیں۔ confirmación میں دہرائیں کہ یہ formulario صرف Sora Nexus cadenas پر کام کرتی ہے۔
- Etiquetado: ہر ocurrencia میں واضح Insignia “solo Sora” اور información sobre herramientas دیں جو بتائے کہ دوسری redes کو Formulario I105 کیوں چاہیے۔
- Guardrails: اگر cadena activa discriminante Nexus asignación نہ ہو تو dirección comprimida generar کرنے سے مکمل انکار کریں اور usuario کو I105 پر واپس بھیجیں۔
- Telemetría: forma comprimida کے solicitud/copia کی frecuencia ریکارڈ کریں تاکہ libro de estrategias de incidentes picos compartidos accidentales کو detectar کر سکے۔

## Puertas de calidad

- pruebas de UI automatizadas (یا storybook a11y suites) کو بڑھائیں تاکہ componentes de dirección مطلوبہ Exposición de metadatos ARIA کریں اور Mensajes de rechazo de IME ظاہر ہوں۔
- escenarios de control de calidad manual میں entrada IME (kana, pinyin), paso de lector de pantalla (VoiceOver/NVDA), اور temas de alto contraste پر copia QR شامل کریں، lanzamiento سے پہلے۔
- Verificaciones, listas de verificación de lanzamiento, pruebas de paridad I105, regresiones bloqueadas, controles