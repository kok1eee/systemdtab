# sdtab

systemd のユーザータイマーとサービスを crontab のように管理するCLIツール。

```bash
# タイマーを追加（cron構文）
sdtab add "0 9 * * *" "uv run ./report.py"

# 常駐サービスを追加
sdtab add "@service" "node server.js" --restart on-failure

# 一覧表示
sdtab list
```

```
NAME    TYPE     SCHEDULE     COMMAND               STATUS
report  timer    0 9 * * *    uv run ./report.py    Tue 2026-03-03 09:00:00 JST
web     service  @service     node server.js        active
```

## 特徴

- **cron構文** — おなじみの `* * * * *` 形式で、systemd の OnCalendar に自動変換
- **常駐サービス** — `@service` でリスタートポリシー付きのデーモンを作成
- **リソース制限** — メモリ・CPU・I/O の上限をユニットごとに設定
- **エクスポート/インポート** — `sdtab export` で TOML に書き出し、`sdtab apply` で別マシンに復元
- **systemd 以外の依存なし** — データベースもデーモンも不要、ユニットファイルだけ

## インストール

```bash
cargo install --git https://github.com/kok1eee/systemdtab
```

ソースからビルドする場合:

```bash
git clone https://github.com/kok1eee/systemdtab
cd systemdtab
cargo build --release
cp target/release/sdtab ~/.local/bin/
```

### 必要環境

- systemd が動作する Linux（ユーザーセッション）
- Rust 1.70+

## クイックスタート

```bash
# 初期化（linger有効化、設定ディレクトリ作成）
sdtab init

# 毎日9時に実行するタスクを追加
sdtab add "0 9 * * *" "./backup.sh" --name backup --memory-max 512M

# 常駐サービスを追加
sdtab add "@service" "node dist/index.js" --name web --restart on-failure --env-file .env

# 状態を確認
sdtab list
sdtab status backup

# ログを確認
sdtab logs web -f

# 設定をファイルにエクスポート
sdtab export -o Sdtabfile.toml

# ファイルから設定を適用（別マシンで）
sdtab apply Sdtabfile.toml --dry-run
sdtab apply Sdtabfile.toml
```

## コマンド一覧

| コマンド | 説明 |
|---------|------|
| `sdtab init` | linger 有効化とディレクトリ作成 |
| `sdtab add "<schedule>" "<command>" [--dry-run]` | タイマーを追加 |
| `sdtab add "@service" "<command>" [--dry-run]` | 常駐サービスを追加 |
| `sdtab list [--json]` | 管理中のタイマー・サービス一覧 |
| `sdtab status <name>` | 詳細ステータス表示（次回5回分の実行時刻付き） |
| `sdtab edit <name>` | $EDITOR でユニットファイルを編集 |
| `sdtab logs <name> [-f] [-n N]` | ログ表示（journalctl） |
| `sdtab restart <name>` | サービスを再起動 |
| `sdtab enable <name>` | タイマー・サービスを有効化 |
| `sdtab disable <name>` | 一時停止（ファイルは保持） |
| `sdtab remove <name>` | 完全に削除 |
| `sdtab export [-o <file>]` | 設定を TOML でエクスポート |
| `sdtab apply <file> [--prune] [--dry-run]` | TOML から一括適用 |

## スケジュール構文

標準的な cron 式と便利なショートカット:

| 式 | 意味 |
|---|------|
| `*/5 * * * *` | 5分ごと |
| `0 9 * * *` | 毎日 9:00 |
| `0 9 * * Mon-Fri` | 平日 9:00 |
| `@daily` | 1日1回（深夜0時） |
| `@hourly` | 1時間ごと |
| `@reboot` | システム起動時 |
| `@daily/3` | 3日ごと |
| `@weekly/Mon,Wed` | 毎週月曜と水曜 |
| `@service` | 常駐サービス（タイマーではない） |

## add オプション

| オプション | 説明 |
|-----------|------|
| `--name <name>` | ユニット名（省略時はコマンドから自動生成） |
| `--workdir <path>` | 作業ディレクトリ（省略時はカレント） |
| `--description <text>` | 説明文 |
| `--env-file <path>` | 環境変数ファイル |
| `--restart <policy>` | `always` / `on-failure` / `no`（サービスのみ、デフォルト: `always`） |
| `--memory-max <size>` | メモリ上限（例: `512M`, `1G`） |
| `--cpu-quota <percent>` | CPU使用率上限（例: `50%`, `200%`） |
| `--io-weight <N>` | I/O優先度: 1-10000（デフォルト: 100） |
| `--timeout-stop <duration>` | 停止タイムアウト（例: `30s`） |
| `--random-delay <duration>` | タイマー発火のランダム遅延（例: `5m`） |
| `--env <KEY=VALUE>` | 環境変数（複数指定可） |
| `--dry-run` | ユニットファイルをプレビュー（作成しない） |

## エクスポート形式

`sdtab export` は TOML ファイルを出力します:

```toml
[timers.backup]
schedule = "0 3 * * *"
command = "./backup.sh"
workdir = "/home/user/project"
memory_max = "512M"

[services.web]
command = "node dist/index.js"
workdir = "/home/user/app"
description = "Web Server"
restart = "on-failure"
env_file = "/home/user/.env"
```

`sdtab apply Sdtabfile.toml` でファイルからユニットを一括作成できます。`--prune` を付けるとファイルにないユニットを削除します。

## 仕組み

sdtab は `~/.config/systemd/user/` に `sdtab-` プレフィックス付きの標準的な systemd ユニットファイルを生成します。独自のデーモンやデータベースは不要で、すべてが素の systemd です。

```
~/.config/systemd/user/
├── sdtab-backup.service    # [Service] 定義
├── sdtab-backup.timer      # [Timer] OnCalendar 付き
├── sdtab-web.service       # 常駐サービス
```

メタデータはサービスファイル内のコメント（`# sdtab:type=`, `# sdtab:cron=` など）として保存されるため、外部データベースなしで元の設定を復元できます。

## AI エージェント対応

sdtab は AI コーディングエージェント（Claude Code, Cline, Devin, Cursor など）ですぐに使えるよう設計されています。

**課題**: systemd は AI エージェントにとって操作が難しいツールです。タイマー1つ作るのに、正しいフォーマットで2つのファイルを書き、正しいディレクトリに配置し、`daemon-reload` してから `enable --now` する必要があります。1つでもミスすると動きません。

**解決策**: `sdtab add "0 9 * * *" "./backup.sh"` — コマンド1つで完了。

### 同梱ファイル

- **`CLAUDE.md`** — AI エージェントが自動で読み込むプロジェクト指示書。コマンドリファレンス、アーキテクチャ、設計方針を含みます。
- **スキルファイル** — Claude Code に sdtab の使い方を教えるプロンプト（`sdtab.md`）。
- **`--dry-run`** — 実行前にユニットファイルをプレビューできます。
- **`--json`** — プログラムから扱いやすい機械可読出力。

```bash
# エージェントがユニット一覧を取得してパース
sdtab list --json
```

```json
[
  {"name":"backup","type":"timer","schedule":"0 3 * * *","command":"./backup.sh","status":"Mon 2026-03-02 03:00:00 JST"},
  {"name":"web","type":"service","schedule":"@service","command":"node index.js","status":"active"}
]
```

### 比較

| 機能 | 素の systemd | crontab | sdtab |
|-----|-------------|---------|-------|
| タイマー+サービスを1コマンドで | No（2ファイル+3コマンド） | N/A（サービス非対応） | Yes |
| AI エージェント対応 | No | 部分的 | Yes |
| 設定のエクスポート/インポート | No | No | Yes（TOML） |
| リソース制限 | 手動設定 | No | `--memory-max`, `--cpu-quota` |
| 機械可読出力 | `systemctl show`（冗長） | No | `--json` |

## ライセンス

MIT
