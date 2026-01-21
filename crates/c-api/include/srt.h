#ifndef SHIGUREDO_SRT_H
#define SHIGUREDO_SRT_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/**
 * 接続状態
 */
typedef enum SrtConnectionState {
  SRT_CONNECTION_STATE_DISCONNECTED = 0,
  SRT_CONNECTION_STATE_INDUCTION = 1,
  SRT_CONNECTION_STATE_CONCLUSION = 2,
  SRT_CONNECTION_STATE_LISTENING = 3,
  SRT_CONNECTION_STATE_CONNECTED = 4,
  SRT_CONNECTION_STATE_CLOSING = 5,
} SrtConnectionState;

/**
 * 出力パケットタイプ
 */
typedef enum SrtOutputType {
  SRT_OUTPUT_TYPE_NONE = 0,
  SRT_OUTPUT_TYPE_SEND_PACKET = 1,
  SRT_OUTPUT_TYPE_SET_TIMER = 2,
  SRT_OUTPUT_TYPE_CLEAR_TIMER = 3,
} SrtOutputType;

/**
 * イベントタイプ
 */
typedef enum SrtEventType {
  SRT_EVENT_TYPE_NONE = 0,
  SRT_EVENT_TYPE_CONNECTED = 1,
  SRT_EVENT_TYPE_DATA_RECEIVED = 2,
  SRT_EVENT_TYPE_STATE_CHANGED = 3,
  SRT_EVENT_TYPE_ERROR = 4,
  SRT_EVENT_TYPE_DISCONNECTED = 5,
  SRT_EVENT_TYPE_KEY_REFRESH_NEEDED = 6,
} SrtEventType;

/**
 * SRT 接続ハンドル
 */
typedef struct SrtConnectionHandle SrtConnectionHandle;

/**
 * 接続オプション
 */
typedef struct SrtConnectionOptions {
  uint32_t socket_id;
  const char *passphrase;
  uint8_t key_length;
  uint16_t tsbpd_delay;
} SrtConnectionOptions;

/**
 * 出力パケット
 */
typedef struct SrtOutput {
  enum SrtOutputType output_type;
  uint8_t *data;
  uintptr_t data_len;
  uint32_t timer_id;
  uint64_t duration_micros;
} SrtOutput;

/**
 * イベント
 */
typedef struct SrtEvent {
  enum SrtEventType event_type;
  uint8_t *data;
  uintptr_t data_len;
  uint32_t message_number;
  uint32_t timestamp;
  enum SrtConnectionState state;
  uint8_t key_length;
} SrtEvent;

/**
 * バージョン文字列
 */
const char *srt_version(void);

/**
 * Caller として接続を作成
 *
 * # Safety
 * options が非 null の場合、有効なポインタであること
 */
struct SrtConnectionHandle *srt_connection_new_caller(const struct SrtConnectionOptions *options);

/**
 * Listener として接続を作成
 *
 * # Safety
 * options が非 null の場合、有効なポインタであること
 */
struct SrtConnectionHandle *srt_connection_new_listener(const struct SrtConnectionOptions *options);

/**
 * 接続を解放
 *
 * # Safety
 * conn は有効なポインタであること
 */
void srt_connection_free(struct SrtConnectionHandle *conn);

/**
 * 接続状態を取得
 *
 * # Safety
 * conn は有効なポインタであること
 */
enum SrtConnectionState srt_connection_state(const struct SrtConnectionHandle *conn);

/**
 * 接続を開始 (Caller のみ)
 *
 * # Safety
 * conn は有効なポインタであること
 */
int32_t srt_connection_connect(struct SrtConnectionHandle *conn, uint64_t now_micros);

/**
 * 受信データを処理
 *
 * # Safety
 * conn, buf は有効なポインタであること
 */
int32_t srt_connection_feed_recv_buf(struct SrtConnectionHandle *conn,
                                     const uint8_t *buf,
                                     uintptr_t len,
                                     uint64_t now_micros);

/**
 * データを送信
 *
 * # Safety
 * conn, payload は有効なポインタであること
 */
int32_t srt_connection_send(struct SrtConnectionHandle *conn,
                            const uint8_t *payload,
                            uintptr_t len,
                            uint64_t now_micros);

/**
 * 切断
 *
 * # Safety
 * conn は有効なポインタであること
 */
void srt_connection_disconnect(struct SrtConnectionHandle *conn, uint64_t now_micros);

/**
 * 出力を取得
 *
 * # Safety
 * conn, output は有効なポインタであること
 */
int32_t srt_connection_poll_output(struct SrtConnectionHandle *conn, struct SrtOutput *output);

/**
 * 出力データを解放
 *
 * # Safety
 * data は srt_connection_poll_output で取得したポインタであること
 */
void srt_output_data_free(uint8_t *data, uintptr_t len);

/**
 * イベントを取得
 *
 * # Safety
 * conn, event は有効なポインタであること
 */
int32_t srt_connection_poll_event(struct SrtConnectionHandle *conn, struct SrtEvent *event);

/**
 * イベントデータを解放
 *
 * # Safety
 * data は srt_connection_poll_event で取得したポインタであること
 */
void srt_event_data_free(uint8_t *data, uintptr_t len);

/**
 * 新しい SEK を提供 (キーリフレッシュ用)
 *
 * # Safety
 * conn, sek は有効なポインタであること
 */
int32_t srt_connection_provide_new_sek(struct SrtConnectionHandle *conn,
                                       const uint8_t *sek,
                                       uintptr_t sek_len,
                                       uint64_t now_micros);

#endif  /* SHIGUREDO_SRT_H */
