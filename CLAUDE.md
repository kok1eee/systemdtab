# systemdtab (sdtab)

systemd timer を crontab のように簡単に管理する Rust CLI ツール。

## コマンド

| コマンド | 機能 |
|---------|------|
| `sdtab init` | linger 有効化 + ディレクトリ作成 |
| `sdtab add "<schedule>" "<command>"` | タイマー追加 |
| `sdtab list` | 管理中タイマー一覧 |
| `sdtab remove <name>` | タイマー削除 |

## アーキテクチャ

```
src/
├── main.rs        # CLI定義（clap derive）
├── cron.rs        # cron式 → OnCalendar変換
├── unit.rs        # .service/.timer ファイル生成
├── systemctl.rs   # systemctl --user ラッパー
├── init.rs        # sdtab init
├── add.rs         # sdtab add
├── list.rs        # sdtab list
└── remove.rs      # sdtab remove
```

## 設計方針

- ユーザーレベルタイマー（`--user`）のみ。システムレベルは対象外
- 管理ユニットには `sdtab-` プレフィックスを付与
- 元の cron 式は `.service` ファイルのコメント（`# sdtab:cron=...`）に保存
- cron パーサーは自前実装（依存最小化）
- 依存: `clap` + `anyhow` のみ

## ビルド・テスト

```bash
cargo build
cargo test        # cron パーサーと unit 生成のテスト
cargo build --release
```
