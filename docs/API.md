# madhyam-core C ABI — method reference

All calls go through two functions:

* `iptv_call(core, method, args_json)` for normal calls
* `iptv_import(core, method, bytes, len, args_json)` when the input is a large blob (a playlist, a guide, a response body)

Each one returns `{"ok": …}` or `{"error": {"code", "message"}}`.

Some conventions apply everywhere:

* **Item IDs are strings.** A `u64` doesn't fit in a JSON number without losing precision.
* **Times are Unix seconds (UTC).**
* **Any method accepts an optional `"now"`.** It overrides the clock, which is useful for tests and replays.
* **Kinds** are `live`, `movie`, `series` and `episode`.

## Sources

| method | args | result |
|---|---|---|
| `add_m3u_source` | `name`, `location?` | `{id}` |
| `add_xtream_source` | `name`, `server`, `username`, `password`, `live_ext?` (`ts`/`m3u8`) | `{id}` |
| `sources` | – | `[{id, name, kind, location, username, epg_urls, updated_at}]` (never includes passwords) |
| `rename_source` | `source`, `name` | `null` |
| `remove_source` | `source` | `null` |
| `epg_fetch_urls` | `source` | `[url]`. **For Xtream sources this includes credentials**, so pass the URLs to your HTTP client only, never to logs or the UI. |
| `import_m3u_file` | `source`, `path` | `{items, errors, epg_urls}` |
| `import_xmltv_file` | `path`, `keep_from?` | `{channels, programmes, skipped}` |

### `iptv_import` methods

| method | args | data |
|---|---|---|
| `m3u` | `source` | playlist bytes |
| `xmltv` | `keep_from?` (default: now − 24 h) | **decompressed** XMLTV |
| `parse_m3u` | – | playlist bytes; stateless, returns `{epg_urls, entries}` |
| `user_state` | – | JSON from `export_user_state` |

## Xtream (the host does the HTTP)

1. Call `xtream_requests {source}`. It returns `[{part, url}]`, seven entries, starting with `account`.
2. For each entry, GET the `url` and pass the body to `iptv_import("xtream_part", body, {source, part})`.
   * The `account` part is validated immediately: a wrong password fails at the first request with code `api`.
3. Call `xtream_commit {source}`. It returns `{items, account, warnings}`. A list that fails to download or parse becomes a warning; the other lists still import.

Other Xtream calls:

| method | args | result |
|---|---|---|
| `series_info_url` | `id` (series) | URL. Fetch it, then `iptv_import("series_info", body, {id})`. |
| `short_epg_url` | `id` (live), `limit?` | URL. Fetch it, then `iptv_import("short_epg", body, {id})`. |
| `timeshift_url` | `id`, `start`, `duration_min?`, `server_offset?` | catch-up URL |

## Browsing

| method | args | result |
|---|---|---|
| `stats` | – | `{sources, items, programmes}` |
| `groups` | `source`, `kind` | `[{name, count}]` in provider order |
| `list` | `source`, `kind`, `group?`, `offset?`, `limit?` (100) | `[item]` |
| `item` | `id` | item plus `favorite` and `progress` |
| `episodes` | `id` (series) | `[item]`, ordered by season and episode |
| `search` | `query`, `kind?`, `source?`, `limit?` (50) | `[item]`, ranked, accent-insensitive, prefix-matching |

Item shape: `{id, source, kind, remote_id, name, group, logo, url, epg_id, archive_days, extra:{…}}`.

`extra` may contain:
* request headers: `user_agent`, `referrer`
* DRM properties: `kodi.inputstream.adaptive.license_type`, `kodi.inputstream.adaptive.license_key`
* catch-up settings: `catchup`, `catchup_source`
* VOD metadata: `container`, `plot`, `rating`, `year`, `season`, `episode`, …

## Guide

| method | args | result |
|---|---|---|
| `now_next` | `id` | `{now, next}` (programme or `null`) |
| `now_next_many` | `ids` | `{"<id>": {now, next}}`. Use one call per visible screen of channels. |
| `guide` | `ids`, `from?`, `to?` | `{"<id>": [programme]}` for the EPG grid |
| `clear_epg` | – | `null` |

Programme shape: `{channel, start, stop, title, sub_title, desc, categories, icon, episode, rating}`.

## User state

| method | args | result |
|---|---|---|
| `set_favorite` | `id`, `on?` (true) | `null` |
| `favorites` | `kind?` | `[item]`, newest first |
| `set_progress` | `id`, `position`, `duration` | `null` |
| `continue_watching` | `limit?` | `[item]` between 2 % and 95 % watched |
| `record_play` | `id` | `null` |
| `recent` | `kind?`, `limit?` | `[item]` |
| `export_user_state` | – | JSON, for backup or cross-device sync |
| `import_user_state` | `state` | `null`. Merges with existing state; the newest entry wins. |

## Persistence

| method | args | result |
|---|---|---|
| `save_snapshot` | `path` | `null`. Writes atomically to a temporary file, then renames it. Contains passwords, so keep it in app-private storage. |
| `load_snapshot` | `path` | `{items}`. Restores the full catalog with no network access. |
