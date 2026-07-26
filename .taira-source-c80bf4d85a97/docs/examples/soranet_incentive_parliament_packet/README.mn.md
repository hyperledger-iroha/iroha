---
lang: mn
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay урамшууллын парламентын багц

Энэхүү багцад Сора парламентаас батлах шаардлагатай олдворуудыг багтаасан болно
автомат буухиа төлбөр (SNNet-7):

- `reward_config.json` - Norito-цувааруулж болох урамшууллын хөдөлгүүрийн тохиргоо, бэлэн
  `iroha app sorafs incentives service init`-ээр залгих. The
  `budget_approval_id` нь удирдлагын протоколд жагсаасан хэштэй таарч байна.
- `shadow_daemon.json` - дахин тоглуулах үед ашиг хүртэгч болон бондын зураглал
  бэхэлгээ (`shadow-run`) болон үйлдвэрлэлийн демон.
- `economic_analysis.md` - 2025-10 -> 2025-11-ний шударга байдлын хураангуй
  сүүдрийн симуляци.
- `rollback_plan.md` - автомат төлбөрийг идэвхгүй болгох үйл ажиллагааны дэвтэр.
- Туслах олдворууд: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Шударга байдлын шалгалт

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Мэдээллийг УИХ-ын протоколд бичигдсэн утгатай харьцуулна уу. Баталгаажуулах
-д тайлбарласны дагуу сүүдрийн гарын үсэг
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Пакетыг шинэчилж байна

1. Шагналын жин, үндсэн төлбөрийн хэмжээ эсвэл `reward_config.json`-г шинэчил.
   зөвшөөрлийн хэш өөрчлөлт.
2. 60 хоногийн сүүдрийн симуляцийг дахин ажиллуулж, `economic_analysis.md`-г
   шинэ олдворууд болон JSON + салангид гарын үсгийн хосыг баталгаажуулна уу.
3. Шинэчилсэн багцыг ажиглалтын хяналтын самбарын хамт УИХ-д танилцуулах
   шинэчилсэн зөвшөөрөл хүсэх үед экспорт.