<!-- Japanese translation of docs/source/samples/runtime_abi_active.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/runtime_abi_active.md
status: complete
translator: manual
---

# ランタイム ABI — アクティブなバージョン（Torii）

エンドポイント
- `GET /v1/runtime/abi/active`

レスポンス（初回リリース・単一 ABI）
```json
{
  "abi_version": 1
}
```

備考
- 初回リリースでは ABI バージョンは 1 に固定されているため、このエンドポイントは常に `1` を返します。
