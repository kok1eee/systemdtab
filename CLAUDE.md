# systemdtab (sdtab)

systemd timer と常駐サービスを crontab のように簡単に管理する Rust CLI ツール。

## コマンド

| コマンド | 機能 |
|---------|------|
| `sdtab init` | linger 有効化 + ディレクトリ作成 |
| `sdtab add "<schedule>" "<command>"` | タイマー追加 |
| `sdtab add --service "<command>"` | 常駐サービス追加 |
| `sdtab list` | 管理中タイマー・サービス一覧 |
| `sdtab remove <name>` | タイマー・サービス削除 |

### add オプション

| オプション | 説明 |
|-----------|------|
| `--service` | 常駐サービスとして登録（タイマーではなく） |
| `--name <name>` | ユニット名（省略時はコマンドから自動生成） |
| `--workdir <path>` | 作業ディレクトリ（省略時はカレントディレクトリ） |
| `--description <text>` | 説明文 |
| `--env-file <path>` | 環境変数ファイル（サービスのみ） |
| `--restart <policy>` | リスタートポリシー: `always`/`on-failure`/`no`（サービスのみ、デフォルト: `always`） |

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
└── remove.rs      # sdtab remove
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
