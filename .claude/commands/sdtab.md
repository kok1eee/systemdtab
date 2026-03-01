---
description: "systemd timer/serviceを管理（一覧・追加・削除・エクスポート・一括適用）"
---

# /sdtab - systemd timer/service 管理

sdtab CLI を使って systemd timer と常駐サービスを管理します。

## 前提

- `sdtab` がインストール済み（`cargo install --path .` または `cp target/release/sdtab /usr/local/bin/`）
- `sdtab init` 実行済み

## 引数

$ARGUMENTS でサブコマンドを指定。省略時は一覧表示。

例:
- `/sdtab` → 一覧表示
- `/sdtab add "*/5 * * * *" "./sync.sh"` → タイマー追加
- `/sdtab add "@service" "node server.js"` → 常駐サービス追加
- `/sdtab remove sync` → タイマー削除
- `/sdtab status` → 全タイマー・サービスの状態確認
- `/sdtab export` → 現在の設定を TOML 出力
- `/sdtab apply Sdtabfile.toml` → TOML から一括反映

## 実行手順

### $ARGUMENTS が空、または "list" の場合

```bash
sdtab list
```

結果を見やすく表示。タイマーがない場合はその旨伝える。

### $ARGUMENTS が "add" で始まる場合

ユーザーの指定から schedule と command を特定して実行:

```bash
sdtab add "<schedule>" "<command>" --name <name> --workdir <workdir>
```

- `--name` 省略時はコマンドから自動生成される
- `--workdir` 省略時はカレントディレクトリ
- `@service` の場合: `--restart`, `--env-file` も指定可能

追加後、`sdtab list` で結果を表示。

### $ARGUMENTS が "remove" で始まる場合

```bash
sdtab remove <name>
```

削除後、`sdtab list` で残りの一覧を表示。

### $ARGUMENTS が "status" の場合

```bash
sdtab list
```

さらに各タイマー・サービスの詳細状態を取得:

```bash
sdtab status <name>
```

最近のログと次回実行時刻を見やすくまとめて表示。

### $ARGUMENTS が "export" の場合

```bash
sdtab export
```

現在の設定を TOML 形式で出力。ファイルに保存する場合:

```bash
sdtab export -o Sdtabfile.toml
```

### $ARGUMENTS が "apply" で始まる場合

```bash
sdtab apply <file> --dry-run  # まずドライランで確認
sdtab apply <file>            # 実際に適用
sdtab apply <file> --prune    # ファイルにないユニットも削除
```

差分表示の記号: `+` 追加, `~` 変更, `=` 変更なし, `-` 削除

## スケジュール式の参考

| 書き方 | 意味 |
|--------|------|
| `* * * * *` | 毎分 |
| `0 9 * * *` | 毎日 9:00 |
| `*/5 * * * *` | 5分ごと |
| `0 9 * * 1-5` | 平日 9:00 |
| `0 0 1 * *` | 毎月1日 0:00 |
| `@daily` | 毎日 0:00 |
| `@daily/9` | 毎日 9:00 |
| `@daily/9:30` | 毎日 9:30 |
| `@monday/9` | 毎週月曜 9:00 |
| `@1st/8` | 毎月1日 8:00 |
| `@hourly` | 毎時 0:00 |
| `@reboot` | 起動時 |
| `@service` | 常駐サービス |
