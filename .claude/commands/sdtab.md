---
description: "systemd timerを管理（一覧・追加・削除）"
---

# /sdtab - systemd timer 管理

sdtab CLI を使って systemd timer を管理します。

## 前提

- `sdtab` がインストール済み（`cargo install --path .` または `cp target/release/sdtab /usr/local/bin/`）
- `sdtab init` 実行済み

## 引数

$ARGUMENTS でサブコマンドを指定。省略時は一覧表示。

例:
- `/sdtab` → 一覧表示
- `/sdtab add "*/5 * * * *" "./sync.sh"` → タイマー追加
- `/sdtab remove sync` → タイマー削除
- `/sdtab status` → 全タイマーの状態確認

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

さらに各タイマーの詳細状態を取得:

```bash
systemctl --user status sdtab-<name>.timer
journalctl --user -u sdtab-<name>.service --no-pager -n 5
```

最近のログと次回実行時刻を見やすくまとめて表示。

## cron 式の参考

| 書き方 | 意味 |
|--------|------|
| `* * * * *` | 毎分 |
| `0 9 * * *` | 毎日 9:00 |
| `*/5 * * * *` | 5分ごと |
| `0 9 * * 1-5` | 平日 9:00 |
| `0 0 1 * *` | 毎月1日 0:00 |
| `@daily` | 毎日 0:00 |
| `@hourly` | 毎時 0:00 |
| `@reboot` | 起動時 |
