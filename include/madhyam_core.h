/*
 * madhyam-core — C ABI.  JSON in, JSON out.
 *
 * Every function returning `char*` returns a UTF-8 JSON envelope:
 *     {"ok": <result>}
 *     {"error": {"code": "io|parse|api|not_found|invalid|panic", "message": "..."}}
 * Release it with iptv_string_free(). Never free it with free().
 *
 * Thread safety: a handle may be shared between threads (internal mutex).
 * Calls block; never call from the UI thread.
 *
 * Methods: see docs/API.md.
 */
#ifndef MADHYAM_CORE_H
#define MADHYAM_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MadhyamCore MadhyamCore;

MadhyamCore   *iptv_core_new(void);
void        iptv_core_free(MadhyamCore *core);

/* args_json may be NULL or "" for "no arguments". */
char       *iptv_call(MadhyamCore *core, const char *method, const char *args_json);

/* Bulk input (playlist text, XMLTV, Xtream response bodies) without JSON-encoding it. */
char       *iptv_import(MadhyamCore *core, const char *method,
                        const uint8_t *data, size_t len, const char *args_json);

void        iptv_string_free(char *s);

/* Static string, do not free. */
const char *iptv_version(void);

#ifdef __cplusplus
}
#endif

#endif /* MADHYAM_CORE_H */
