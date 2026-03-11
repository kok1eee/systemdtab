# systemdtab (sdtab)

systemd timer と常駐サービスを crontab のように簡単に管理する Rust CLI ツール。

## コマンド

| コマンド | 機能 |
|---------|------|
| `sdtab init [--slack-webhook URL] [--slack-mention USER_ID]` | linger 有効化 + ディレクトリ作成 + Claude Code スキルインストール + Slack通知設定 |
| `sdtab add "<schedule>" "<command>" [--dry-run]` | タイマー追加 |
| `sdtab add "@service" "<command>" [--dry-run]` | 常駐サービス追加 |
| `sdtab list [--json] [--sort time\|name]` | 管理中タイマー・サービス一覧（色付き●/○ステータス、descriptionサブライン表示） |
| `sdtab status <name>` | 詳細ステータス表示 |
| `sdtab edit <name>` | $EDITOR でユニットファイル編集 |
| `sdtab logs <name> [-f] [-n N] [-p PRIO]` | ログ表示（journalctl） |
| `sdtab restart <name>` | サービス再起動（`@service`のみ） |
| `sdtab enable <name>` | タイマー・サービス有効化 |
| `sdtab disable <name>` | タイマー・サービス一時停止（ファイル保持） |
| `sdtab remove <name>` | タイマー・サービス削除 |
| `sdtab export [-o <file>]` | 現在の設定を TOML で出力 |
| `sdtab apply <file> [--prune] [--dry-run]` | TOML から一括反映 |

### スケジュール構文

標準 cron 式に加え、可読性の高い拡張構文を使用可能。**週次・月次は拡張構文を推奨**。

| 構文 | 意味 | 備考 |
|------|------|------|
| `0 9 * * *` | 毎日 9:00 | 標準 cron 式 |
| `*/5 * * * *` | 5分ごと | 標準 cron 式 |
| `0 8,12,16,20 * * *` | 1日4回 | カンマ区切り |
| `0 1,6-20 * * *` | 1時と6-20時 | カンマ+レンジ |
| `@daily` | 毎日 0:00 | ショートカット |
| `@daily/9` | 毎日 9:00 | 拡張: 時刻指定 |
| `@daily/9:30` | 毎日 9:30 | 拡張: 分指定 |
| `@mon/13` | 毎週月曜 13:00 | 拡張: 曜日+時刻 |
| `@tue/18` | 毎週火曜 18:00 | 拡張: 曜日+時刻 |
| `@sun/0` | 毎週日曜 0:00 | 拡張: 曜日+時刻 |
| `@weekly/Mon,Wed` | 毎週月水 | 拡張: 複数曜日 |
| `@1st/8` | 毎月1日 8:00 | 拡張: 日付序数 |
| `@20th/8` | 毎月20日 8:00 | 拡張: 日付序数 |
| `@26th/11:30` | 毎月26日 11:30 | 拡張: 序数+分 |
| `@hourly` | 毎時 0:00 | ショートカット |
| `@reboot` | 起動時 | ショートカット |
| `@service` | 常駐サービス | タイマーではない |

### add オプション

| オプション | 説明 |
|-----------|------|
| `--name <name>` | ユニット名（省略時はコマンドから自動生成） |
| `--workdir <path>` | 作業ディレクトリ（省略時はカレントディレクトリ） |
| `--description <text>` | 説明文（list でサブライン表示される） |
| `--env-file <path>` | 環境変数ファイル |
| `--restart <policy>` | リスタートポリシー: `always`/`on-failure`/`no`（`@service`のみ、デフォルト: `always`） |
| `--memory-max <size>` | メモリ上限（例: `512M`, `1G`） |
| `--cpu-quota <percent>` | CPU使用率上限（例: `50%`, `200%`） |
| `--io-weight <N>` | I/O優先度: 1-10000（デフォルト: 100、低い=I/O抑制） |
| `--timeout-stop <duration>` | 停止タイムアウト（例: `30s`, `5m`） |
| `--exec-start-pre <cmd>` | ExecStart 前に実行するコマンド |
| `--exec-stop-post <cmd>` | プロセス停止後に実行するコマンド |
| `--log-level-max <level>` | 保存ログレベル上限（例: `warning`, `err`） |
| `--random-delay <duration>` | タイマー発火のランダム遅延（例: `5m`）。タイマーのみ |
| `--env <KEY=VALUE>` | 環境変数（複数指定可: `--env "PATH=..." --env "FOO=bar"`） |
| `--no-notify` | このユニットの失敗通知を無効化 |
| `--dry-run` | 生成されるユニットファイルをプレビュー（作成しない） |

### list 表示

- `● active`（緑）/ `● failed`（赤）/ `○ inactive`（黄）で色付きステータス
- `--description` 設定済みのユニットは2行目にグレーでサブライン表示
- COMMAND 列は最大40文字でトランケート（`…` で省略）
- パイプ時は自動で色無効化

### 失敗通知

- `sdtab init --slack-webhook URL [--slack-mention USER_ID]` で設定
- テンプレートユニット `sdtab-notify@.service` が生成される
- 以降追加するユニットに自動で `OnFailure=` が設定される
- `--no-notify` で個別に無効化可能

## アーキテクチャ

```
src/
├── main.rs         # CLI定義（clap derive）
├── cron.rs         # cron式 → OnCalendar変換（拡張構文含む）
├── unit.rs         # .service/.timer ファイル生成（タイマー + 常駐サービス）
├── systemctl.rs    # systemctl --user ラッパー
├── config.rs       # グローバル設定（~/.config/sdtab/config.toml）
├── init.rs         # sdtab init（Slack通知テンプレート生成含む）
├── add.rs          # sdtab add
├── parse_unit.rs   # ユニットファイル解析（共通モジュール）
├── sdtabfile.rs    # TOML シリアライズ/デシリアライズ構造体（export/apply 共有）
├── export.rs       # sdtab export
├── apply.rs        # sdtab apply
├── list.rs         # sdtab list（色付き・description サブライン）
├── remove.rs       # sdtab remove
├── edit.rs         # sdtab edit
├── logs.rs         # sdtab logs
├── restart.rs      # sdtab restart
├── status.rs       # sdtab status
├── enable.rs       # sdtab enable
└── disable.rs      # sdtab disable
```

## 設計方針

- ユーザーレベルタイマー（`--user`）のみ。システムレベルは対象外
- 管理ユニットには `sdtab-` プレフィックスを付与
- メタデータは `.service` ファイルのコメントに保存（`# sdtab:type=`, `# sdtab:cron=`, `# sdtab:restart=`, `# sdtab:command=`, `# sdtab:no-notify=true`）
- 失敗通知は systemd `OnFailure=` + テンプレートユニット `sdtab-notify@.service` で実現（Slack webhook）
- cron パーサーは自前実装（依存最小化）。拡張構文（`@mon/9`, `@1st/8` 等）もサポート
- 依存: `clap` + `anyhow` + `serde` + `toml` + `serde_json`

## ビルド・テスト

```bash
cargo build
cargo test        # cron パーサー、unit 生成、TOML シリアライズ等 93 テスト
cargo build --release
cp target/release/sdtab ~/.local/bin/  # インストール
```
