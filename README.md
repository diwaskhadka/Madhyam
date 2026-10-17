# Madhyam (माध्यम) — madhyam-core

Madhyam is an IPTV player for your own playlists, on Windows and Android TV, built on one
Rust engine.

## Download

The [latest release](https://github.com/diwaskhadka/Madhyam/releases/latest) has every platform, all with the same version number:

| Platform | File | Install |
|---|---|---|
| Windows 10 / 11 (64-bit) | `Madhyam-Setup-X.Y.Z.exe` | Run it; it installs for your account and upgrades in place. |
| Android TV / Google TV (Android 5+) | `Madhyam-vX.Y.Z-androidtv.apk` | Sideload it (e.g. *Send Files to TV*, or `adb install`). Newer versions install over it. |

What changed on each platform: [CHANGELOG.md](CHANGELOG.md). How releases are made: [docs/RELEASING.md](docs/RELEASING.md). Name, icon and colours: [branding/](branding/README.md).

## The engine

A platform-independent IPTV engine in Rust. It handles M3U/M3U8 playlists, XMLTV guides and Xtream Codes panels. It keeps everything in an indexed catalog with search, now/next and the EPG grid, plus favorites, watch progress and offline snapshots.

The engine is meant to sit under any number of native UIs: Android TV, Windows, and anything later. Every platform calls it through one small C ABI.

```
┌──────────────────────┐   ┌──────────────────────┐   ┌──────────────┐
│ Android TV app       │   │ Windows app          │   │ future: iOS, │
│ Kotlin + Compose TV  │   │ C# WinUI / Avalonia  │   │ Linux, web…  │
│ Media3 / libVLC      │   │ libmpv / libVLC      │   │              │
│ OkHttp  ─────┐       │   │ HttpClient ─┐        │   │              │
└──────────────┼───────┘   └─────────────┼────────┘   └──────┬───────┘
     JNA       │ bytes         P/Invoke  │ bytes             │
┌──────────────▼─────────────────────────▼───────────────────▼───────┐
│ madhyam-core-ffi   6 C functions, JSON in / JSON out  (include/*.h)    │
├────────────────────────────────────────────────────────────────────┤
│ madhyam-core       m3u · xml · xmltv · json · xtream · catalog · time  │
│                pure std, zero dependencies, no I/O of its own      │
└────────────────────────────────────────────────────────────────────┘
```

## Two design decisions worth knowing

**The core is "sans-IO".** The core never opens a socket. It tells the host *which URLs* to fetch, and the host fetches them with its native HTTP stack: OkHttp on Android, `HttpClient` on .NET. The host then hands the bytes back.

This means proxies, custom certificates, Android's network-security config and background/Doze rules all keep working, with no TLS library compiled into Rust. See `xtream_requests`, `xtream_part` and `xtream_commit` in [docs/API.md](docs/API.md).

**The ABI is JSON over 6 C functions.** New features become new *method names*, never new exported symbols. So the native interface is identical on Windows, Android, macOS and Linux, and it never breaks. Anything that can call C can use it: Kotlin, C#, Swift, Dart/Flutter, Python, C++. Large inputs (playlists, guides) go through `iptv_import` as raw bytes, so they skip JSON encoding.

## Status

| Part | State |
|---|---|
| M3U parser | Streaming. Handles `#EXTM3U` header and EPG URLs, quoted/unquoted attributes, commas inside quotes, `#EXTGRP`, `#EXTVLCOPT` (user-agent/referrer), `#KODIPROP` (DRM), Kodi `url\|Header=…`, catch-up attributes, BOM/CRLF/invalid UTF-8 |
| XML tokenizer + XMLTV | Streaming, bounded memory, XXE-proof. Handles entities, CDATA, missing `stop`, `xmltv_ns` → `S1E2`, timezone offsets |
| Xtream | Account check, live/VOD/series lists, series episodes, short EPG (base64), `xmltv.php`, catch-up URLs. Tolerant of panels that send numbers as strings, objects instead of arrays, and so on |
| Catalog | Stable item IDs (favorites survive refreshes), groups, paging, ranked accent-insensitive prefix search, EPG matching by `tvg-id` *or* channel name, now/next, guide grid |
| User state | Favorites, progress/continue-watching, recent channels, export/merge for sync |
| Persistence | Atomic JSON-Lines snapshot for instant offline start-up |
| C ABI | Tested end-to-end from another language ([`bindings/python/e2e_test.py`](bindings/python/e2e_test.py)): local fake panel, 8-thread hammering, error codes |
| Kotlin (JNA) / C# (P/Invoke) wrappers | Used by the Android TV app (`android/`) and the Windows app (`windows/`); CI builds both |

There are 33 Rust tests, including a smoke fuzzer that feeds 20,000 mutated and random inputs through every parser. It found one real panic, which is now fixed with a regression test.

## Performance

Measured with `cargo run --release -p madhyam-core-cli -- bench 100000 50` on one 2.1 GHz Xeon core:

| Operation | Size | Time |
|---|---|---|
| M3U parse + index | 100,000 entries (20 MB) | 0.32 s |
| XMLTV parse + normalise | 1.25 M programmes (257 MB) | 3.4 s (~75 MB/s) |
| Search, top 50 | 100k items | 3.6 ms average |
| now/next for every channel | 75,000 channels | 98 ms (≈1.3 µs each) |
| Snapshot save / load | 34 MB | 0.27 s / 0.45 s |

The C library is 0.95 MB stripped.

Android TV boxes are ARM chips with far lower single-thread speed, so expect these times to be **several times slower** there. That's a guess, not measured; run the `bench` command on a real device. The 257 MB guide is a stress case; typical provider guides are smaller.

## Layout

```
crates/madhyam-core/       the engine (library, zero deps)
  src/json.rs            JSON parser/writer          ← start reading here
  src/textutil.rs        hashing, search tokens, base64
  src/time.rs            calendar maths, XMLTV timestamps
  src/m3u.rs             streaming playlist parser
  src/xml.rs             streaming XML tokenizer
  src/xmltv.rs           guide parser on top of xml.rs
  src/xtream.rs          Xtream URLs + response parsing + sync driver
  src/catalog.rs         indices, search, EPG, user state, snapshots
  tests/no_panic.rs      smoke fuzzer
crates/madhyam-core-ffi/   C ABI (cdylib + staticlib) and JSON dispatcher
crates/madhyam-core-cli/   `iptv` tool: parse-m3u, parse-epg, search, now, bench
include/madhyam_core.h     C header
bindings/              python (tested), kotlin, csharp
docs/API.md            every method, arguments and results
```

## Building

```bash
cargo test --workspace            # everything
cargo run --release -p madhyam-core-cli -- parse-m3u my.m3u
cargo run --release -p madhyam-core-cli -- now my.m3u guide.xml bbc
```

**Windows**

```bash
cargo build --release -p madhyam-core-ffi --target x86_64-pc-windows-msvc
```

This produces `madhyam_core_ffi.dll`. Put it next to your `.exe` and use `bindings/csharp/MadhyamCore.cs`.

**Android / Android TV**

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -P 21 \
    -o app/src/main/jniLibs build --release -p madhyam-core-ffi
```

Then add JNA (`net.java.dev.jna:jna:5.17.0@aar`) and `bindings/kotlin/MadhyamCore.kt`. Keep `armeabi-v7a`: many cheap TV boxes still run 32-bit Android.

`.github/workflows/ci.yml` does all of this automatically on every push: tests on Linux, Windows and macOS, the cross-language test on each, and Windows (x64 + ARM64) and Android artifacts.

## What it deliberately doesn't do

* **Video playback.** Use Media3/ExoPlayer or libVLC on Android, and libmpv or libVLC on Windows. They already are the fast native code, and Rust wouldn't speed them up.
* **HTTP and gzip.** The host handles these; see "sans-IO" above. Decompress `.xml.gz` guides before importing them.
* **UI.** Compose for TV on Android, WinUI/Avalonia on Windows.

## Roadmap, once crates.io is reachable

The sandbox this was written in had no access to crates.io, so it's dependency-free by necessity. Each item below is optional and would sit behind a cargo feature:

1. **`rusqlite`.** On-disk catalog, so a 500k-item provider doesn't have to fit in RAM on a 1 GB TV box. This is the most valuable addition.
2. **`flate2`.** Native `.xml.gz` support, for hosts that want the core to decompress.
3. **`uniffi`.** Generated, typed Kotlin/Swift bindings instead of JSON, if call overhead ever shows up in profiles.
4. **XML tokenizer allocation work.** Profiling shows about 20 % of XMLTV time in malloc/free. A zero-copy end-tag path is the next optimisation.
5. Stalker/MAG portal support.

## Suggested reading order for learning Rust

1. `json.rs`: enums, recursion, `impl From`, traits (`ToJson`), byte-level parsing.
2. `m3u.rs`: implementing `Iterator` over a generic `BufRead`, ownership with `std::mem::take`.
3. `xml.rs` then `xmltv.rs`: a state machine, error propagation with `?`.
4. `catalog.rs`: `HashMap`/`BTreeMap`, binary search via `partition_point`, borrowing rules in a larger struct.
5. `madhyam-core-ffi/src/lib.rs`: `unsafe`, raw pointers, `catch_unwind`, `Mutex`, and why each `SAFETY:` comment is true.

## License

MIT.
