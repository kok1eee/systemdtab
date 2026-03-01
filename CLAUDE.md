# systemdtab (sdtab)

systemd timer と常駐サービスを crontab のように簡単に管理する Rust CLI ツール。

## コマンド

| コマンド | 機能 |
|---------|------|
| `sdtab init` | linger 有効化 + ディレクトリ作成 |
| `sdtab add "<schedule>" "<command>"` | タイマー追加 |
| `sdtab add "@service" "<command>"` | 常駐サービス追加 |
| `sdtab list` | 管理中タイマー・サービス一覧 |
| `sdtab status <name>` | 詳細ステータス表示 |
| `sdtab edit <name>` | $EDITOR でユニットファイル編集 |
| `sdtab logs <name> [-f] [-n N] [-p PRIO]` | ログ表示（journalctl） |
| `sdtab restart <name>` | サービス再起動（`@service`のみ） |
| `sdtab enable <name>` | タイマー・サービス有効化 |
| `sdtab disable <name>` | タイマー・サービス一時停止（ファイル保持） |
| `sdtab remove <name>` | タイマー・サービス削除 |

### add オプション

| オプション | 説明 |
|-----------|------|
| `--name <name>` | ユニット名（省略時はコマンドから自動生成） |
| `--workdir <path>` | 作業ディレクトリ（省略時はカレントディレクトリ） |
| `--description <text>` | 説明文 |
| `--env-file <path>` | 環境変数ファイル（`@service`のみ） |
| `--restart <policy>` | リスタートポリシー: `always`/`on-failure`/`no`（`@service`のみ、デフォルト: `always`） |
| `--memory-max <size>` | メモリ上限（例: `512M`, `1G`） |
| `--cpu-quota <percent>` | CPU使用率上限（例: `50%`, `200%`） |
| `--io-weight <N>` | I/O優先度: 1-10000（デフォルト: 100、低い=I/O抑制） |
| `--timeout-stop <duration>` | 停止タイムアウト（例: `30s`, `5m`） |
| `--exec-start-pre <cmd>` | ExecStart 前に実行するコマンド |
| `--exec-stop-post <cmd>` | プロセス停止後に実行するコマンド |
| `--log-level-max <level>` | 保存ログレベル上限（例: `warning`, `err`） |
| `--random-delay <duration>` | タイマー発火のランダム遅延（例: `5m`）。タイマーのみ |

## アーキテクチャ

```
src/
├── main.rs        # CLI定義（clap derive）
├── cron.rs        # cron式 → OnCalendar変換
├── unit.rs        # .service/.timer ファイル生成（タイマー + 常駐サービス）
├── systemctl.rs   # systemctl --user ラッパー
├── init.rs        # sdtab init
├── add.rs         # sdtab add
├── list.rs        # sdtab list
├── remove.rs      # sdtab remove
├── edit.rs        # sdtab edit
├── logs.rs        # sdtab logs
├── restart.rs     # sdtab restart
├── status.rs      # sdtab status
├── enable.rs      # sdtab enable
└── disable.rs     # sdtab disable
```

## 設計方針

- ユーザーレベルタイマー（`--user`）のみ。システムレベルは対象外
- 管理ユニットには `sdtab-` プレフィックスを付与
- メタデータは `.service` ファイルのコメントに保存（`# sdtab:type=`, `# sdtab:cron=`, `# sdtab:restart=`）
- cron パーサーは自前実装（依存最小化）
- 依存: `clap` + `anyhow` のみ

## ビルド・テスト

```bash
cargo build
cargo test        # cron パーサーと unit 生成のテスト
cargo build --release
```
