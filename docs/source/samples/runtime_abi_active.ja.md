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
  "active_versions": [1],
  "default_compile_target": 1
}
```

備考
- 返却されるリストは昇順でソートされています。`default_compile_target` はアクティブな最大バージョンであり、初回リリースでは 1 です。
