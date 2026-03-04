<!-- Hebrew translation of docs/profile_build.md -->

---
lang: he
direction: rtl
source: docs/profile_build.md
status: complete
translator: manual
---

<div dir="rtl">

# פרופיל בנייה של `iroha_data_model`

כדי לזהות שלבי בנייה איטיים ב-`iroha_data_model`, הריצו:

```sh
./scripts/profile_build.sh
```

הסקריפט מפעיל `cargo build -p iroha_data_model --timings` ושומר דוחות ב-`target/cargo-timings/`. פתחו את `cargo-timing.html` בדפדפן ומיינו לפי משך כדי לראות אילו קרייטים או שלבים צורכים הכי הרבה זמן.

השתמשו בממצאים כדי למקד מאמצי אופטימיזציה בשלבים האיטיים.

</div>
