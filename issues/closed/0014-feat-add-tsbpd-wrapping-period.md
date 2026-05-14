# TSBPD wrapping period が未実装である

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/add-tsbpd-wrapping-period

## 目的

SRT 仕様（TSBPD wrapping period, 2152-2166 行目）では、TSBPD タイムスタンプが 32-bit であるため約 01:11:35 時間ごとにラップアラウンドが発生し、その際に `TsbpdTimeBase` を再計算する必要がある。現在の実装には wrapping period の処理が一切なく、長時間のストリーミングで配信時刻計算が破綻する。

## 優先度根拠

約 71.6 分を超える長時間ストリーミングで TSBPD 配信時刻が不正になる。短時間のセッションでは影響がないため Medium。

## 根拠

> The TSBPD wrapping period starts 30 seconds before reaching the maximum timestamp value of a packet and ends once the packet with timestamp within (30, 60) seconds interval is delivered. The updated value of TsbpdTimeBase will be recalculated as:
>
> TsbpdTimeBase = TsbpdTimeBase + MAX_TIMESTAMP + 1

`MAX_TIMESTAMP = 0xFFFFFFFF` = 約 4295 秒。

## 設計方針

`ReceiverBuffer` に wrapping period の状態管理を追加する。`MAX_TIMESTAMP` 到達の 30 秒前から wrapping period を開始し、受信パケットのタイムスタンプが 30-60 秒の範囲に入った時点で `TsbpdTimeBase += MAX_TIMESTAMP + 1` を実行し wrapping period を終了する。

`TsbpdTimeBase` の基本計算は #0004 に依存する。#0004 の修正後に本 issue を実装する。

### 修正対象

1. `ReceiverBuffer` に `wrapping_period_active: bool` フィールドを追加する
2. `MAX_TIMESTAMP` 定数を定義する（`0xFFFF_FFFF`）
3. パケット受信時にタイムスタンプをチェックし、wrapping period の開始・終了・`TsbpdTimeBase` 再計算を行う

### テスト戦略

`pbt/tests/prop_receiver.rs` に wrapping period 境界のテストを追加する:
- `MAX_TIMESTAMP - 30秒` 付近のタイムスタンプで wrapping period が開始されること
- 30-60 秒範囲のタイムスタンプで `TsbpdTimeBase` が再計算されること

### 依存関係

- #0004: `TsbpdTimeBase` の基本計算に依存する。#0004 の修正後に本 issue を実装する

## 完了条件

- wrapping period の状態管理が実装されていること
- `MAX_TIMESTAMP` 到達時に `TsbpdTimeBase` が正しく再計算されること
- wrapping period 境界のテストが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

1. `MAX_TIMESTAMP` 定数 (`0xFFFF_FFFF`) とラップアラウンド期間定数を定義
2. `ReceiverBuffer` に `wrapping_period_active: bool` フィールドを追加
3. `receive()` メソッド内でタイムスタンプのラップアラウンドを検出し、`TsbpdTimeBase += MAX_TIMESTAMP + 1` を実行するロジックを追加
