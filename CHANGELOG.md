# Changelog

From 0.9.0 on, every platform shares **one version number** (the `VERSION` file) and ships in
the same release. Each version has one section per platform; a platform with nothing new says
so. The engine (`crates/madhyam-core`) keeps its own version, given in each section.

How to cut a release and how to add a platform: [docs/RELEASING.md](docs/RELEASING.md).

## 1.1.0

Madhyam for Linux.

### Linux
- **New platform:** Madhyam for Linux (x86_64), as a `.deb` for Ubuntu 24.04 / Debian 13 and newer, and as an AppImage for any distribution from 2024 on.
- The same app as on Windows: Dashboard, Live TV with now / next and the guide timeline or list, Movies and Series with details, seasons and episodes, Favorites, Recently viewed, Recently added and Advanced search.
- Navy & Crimson, Light, System (follows the desktop's light or dark style) and Custom themes; the theme changes at once.
- Free TV comes preinstalled on the first start.
- Video by mpv, drawn inside the window: hardware decoding, audio and subtitle tracks, audio and subtitle delay, speed, subtitle files and OpenSubtitles search, full screen.
- Playlists, guides and settings live in `~/.local/share/Madhyam`; Xtream and OpenSubtitles passwords go to the desktop keyring (GNOME Keyring, KWallet, KeePassXC), or to a file only you can read when there is none.
- Trailers open on YouTube in your browser.

### Windows
- No functional changes.

### Android TV
- No functional changes.

### Engine
- madhyam-core 0.2.0, unchanged. Also built as `libmadhyam_core_ffi.so` for Linux (developer files).

## 1.0.0

First stable release.

### Windows
- Setup and the first start no longer carry the one-time upgrade steps of 0.9.0.

### Android TV
- No functional changes.

### Engine
- madhyam-core 0.2.0, unchanged.

## 0.10.0

New dark theme and a custom colour theme.

### Windows
- **Navy & Crimson:** the dark theme uses deep navy surfaces with crimson highlights.
- **Custom theme:** Settings › General › Visual theme › Custom, then **Pick colour…** opens a colour picker. Choose a preset (Crimson, Royal blue, Vermilion, Saffron, Green, Teal, Sky blue, Violet, Purple, Rose, Slate, Brown), set Hue, Saturation and Lightness, or type a hex code, then choose a dark or light base. A preview shows the result, and the whole theme is built from that one colour so text stays readable. It applies on the next start.

### Android TV
- **Navy & Crimson:** the same dark theme.
- **Custom theme:** Settings › Appearance › Custom colour opens a colour picker made for the remote: the same named presets, ◀ ▶ on Hue, Saturation and Lightness, a hex code and a dark or light base, with a live preview. The theme changes at once.

### Engine
- madhyam-core 0.2.0, unchanged.

## 0.9.1

Free TV comes preinstalled, and a new README.

### Windows
- **Free TV preinstalled:** on the first start, the Free TV playlist (12,000+ free iptv-org channels) is added automatically. Installs that already have playlists are left as they are, and once you turn Free TV off it stays off.

### Android TV
- **Free TV preinstalled:** a fresh install opens straight into 12,000+ free channels, by category or by country, with no setup step. Same rules as Windows for existing playlists and turning it off.

### Engine
- madhyam-core 0.2.0, unchanged.

### Project
- README as a product page: features, screenshots, remote and keyboard keys, the platforms in the pipeline, and a Buy Me a Coffee link. Engine details are in [docs/ENGINE.md](docs/ENGINE.md).
- Release notes use full links, so links from the changelog work on the release page.

## 0.9.0

First release with one version for every platform: the Windows installer and the Android TV
app are published together, with the Madhyam (माध्यम) icon: a folded ribbon play mark
in crimson and blue. Name and icon come from one place for every platform,
[branding/](branding/README.md). Release files are `Madhyam-Setup-X.Y.Z.exe` and
`Madhyam-vX.Y.Z-androidtv.apk`.

### Windows
- Version number aligned with the other platforms: 0.4.11 → 0.9.0.
- **New icon** for the exe, the window, the sidebar and the installer; About shows माध्यम too.

### Android TV (first public release)
Built in steps 0.5 to 0.8 (never published separately), to match Windows 0.4.11:
- **Shell:** navigation rail (Home, Live TV, Movies, Series, Favorites, Recent, Search, Settings), playlist switcher, Xtream and M3U playlists, Free TV from iptv-org by category or country.
- **Live TV:** categories, channels with now/next, a preview window that keeps playing in full screen, channel zapping, a channels × time **TV guide** and **catch-up** of past programmes on Xtream archive channels.
- **Movies and Series:** poster grid with sort and minimum rating, details pages (backdrop, cast, IMDb and Rotten Tomatoes ratings, trailer), seasons and episodes with watched marks and resume, Continue watching.
- **Player:** audio track, subtitles, OpenSubtitles search, speed, picture size, audio delay and subtitle delay.
- **Home:** continue-watching banner and rows of favorites, recent channels and recently added; each row can be turned off.
- **Settings:** playlists (refresh, update guide, test connection, edit with user agent, remove, Xtream account status), appearance (dark, light or follow the TV, cover size, titles under covers), hidden categories and their order, live stream format, buffer, preferred languages, guide refresh interval, time offset and extra guide URLs, OMDb and OpenSubtitles keys.
- **Security:** Xtream and OpenSubtitles passwords are encrypted with an Android Keystore key and left out of the catalog snapshot.
- **Launcher:** Madhyam on the launcher, माध्यम too on the Google TV banner and in About; adaptive launcher icon with a themed (monochrome) layer, and a 320×180 home-row banner.
- Package ID `dev.madhyam`; Android 5.0 (API 21) and later; built for ARM 32/64-bit and x86_64.

### Engine
- madhyam-core 0.2.0, unchanged.

### Releases
- The public release carries the Windows installer and the Android TV APK; developer files stay in a private draft.
- Publishing a release hides the older ones (turns them into drafts), so only the latest is public.

## Before 0.9.0: Windows only

Each Windows version had its own number; Android TV was not published yet.

- **0.4.11** Windows installer (Inno Setup); public releases carry only the installer, developer files go to a private draft.
- **0.4.10** Show / hide countries in Free TV; the Free TV rail shortcut was removed (the playlist menu already does it).
- **0.4.9** One Free TV playlist with a Categories | Countries switch in Live TV.
- **0.4.8** Playlist password encrypted (DPAPI); Free TV from iptv-org (all or by country).
- **0.4.7** QA fixes: playlist ⋮ menu, junk ratings ignored, series posters for episodes on the dashboard, clean episode labels, trailer autoplay.
- **0.4.6** Clip the scrolled player card at the page top.
- **0.4.5** Maximize-style full-screen animation; trailer initialises once; fast season watched marks.
- **0.4.4** Working overlay guard (fixes a full-screen layout loop and a sluggish UI); trailer after episodes.
- **0.4.3** One-step full screen; inline trailers; IMDb and Rotten Tomatoes ratings; playlist actions menu.
- **0.4.2** Raised player cards; playlist menu and episode watched toggle; live badge; app icon.
- **0.4.1** Pop-up menu font and floating player controls fixed.
- **0.4.0** Dashboard, advanced search, movie and series pages, sources.
- **0.3.0** Settings page, timeline guide, light theme.
- **0.2.0** New Windows UI: icon rail, playlist switcher and search, dense channel list; a single exe.
- **0.1.x** First Windows player (WPF + LibVLC) on the madhyam-core engine.
