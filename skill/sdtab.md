---
description: "systemd timer/serviceを管理（一覧・追加・削除・エクスポート・一括適用）"
---
<!-- managed by sdtab - this file is overwritten on `sdtab init` -->
<!-- customize? copy to a different filename instead of editing this file -->

# /sdtab - systemd timer/service 管理

sdtab CLI を使って systemd timer と常駐サービスを管理します。

## 前提

- `sdtab` がインストール済み（`cargo install --git https://github.com/kok1eee/systemdtab`）
- `sdtab init` 実行済み

## 引数

$ARGUMENTS でサブコマンドを指定。省略時は一覧表示。

例:
- `/sdtab` → 一覧表示
- `/sdtab 毎朝9時にreport.pyを実行して` → 自然言語でタイマー追加
- `/sdtab add "*/5 * * * *" "./sync.sh"` → タイマー追加
- `/sdtab add "@service" "node server.js"` → 常駐サービス追加
- `/sdtab remove sync` → タイマー削除
- `/sdtab status` → 全タイマー・サービスの状態確認
- `/sdtab export` → 現在の設定を TOML 出力
- `/sdtab apply Sdtabfile.toml` → TOML から一括反映

## 実行手順

### 自然言語の場合

$ARGUMENTS がスケジュール式やサブコマンドではなく自然言語の場合:
1. ユーザーの意図からスケジュールとコマンドを推定
2. `--dry-run` で確認を求める
3. 承認後に実行

例: 「毎朝9時にreport.pyを実行して」→ `sdtab add "0 9 * * *" "uv run ./report.py" --name report --dry-run`

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

#### インラインモード（推奨パターン）

`sdtab add` はコマンド文字列をインラインで渡す設計。**`cd X && cmd` を連結せず、`--workdir` でディレクトリを分離するのが綺麗**。sdtab が内部で `ExecStart` の実行ファイルをフルパス解決してくれるので、`git` や `uv` のような PATH 依存コマンドもそのまま書ける。

```bash
# 悪い例: cd を command 欄に埋め込む
sdtab add "@weekly" "cd /home/ec2-user/dotfiles && git pull --ff-only" --name dotfiles-pull

# 良い例: --workdir で分離
sdtab add "@weekly" "git pull --ff-only" \
  --workdir "/home/ec2-user/dotfiles" \
  --name "dotfiles-pull" \
  --description "Weekly pull of dotfiles"
```

後者の方が:
- 生成される unit ファイルが綺麗（`WorkingDirectory=` が独立）
- `sdtab status` / `list` で command 欄に純粋なコマンドだけが表示される
- `--dry-run` で読みやすい

複雑なロジック（条件分岐、パイプ、複数コマンド）が必要になったら、初めてシェルスクリプトに抽出する。それまではインライン + `--workdir` で十分。

#### 実行前に必ず `--dry-run`

いきなり本番 add せず、まず `--dry-run` で生成される unit ファイルを確認する:

```bash
sdtab add "@weekly" "git pull --ff-only" \
  --workdir "/home/ec2-user/dotfiles" \
  --name "dotfiles-pull" \
  --dry-run
```

`ExecStart=`、`WorkingDirectory=`、`OnCalendar=` が想定通りか確認してから `--dry-run` を外して実行。

### $ARGUMENTS が "logs" で始まる場合

```bash
sdtab logs <name>          # 直近ログ（systemd lifecycle + 子プロセス stdout）
sdtab logs <name> -f       # follow
sdtab logs <name> -n 50    # 直近50行
sdtab logs <name> -p err   # エラー以上のみ
```

生成される unit には `SyslogIdentifier=sdtab-<name>` が自動設定されているので、`sdtab logs` は systemd の Starting/Finished に加えて、実行中の子プロセスの stdout/stderr（例: `git pull` の "Already up to date." 等）も拾える。

運用ルール:
- **失敗調査**: `sdtab logs <name>` → 該当時刻の Finished 行と子プロセスの出力を確認
- **ファイルログは廃止**: 旧プロジェクトは `/tmp/*.log` を見ていたが、sdtab 管理下は journal に一本化されている

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
