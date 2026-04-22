# quicssh-rs

> **これは [oowl/quicssh-rs](https://github.com/oowl/quicssh-rs) のフォークです。** 本番環境での使用に向けた**安定化と堅牢性の向上**を目的としています。

## フォークの目的

このフォークは、多様なネットワーク環境において信頼性の高いSSH-over-QUIC接続を実現するため、quicssh-rsの安定化を目指しています。

### 主な変更点

- **MTU上限の設定**: `--mtu-upper-bound`オプションでMTU探索を制御
  - **デフォルト**: 標準ネットワーク向けにQuinnのデフォルト値（1452バイト）を使用
  - **`--mtu-upper-bound safety`**: RFC 9000 §14.1およびRFC 8899 §5.1.2に準拠した保守的な1200バイト
    - VPNトンネル（Tailscale、WireGuard）やIPv6最小MTU（RFC 8200）との互換性を確保
    - 制限のあるネットワークインターフェースでのUDPパケット拒否を防止
  - **カスタム値**: 任意のMTU値を指定可能（例: `--mtu-upper-bound 1300`）
- **Linux MTU探索の修正**: Linux上でMTU探索が正しく有効になるよう`cfg`条件を修正

---

## quicssh-rsについて

> **quicssh-rs**は、クライアントやサーバーにパッチを当てることなく、QUICを使用してSSHサーバーに接続できるQUICプロキシです。

`quicssh-rs`は[quicssh](https://github.com/moul/quicssh)のRust実装です。[quinn](https://github.com/quinn-rs/quinn)と[tokio](https://github.com/tokio-rs/tokio)をベースにしています。

なぜQUICを使うのか？SSHはTCP接続環境において脆弱であり、ほとんどのSSHパケットは実際には小さいため、あらゆるネットワーク環境でSSH接続を維持するだけで使用できます。QUICは弱いネットワークに対する最適化が優れており、**コネクションマイグレーション**という重要な機能を持っているため、良い選択肢です。つまり、リモート作業中にWi-Fiネットワークを自由に切り替えても、安定したSSH接続を維持できます。

## デモ

https://user-images.githubusercontent.com/39181969/235409750-234de94a-1189-4288-93c2-45f62a9dfc48.mp4

## なぜmoshではないのか？

moshのアーキテクチャでは、制御とデータ接続をサポートするために多くのポートを開く必要があり、多くの環境であまりユーザーフレンドリーではありません。また、VSCodeのリモート開発はmoshをサポートしていません。

## セキュリティに関する注意事項

**重要**: デフォルトでは、このツールは自己署名証明書を簡単に使用できるように**QUIC証明書検証を無効化**しています。これはほとんどのSSHユースケースでは許容されます：

- SSH自体がエンドツーエンドの暗号化とホストキー検証を提供
- QUIC層はTCPと同様のトランスポートトンネルとして機能
- 主なリスクは**DNS/IPスプーフィング**によるトラフィック分析の可能性（平文の露出ではない）

**ただし、以下の点に注意してください：**

1. **QUIC証明書検証なしの場合**、DNSスプーフィングやIPルーティングのハイジャックが可能な攻撃者は以下のことができます：
   - 将来の復号試行のために暗号化トラフィックを傍受
   - トラフィック分析（タイミング、パケットサイズ）の実行
   - 中間者位置の確立（ただしSSHホストキー検証はセッションを保護）

2. **このリスクを排除するには**：
   - 計画中: 将来の`--verify-cert`フラグと適切なTLS証明書の使用（**未実装：現在のリリースでは利用不可**）
   - 信頼できるネットワーク環境でのみ展開
   - SSHのホストキー検証を主要なセキュリティ層として依存

**推奨事項**: 機密性の高い環境では、証明書検証の実装を検討するか、SSHの組み込みセキュリティ機能（ホストキーピンニング、証明書認証）を主要な防御手段として使用してください。

## アーキテクチャ

標準的なSSH接続

```
┌───────────────────────────────────────┐             ┌───────────────────────────────────┐
│                  bob                  │             │               wopr                │
│ ┌───────────────────────────────────┐ │             │ ┌───────────────────────────────┐ │
│ │           ssh user@wopr           │─┼────tcp──────┼▶│             sshd              │ │
│ └───────────────────────────────────┘ │             │ └───────────────────────────────┘ │
└───────────────────────────────────────┘             └───────────────────────────────────┘
```

---

QUICでプロキシされたSSH接続

```
┌─────────────────────────────────────┐             ┌─────────────────────────────────────┐
│                 bob                 │             │                wopr                 │
│ ┌─────────────────────────────────┐ │             │ ┌─────────────────────────────────┐ │
│ │ssh -o ProxyCommand=             │ │             │ │              sshd               │ │
│ │ "quicssh-rs client       │ │             │ └─────────────────────────────────┘ │
│ │  quic://%h:4433" user@wopr      │ │             │                  ▲                  │
│ └─────────────────────────────────┘ │             │                  │                  │
│                  │                  │             │                  │                  │
│               process               │             │     tcp to localhost:22            │
│                  │                  │             │                  │                  │
│                  ▼                  │             │                  │                  │
│ ┌─────────────────────────────────┐ │             │ ┌─────────────────────────────────┐ │
│ │quicssh-rs client         │─┼─quic (udp)─▶│ │      quicssh-rs          │ │
│ │                    wopr:4433    │ │             │ │           server                │ │
│ └─────────────────────────────────┘ │             │ └─────────────────────────────────┘ │
└─────────────────────────────────────┘             └─────────────────────────────────────┘
```

## 使い方

```console
$ quicssh-rs -h
A simple ssh server based on quic protocol

Usage: quicssh-rs <COMMAND>

Commands:
  server  Server
  client  Client
  help    Print this message or the help of the given subcommand(s)

Options:
      --log <LOG_FILE>         Location of log, Default if
      --log-level <LOG_LEVEL>  Log level, Default Error
  -h, --help                   Print help
  -V, --version                Print version
```

### クライアント

```console
$ quicssh-rs client -h
Client

Usage: quicssh-rs client [OPTIONS] <URL>

Arguments:
  <URL>  Server address

Options:
  -b, --bind <BIND_ADDR>                Client address
      --mtu-upper-bound <MTU_UPPER_BOUND>
                                        MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
  -h, --help                            Print help
  -V, --version                         Print version
```

#### クライアントSSH設定

```console
╰─$ cat ~/.ssh/config
Host test
    HostName test.test
    User root
    Port 22333
    ProxyCommand /path/to/quicssh-rs client quic://%h:%p

╰─$ ssh test
Last login: Mon May  1 13:32:15 2023 from 127.0.0.1
```

### サーバー

```console
$ quicssh-rs server -h
Server

Usage: quicssh-rs server [OPTIONS]

Options:
  -l, --listen <LISTEN>                 Address to listen on [default: 0.0.0.0:4433]
  -p, --proxy-to <PROXY_TO>             Address of the ssh server
  -F, --conf <CONF_PATH>
      --mtu-upper-bound <MTU_UPPER_BOUND>
                                        MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
  -h, --help                            Print help
  -V, --version                         Print version
```

## インストール

### GitHubリリースから

[Releases](https://github.com/aruz/quicssh-rs/releases)ページからプラットフォームに合ったバイナリをダウンロードしてください。

### Cargoから

```bash
cargo install quicssh-rs
```

### ソースからビルド

```bash
git clone https://github.com/aruz/quicssh-rs.git
cd quicssh-rs
cargo build --release
```

## ライセンス

このプロジェクトはMITライセンスの下で公開されています。

[![Powered by DartNode](https://dartnode.com/branding/DN-Open-Source-sm.png)](https://dartnode.com "Powered by DartNode - Free VPS for Open Source")
