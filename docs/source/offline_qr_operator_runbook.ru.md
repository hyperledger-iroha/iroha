<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Автономный справочник QR-оператора

В этом модуле Runbook определены практические предустановки `ecc`/dimension/fps для шумов камеры.
средах при использовании автономного QR-транспорта.

### Рекомендуемые пресеты

| Окружающая среда | Стиль | ЕСЦ | Размерность | Шутер от первого лица | Размер куска | Паритетная группа | Заметки |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Управляемое освещение ближнего радиуса действия | `sakura` | `M` | `360` | `12` | `360` | `0` | Высочайшая пропускная способность, минимальное резервирование. |
| Типичный шум мобильной камеры | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Предпочтительный балансный пресет (`~3 KB/s`) для смешанных устройств. |
| Сильные блики, размытие изображения при движении, камеры бюджетного класса | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Более низкая пропускная способность, максимальная устойчивость к декодированию. |

### Контрольный список кодирования/декодирования

1. Кодируйте с помощью явных регуляторов транспорта.
2. Перед развертыванием проверьте с помощью циклического сканирования сканера.
3. Закрепите тот же профиль стиля в помощниках воспроизведения SDK, чтобы сохранить четность предварительного просмотра.

Пример:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Проверка цикла сканера (профиль sakura-storm 3 КБ/с)

Используйте один и тот же транспортный профиль для всех путей захвата:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Цели проверки:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Браузер/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Принятие:

- Полная реконструкция полезной нагрузки завершается успешно с одним отброшенным кадром данных на группу четности.
- Отсутствие несоответствий контрольной суммы и хеша полезной нагрузки в обычном цикле захвата.