---
lang: he
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_testnet_operator_kit/04-telemetry.md -->

# דרישות טלמטריה

## יעדי Prometheus

בצעו scrape ל-relay ול-orchestrator עם התוויות הבאות:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## דשבורדים נדרשים

1. `dashboards/grafana/soranet_testnet_overview.json` *(טרם פורסם)* - טענו את ה-JSON וייבאו את המשתנים `region` ו-`relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(נכס קיים מ-SNNet-8)* - ודאו שלוחות ה-privacy bucket מוצגות ללא פערים.

## כללי התראה

הספים חייבים להתאים לציפיות ה-playbook:

- עלייה של `soranet_privacy_circuit_events_total{kind="downgrade"}` > 0 במשך 10 דקות מפעילה `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 במשך 30 דקות מפעילה `warning`.
- `up{job="soranet-relay"}` == 0 למשך 2 דקות מפעילה `critical`.

טענו את הכללים ל-Alertmanager עם המקלט `testnet-t0`; אמתו עם `amtool check-config`.

## הערכת מדדים

אגרו snapshot של 14 ימים והעבירו אותו לוולידטור SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- החליפו את קובץ הדוגמה ב-snapshot המיוצא שלכם בעת שימוש בנתונים חיים.
- תוצאה `status = fail` חוסמת קידום; תקנו את הבדיקות המסומנות לפני ניסיון חוזר.

## דיווח

בכל שבוע העלו:

- צילומי שאילתות (`.png` או `.pdf`) שמציגים יחס PQ, שיעור הצלחת מעגלים והיסטוגרמת פתרון PoW.
- פלט כלל רישום Prometheus עבור `soranet_privacy_throttles_per_minute`.
- תיאור קצר של כל התראה שנורתה ושל צעדי המיתון (כולל timestamps).

</div>
