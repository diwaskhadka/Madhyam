# Changelog

From 0.9.0 on, every platform shares **one version number** (the `VERSION` file) and ships in
the same release. Each version has one section per platform; a platform with nothing new says
so. The engine (`crates/madhyam-core`) keeps its own version, given in each section.

How to cut a release and how to add a platform: [docs/RELEASING.md](docs/RELEASING.md).

## 1.1.1

Android TV: a much faster start, streams stop when you leave them, large playlists need half the memory, and fixes from a test on a TV.

### Android TV
- Leaving the player from Search, Favorites, Recent or Home, or leaving Live TV for another section, now stops the stream. Before, it kept playing hidden: the sound, the data and the provider's only connection stayed in use, so the next channel could fail with a network error.
- Home button, another app or the screen saver: the stream stops and frees the connection, then starts again when you come back (live at the live edge, films where they were). Resume points are saved as before.
- About half the memory with a large playlist: on a TCL TV with 1.8 GB of RAM and a playlist of 53 000 channels, 161 000 films and 38 000 series, 194 MB instead of 426 MB after start. Android no longer closes Madhyam a few seconds after you press Home, so you come back to where you were.
- The same playlist loads in about 12 s instead of 17 s.
- Favorites, resume positions and recently watched are saved in a small file of their own when you close the player, press Home, add a favorite or mark something watched. Before, each of these rewrote the whole catalog: for a 265 000-item playlist, a 122 MB file and about 4 s of work on a TV. The catalog is saved only when it changes (refresh, adding, editing or removing a playlist, a series' episodes loaded for the first time).
- The guide is kept from a day back to three days ahead (the TV guide shows two days ahead).
- Posters and logos: a smaller memory cache, 16-bit pictures on TVs with less than 2 GB of RAM, and the cache is emptied in the background. The video buffer is capped at a quarter of the app's memory.
- Settings › Categories no longer freezes the TV and closes Madhyam with a large playlist: the categories are listed as you scroll instead of all at once.
- Settings shows a section once the focus rests on it, so moving down the list no longer opens every section on the way. OK or Right opens a section at once, and entering a section puts the focus on its first row.
- A setting shows its new value as soon as you press OK, and pressing OK again changes it again (before, the row kept the old value until you left the page, and "Start in" moved only once however often you pressed). Changing a setting no longer redraws the whole app, and settings are saved in the background.
- The custom colour dialog fits on the screen.
- Clear watch history asks you to press OK again, like Remove playlist.
- OK during a film or episode shows the controls instead of pausing; OK again on Play / Pause pauses. While paused, the bar says "Paused" and stays on screen. The Play and Pause keys of the remote do what they say.
- Trailers open the YouTube web link first, which every YouTube app and browser accepts; some apps (SmartTube) failed on the `vnd.youtube:` link Madhyam sent before.
- Playback errors are a sentence ("Couldn't connect to this channel. The provider may be offline, or your account may be using all its connections.") with the player's code on a smaller line under it.
- Smoother scrolling in big poster grids: posters and logos are drawn without a sub-composition each, and grid covers load TMDB's 342-pixel pictures instead of 600-pixel ones.
- Search: the search box has the focus when you open Search, and OK on it opens the keyboard without typing a "q". The Live TV, Movies and Series chips (and the other filter chips) show when they are on: filled with a tick.
- Ratings show one decimal ("8.2", not "8.227").
- Episode titles leave out the series name and episode code providers repeat in them: "S01E01 · Dom Chthona" in the player and on Home, "1. Dom Chthona" on the details page.
- Providers list a film once per version ("4K-TOP - …", "TOP - …"): with titles under covers off, each poster shows its tag, so the look-alikes are told apart.
- The details page keeps a place for the Play button while the episodes load, so the page no longer jumps and the focus stays put; the focused episode shows its title, and two-line titles no longer overlap.
- Live TV: the focused channel's name scrolls to show all of it, its number is readable on the highlight, rows fade out at the top and bottom of the list, the channel count is no longer cut off, and the info under the preview follows the playing channel when the focus moves onto the preview.
- The TV guide opens at the channel that was playing (or the focused one), not at the first row.
- The open navigation rail dims the page behind it, so cut-off category names no longer show beside it.
- Secondary text and values in focused rows (Settings, search results, the playlist list) are readable on the light highlight.
- The focused poster's two-line title has normal line spacing, and the player's hint bar uses plain arrows instead of coloured emoji.

- **Faster start:** a large playlist opens in about 5 s on a TV instead of 15 s. The pages show as soon as the catalog is in; the guides load behind them and now/next fills in a few seconds later.
- **New loading screen:** the logo, the name and a progress bar timed from the previous start, instead of a plain "Loading…".
- **Search shows cards:** channels as logo cards and films, series and episodes as posters, in rows like Home. The search box opens the keyboard only when you press OK on it (before, some TVs opened it on their own and the next OK typed a "q"), and Down from it goes to the filter chips.
- **Smaller covers:** Settings › Appearance › Cover size adds Extra small and Tiny (six posters a row). Home and Search rows follow the setting too.
- **Exit asks first:** Back at the top of the app asks "Exit Madhyam?", with Cancel focused.
- **OK goes into the page:** OK on a section in the left rail closes the rail and moves into that page, and OK on a category moves into its channels or posters. Entering the category column lands on the selected category, not on the one at the same height.
- Filter chips keep the focus when you turn them on or off, and after choosing a sort or a minimum rating the focus returns to that button (before, it fell back to the rail).

### Windows
- No functional changes. Uses engine 0.3.0: large playlists take less memory and load and save faster.

### Linux
- **Parental controls** (Settings › Parental controls): a 4-digit PIN (stored only as a salted hash), locked categories per playlist, automatic locking of adult categories and channels, an optional lock on Settings and playlists, and an "Allow up to" age limit. The PIN is remembered for 10 minutes; the open-lock button in the top bar locks again. Locked channels are skipped when zapping and locked items stay out of Search, Home and the "All" lists.
- **Subtitle style** like YouTube's caption options (size in points, font, text, background and window colours and opacity, edge style, position), in Settings › Playback with a live preview and from the player's subtitle menu; applies at once to embedded, file and downloaded subtitles.
- **Buy me a coffee** in Settings › About.
- Trailers play inside the app (through yt-dlp), icons are bundled with the app, and the segment highlight and overlapping chips are fixed.
- Uses engine 0.3.0: large playlists take less memory and load and save faster.

### Engine
- madhyam-core 0.3.0.
- A catalog takes less than half the memory: each item keeps its text in one allocation and shares its group name and the directories of its logo and stream URLs with the other items; guide programmes are stored packed; the search index is three flat arrays. For a 265 000-item catalog on 32-bit ARM: 78 MB instead of 176 MB, plus 33 MB instead of 55 MB for its guide.
- Snapshots are read and written without building a JSON tree for every item, so loading takes about a third less time on a TV and saving half. The file is the same.
- `import_xmltv_file` and `xmltv` take `keep_until`: programmes starting later are left out.
- Snapshots load on up to four threads and the search index is built in parallel: about a quarter faster on a PC, a third on a four-core TV.
- `import_xmltv_file` parses the guide before taking the catalog's lock, so other calls (browsing, search) go on while a guide loads. New `ParsedGuide::parse` and `Catalog::merge_guide` do the same in Rust.
- Rust API: the `Item` fields `remote_id`, `name`, `logo`, `url`, `epg_id` and `extra` are now methods (`name()`, `extra(key)`, `extras()`) with `set_*` setters, and `group` is a shared `Arc<str>`. `Catalog::now_next` and `Catalog::guide` return owned programmes. The JSON of the FFI is unchanged, except that a programme's `channel` is now its (lowercase) guide key.
- `cargo run --release --example memory -- catalog.jsonl [guide.xml]` measures how much memory a catalog takes.

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
