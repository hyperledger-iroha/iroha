---
lang: mn
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

1. **Илрүүлэх**
   - Анхааруулга `soranet_privacy_circuit_events_total{kind="downgrade"}` гал түймэр эсвэл
     brownout webhook нь засаглалаас үүдэлтэй.
   - 5 минутын дотор `kubectl logs soranet-relay` эсвэл systemd журналаар баталгаажуулна уу.

2. **Тогтворжуулах**
   - Хөлдөөх хамгаалалтын эргэлт (`relay guard-rotation disable --ttl 30m`).
   - Нөлөөлөлд өртсөн үйлчлүүлэгчдэд зөвхөн шууд хүчингүй болгохыг идэвхжүүлнэ
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Одоогийн нийцлийн тохиргооны хэш (`sha256sum compliance.toml`) авах.

3. **Оношлогоо**
   - Хамгийн сүүлийн үеийн лавлах агшин зураг болон релей хэмжигдэхүүнийг цуглуул:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW дарааллын гүн, тохируулагч тоолуур, GAR ангиллын огцом өсөлтийг анхаарна уу.
   - PQ дутагдал, дагаж мөрдөхгүй байх, эсвэл релений доголдол энэ үйл явдалд хүргэсэн эсэхийг тодорхойлох.

4. **Дамжуулах**
   - Засаглалын гүүрэнд (`#soranet-incident`) хураангуй болон багц хэштэй мэдэгдэнэ үү.
   - Сэрэмжлүүлэгтэй холбосон ослын тасалбарыг нээ, үүнд цагийн тэмдэг, нөлөөллийг бууруулах алхмууд орно.

5. **Сэргээх**
   - Үндсэн шалтгааныг арилгасны дараа эргэлтийг дахин идэвхжүүлнэ
     (`relay guard-rotation enable`) болон зөвхөн шууд хүчингүй болгохыг буцаана.
   - KPI-г 30 минутын турш хянах; шинэ бор толбо гарахгүй эсэхийг шалгаарай.

6. **Нас барсны дараах**
   - Захиргааны загвар ашиглан 48 цагийн дотор ослын тайланг гаргах.
   - Шинэ алдааны горим олдвол runbook-ийг шинэчил.