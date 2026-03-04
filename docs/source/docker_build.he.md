<!-- Hebrew translation of docs/source/docker_build.md -->

---
lang: he
direction: rtl
source: docs/source/docker_build.md
status: complete
translator: manual
---

<div dir="rtl">

# דמות Docker לבנייה

הקונטיינר מוגדר בקובץ `Dockerfile.build` ומאגד את כל תלויות שרשרת הכלים הדרושות ל-CI ולבניות ריליס מקומיות. התמונה רצה כברירת מחדל כמשתמש שאינו root, כך שפעולות Git ממשיכות לעבוד גם עם החבילה `libgit2` של Arch Linux, ללא צורך בעקיפת `safe.directory` הגלובלית.

## ארגומנטים לבנייה

- `BUILDER_USER` – שם המשתמש שנוצר בתוך הקונטיינר (ברירת מחדל: `iroha`).
- `BUILDER_UID` – מזהה המשתמש (ברירת מחדל: `1000`).
- `BUILDER_GID` – מזהה קבוצת הבסיס (ברירת מחדל: `1000`).

כאשר מחברים את סביבת העבודה מה-host, מומלץ להעביר UID/GID תואמים כדי שהאומנות שנוצרות יישארו ניתנות לכתיבה:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

ספריות שרשרת הכלים (`/usr/local/rustup`,‏ `/usr/local/cargo`,‏ `/opt/poetry`) בבעלות המשתמש שהוגדר, כך שפקודות Cargo,‏ rustup ו-Poetry ימשיכו לפעול גם לאחר שהקונטיינר יוריד הרשאות root.

## הרצת בניות

חברו את סביבת העבודה אל `/workspace` (ה-`WORKDIR` של הקונטיינר) בזמן ההרצה. למשל:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

התמונה שומרת על חברות בקבוצת `docker`, כך שפקודות Docker מקוננות (לדוגמה `docker buildx bake`) זמינות גם בזרימות CI שממפות את ה-PID וה-socket של ה-host. התאימו את מיפויי הקבוצות לפי צרכי הסביבה שלכם.

</div>
