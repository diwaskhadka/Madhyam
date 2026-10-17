# Changelog

From 0.9.0 on, every platform shares **one version number** (the `VERSION` file) and ships in
the same GitHub release. Each version has one section per platform; a platform with nothing new
says so. The engine (`crates/madhyam-core`) keeps its own version, given in each section.

How to cut a release and how to add a platform: [docs/RELEASING.md](docs/RELEASING.md).

## 0.9.0

First release with one version for every platform: the Windows installer and the Android TV
app are published together.

The app has a new name, **Madhyam** (माध्यम), and a new icon: a folded ribbon play
mark in the crimson and blue of the Nepal flag. Name and icon come from one place for every
platform, [branding/](branding/README.md). Release files are now `Madhyam-Setup-X.Y.Z.exe`
and `Madhyam-vX.Y.Z-androidtv.apk`.

### Windows
- Version number aligned with the other platforms: 0.4.11 → 0.9.0.
- **Name:** window title, Start menu and desktop shortcuts, Settings > Apps, `Madhyam.exe`; About shows माध्यम too.
- **New icon** for the exe, the window, the sidebar and the installer.
- **Upgrades Madhyam 0.4.x in place:** setup replaces `Madhyam.exe` and its Start menu and desktop shortcuts, leaving one app in Settings > Apps.

### Android TV (first public release)
Built in steps 0.5 to 0.9 (never published separately), to match Windows 0.4.11:
- **Shell:** navigation rail (Home, Live TV, Movies, Series, Favorites, Recent, Search, Settings), playlist switcher, Xtream and M3U playlists, Free TV from iptv-org by category or country.
- **Live TV:** categories, channels with now/next, a preview window that keeps playing in full screen, channel zapping, a channels × time **TV guide** and **catch-up** of past programmes on Xtream archive channels.
- **Movies and Series:** poster grid with sort and minimum rating, details pages (backdrop, cast, IMDb and Rotten Tomatoes ratings, trailer), seasons and episodes with watched marks and resume, Continue watching.
- **Player:** audio track, subtitles, OpenSubtitles search, speed, picture size, audio delay and subtitle delay.
- **Home:** continue-watching banner and rows of favorites, recent channels and recently added; each row can be turned off.
- **Settings:** playlists (refresh, update guide, test connection, edit with user agent, remove, Xtream account status), appearance (dark, light or follow the TV, cover size, titles under covers), hidden categories and their order, live stream format, buffer, preferred languages, guide refresh interval, time offset and extra guide URLs, OMDb and OpenSubtitles keys.
- **Security:** Xtream and OpenSubtitles passwords are encrypted with an Android Keystore key and left out of the catalog snapshot.
- **Name and icon:** Madhyam on the launcher, माध्यम too on the Google TV banner and in About; adaptive launcher icon with a themed (monochrome) layer, and a 320×180 home-row banner.
- Package ID `dev.madhyam`.
- Android 5.0 (API 21) and later; built for ARM 32/64-bit and x86_64.

### Engine
- madhyam-core 0.2.0, unchanged.

### Releases
- The public release carries the Windows installer and the Android TV APK; developer files stay in a private draft.
- Publishing a release hides the older ones (turns them into drafts), so only the latest is public.

## Before 0.9.0: Windows only

Each Windows version had its own number; Android TV was not published.

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
- **0.4.0** dashboard, advanced search, movie and series pages, sources.
- **0.3.0** Settings page, timeline guide, light theme.
- **0.2.0** UI; single Madhyam.exe.
- **0.1.x** First Windows player (WPF + LibVLC) on the madhyam-core engine.
