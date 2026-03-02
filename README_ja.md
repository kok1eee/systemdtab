# sdtab

[English](README.md)

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
web     service  @service     node server.js        ● active
  Web Server
```

## 特徴

- **cron構文** — おなじみの `* * * * *` 形式で、systemd の OnCalendar に自動変換
- **常駐サービス** — `@service` でリスタートポリシー付きのデーモンを作成
- **リソース制限** — メモリ・CPU・I/O の上限をユニットごとに設定
- **エクスポート/インポート** — `sdtab export` で TOML に書き出し、`sdtab apply` で別マシンに復元
- **失敗通知** — ユニット失敗時に Slack webhook で通知（`OnFailure=` メカニズム）
- **色付きステータス** — `● active`（緑）、`● failed`（赤）、`○ inactive`（黄）で一目瞭然。description はグレーのサブラインで表示。パイプ時は自動で色なし
- **systemd 以外の依存なし** — データベースもデーモンも不要、ユニットファイルだけ

## インストール

```bash
# プリビルドバイナリ（Rust不要）
curl -L https://github.com/kok1eee/systemdtab/releases/latest/download/sdtab-x86_64-linux \
  -o ~/.local/bin/sdtab && chmod +x ~/.local/bin/sdtab

# または Cargo 経由
cargo install systemdtab

# またはソースからビルド
git clone https://github.com/kok1eee/systemdtab
cd systemdtab
cargo build --release
cp target/release/sdtab ~/.local/bin/
```

### 必要環境

- systemd が動作する Linux（ユーザーセッション限定 — システムレベルのユニットは非対応）
- `~/.local/bin` が `$PATH` に含まれていること

> **注意**: sdtab は **ユーザーレベル** のユニットのみを管理します（`systemctl --user`）。root 権限が必要なシステムサービスの作成・管理はできません。`loginctl enable-linger` が失敗する場合は、システム管理者にリンガーの有効化を依頼してください。

## クイックスタート

```bash
# 初期化（linger有効化、設定ディレクトリ作成）
sdtab init

# オプション: Slack 失敗通知を設定
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx"

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
| `sdtab init [--slack-webhook URL] [--slack-mention USER_ID]` | linger 有効化 + ディレクトリ作成 + Slack通知設定 |
| `sdtab add "<schedule>" "<command>" [--dry-run]` | タイマーを追加 |
| `sdtab add "@service" "<command>" [--dry-run]` | 常駐サービスを追加 |
| `sdtab list [--json] [--sort time\|name]` | 管理中のタイマー・サービス一覧（デフォルト: 次回実行時刻順） |
| `sdtab status <name>` | 詳細ステータス表示（次回5回分の実行時刻付き） |
| `sdtab edit <name>` | $EDITOR でユニットファイルを編集（下記注意参照） |
| `sdtab logs <name> [-f] [-n N]` | ログ表示（journalctl） |
| `sdtab restart <name>` | サービスを再起動 |
| `sdtab enable <name>` | タイマー・サービスを有効化 |
| `sdtab disable <name>` | 一時停止（ファイルは保持） |
| `sdtab remove <name>` | 停止・無効化してユニットファイルを削除 |
| `sdtab export [-o <file>]` | 設定を TOML でエクスポート |
| `sdtab apply <file> [--prune] [--dry-run]` | TOML から一括適用 |

> `sdtab remove` は実行中のユニットを停止・無効化してからファイルを削除します。`sdtab apply --prune` は `sdtab-` プレフィックス付きのユニットのみを削除対象とし、手動で作成した systemd ユニットには影響しません。

> **`sdtab edit` の注意**: sdtab はメタデータ（元の cron 式、コマンドなど）をサービスファイルの先頭にコメントとして保存しています（`# sdtab:cron=`, `# sdtab:command=`）。編集時にこのコメントを削除すると、`sdtab list` や `sdtab export` で該当フィールドが `?` 表示になります。ユニット自体の動作には影響しません。

> `sdtab apply` は変更されたユニットを**停止せずにファイルを上書き → daemon-reload** で更新します。restart が必要なユニットだけを再起動します: サービスの description のみの変更は restart をスキップ、タイマーのサービス側変更（コマンド、env など）は次回トリガーで反映されるため timer の restart は不要です。

> `sdtab init` は [Claude Code](https://docs.anthropic.com/en/docs/claude-code) のスキルファイルを `~/.claude/commands/sdtab.md` にインストールし、どのプロジェクトからでも `/sdtab` コマンドを使えるようにします。

## スケジュール構文

標準的な cron 式と便利なショートカット:

| 式 | 意味 |
|---|------|
| `*/5 * * * *` | 5分ごと |
| `0 9 * * *` | 毎日 9:00 |
| `0 9 * * Mon-Fri` | 平日 9:00 |
| `@daily` | 1日1回（深夜0時） |
| `@daily/9` | 毎日 9:00 |
| `@daily/9:30` | 毎日 9:30 |
| `@hourly` | 1時間ごと |
| `@reboot` | システム起動時 |
| `@mon/13` | 毎週月曜 13:00 |
| `@tue/18` | 毎週火曜 18:00 |
| `@weekly/Mon,Wed` | 毎週月曜と水曜 |
| `@1st/8` | 毎月1日 8:00 |
| `@20th/8` | 毎月20日 8:00 |
| `@26th/11:30` | 毎月26日 11:30 |
| `@service` | 常駐サービス（タイマーではない） |

曜日は英語略称（`@mon`, `@tue`, ..., `@sun`）、日付は英語序数（`@1st`, `@2nd`, `@20th`, `@26th`）で指定。`/` は「この時刻に」を意味する。

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
| `--exec-start-pre <cmd>` | ExecStart 前に実行するコマンド |
| `--exec-stop-post <cmd>` | プロセス停止後に実行するコマンド |
| `--log-level-max <level>` | 保存ログレベル上限（例: `warning`, `err`） |
| `--random-delay <duration>` | タイマー発火のランダム遅延（例: `5m`） |
| `--env <KEY=VALUE>` | 環境変数（複数指定可） |
| `--no-notify` | このユニットの失敗通知を無効化 |
| `--dry-run` | ユニットファイルをプレビュー（作成しない） |

## 失敗通知

Slack webhook を設定すると、ユニット失敗時に通知を受け取れます:

```bash
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx"

# ユーザーメンション付き
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx" --slack-mention "U0700J8MN3W"
```

テンプレートユニット `sdtab-notify@.service` が作成され、以降追加されるすべてのユニットに `OnFailure=sdtab-notify@%n.service` が設定されます。タイマーやサービスが失敗すると、systemd が通知ユニットを起動し、`curl` で Slack にメッセージを送信します。

特定のユニットで通知を無効化するには:

```bash
sdtab add "0 9 * * *" "./quiet-task.sh" --no-notify
```

`Sdtabfile.toml` では `no_notify = true` を使用:

```toml
[timers.quiet-task]
schedule = "0 9 * * *"
command = "./quiet-task.sh"
workdir = "/home/user"
no_notify = true
```

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

`sdtab apply Sdtabfile.toml` でファイルからユニットを一括作成できます。`--prune` を付けると sdtab 管理下のユニットでファイルにないものを削除します。

## 仕組み

sdtab は `~/.config/systemd/user/` に `sdtab-` プレフィックス付きの標準的な systemd ユニットファイルを生成します。独自のデーモンやデータベースは不要で、すべてが素の systemd です。

```
~/.config/systemd/user/
├── sdtab-backup.service    # [Service] 定義
├── sdtab-backup.timer      # [Timer] OnCalendar 付き
├── sdtab-web.service       # 常駐サービス
├── sdtab-notify@.service   # 失敗通知テンプレート（webhook 設定時）
```

メタデータはサービスファイル内のコメント（`# sdtab:type=`, `# sdtab:cron=` など）として保存されるため、外部データベースなしで元の設定を復元できます。

## 他ツールとの比較

| | sdtab | crontab | [systemd-cron](https://github.com/systemd-cron/systemd-cron) | [fcron](http://fcron.free.fr/) | [jobber](https://github.com/dshearer/jobber) |
|---|---|---|---|---|---|
| コマンド1つでタイマー作成 | Yes | Yes | No（crontab ファイル経由） | Yes | Yes |
| 常駐サービス対応 | Yes（`@service`） | No | No（oneshot のみ） | No | No |
| リソース制限（メモリ/CPU） | `--memory-max`, `--cpu-quota` | No | 手動（ユニットファイル編集） | No | No |
| 設定エクスポート/インポート | Yes（TOML） | `crontab -l`（テキスト） | No | No | No |
| 機械可読出力 | `--json` | No | No | No | No |
| バックエンド | systemd ネイティブ | crond | systemd（自動生成） | 独自デーモン | 独自デーモン |
| root 不要のユーザー実行 | Yes | Yes | システムレベル | root 必要 | root 必要 |

## テスト

cron パーサー、ユニットファイル生成、TOML シリアライズは 93 個のユニットテストでカバーされています:

```bash
cargo test
```

## AI エージェント対応

`sdtab init` は [Claude Code](https://docs.anthropic.com/en/docs/claude-code) のスキルファイルを `~/.claude/commands/sdtab.md` にインストールします。以降、どのプロジェクトからでも自然言語で systemd タイマーを管理できます:

```
You> /sdtab 毎朝9時にreport.pyを実行して
```

Claude Code が意図を解釈し、`sdtab add "0 9 * * *" "uv run ./report.py" --dry-run` で確認を求めた後、タイマーを作成します。

スキルは**グローバルに動作**します。sdtab リポジトリの中だけでなく、インストール後はどのプロジェクトの Claude Code セッションからでも `/sdtab` でタイマーやサービスの作成・管理が可能です。

その他:

- **`CLAUDE.md`** — AI エージェントが自動で読み込むプロジェクト指示書
- **`--dry-run`** — 実行前にユニットファイルをプレビュー
- **`--json`** — プログラムから扱いやすい機械可読出力

```bash
sdtab list --json
```

```json
[
  {"name":"backup","type":"timer","schedule":"0 3 * * *","command":"./backup.sh","status":"Mon 2026-03-02 03:00:00 JST"},
  {"name":"web","type":"service","schedule":"@service","command":"node index.js","status":"active"}
]
```

## ライセンス

MIT
