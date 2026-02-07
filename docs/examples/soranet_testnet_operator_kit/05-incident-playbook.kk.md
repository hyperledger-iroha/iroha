---
lang: kk
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Downgrade Response Playbook

1. **Анықтау**
   - `soranet_privacy_circuit_events_total{kind="downgrade"}` ескертуі өрт немесе
     Браунout webhook басқарудан триггерлер.
   - 5 минут ішінде `kubectl logs soranet-relay` немесе жүйелік журнал арқылы растаңыз.

2. **Тұрақтандыру**
   - Мұздатқыштың айналуы (`relay guard-rotation disable --ttl 30m`).
   - Зардап шеккен клиенттер үшін тек тікелей қайта анықтауды қосыңыз
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Ағымдағы сәйкестік конфигурация хэшін түсіріңіз (`sha256sum compliance.toml`).

3. **Диагностика**
   - Каталогтың соңғы суретін және реле метрикасын жинаңыз:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW кезегінің тереңдігін, дроссель есептегіштерін және GAR санатындағы өскіндерді ескеріңіз.
   - Оқиғаға PQ тапшылығы, сәйкестікті қайта анықтау немесе реле ақаулығы себеп болғанын анықтаңыз.

4. **Эскалация**
   - Басқару көпіріне (`#soranet-incident`) қорытынды және жинақ хэшімен хабарлаңыз.
   - Уақыт белгілерін және азайту қадамдарын қоса, ескертуге байланыстыратын оқиға билетін ашыңыз.

5. **Қалпына келтіру**
   - Түбірлік себеп жойылғаннан кейін, айналдыруды қайта қосыңыз
     (`relay guard-rotation enable`) және тек тікелей қайта анықтауды қайтарыңыз.
   - KPI көрсеткіштерін 30 минут бойы бақылау; жаңа қоңыр дақтар пайда болмайтынына көз жеткізіңіз.

6. **Өлгеннен кейінгі**
   - Басқару үлгісін пайдаланып, 48 сағат ішінде оқиға туралы есепті жіберіңіз.
   - Жаңа сәтсіздік режимі табылса, runbook файлдарын жаңартыңыз.