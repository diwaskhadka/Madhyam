# Branding

The app is called **Madhyam** (माध्यम, Nepali for "medium"). The repository and the
code keep the project's code name, Madhyam (`madhyam-core`, `dev.madhyam`, `Madhyam.Windows`).

## Files

| File | What it is |
|---|---|
| `brand.ini` | The display name (Latin and Devanagari) and tagline. Every platform's build reads it; it is the only place the name is written. |
| `icon.svg` | The master app icon (512×512): a folded ribbon play mark on a white rounded square. |
| `banner.svg` | The TV banner (320×180): icon, both names and "IPTV". |
| `build.py` | Renders the SVGs into each platform's image files (see its docstring). Windows + Edge, no packages: `py branding/build.py`. |
| `concepts/` | The icon variations considered for 0.9.0 (A, flat folds on white, was chosen; B–D kept for reference). |
| `png/` | Large renders (512 px icon, 1280×720 banner) for store pages, the README and new platforms. |

## Colours

From the flag of Nepal. Use only these; do not draw the flag's pennant shape, sun or moon.

| Token | Hex | Use |
|---|---|---|
| crimson | `#DC143C` | the play mark's diagonals, accents |
| crimson shade | `#A50F2D` | fold shadow on crimson |
| blue | `#003893` | the play mark's upright, text on the banner |
| blue shade | `#00225A` | fold shadow on blue |
| white | `#FFFFFF` | icon and banner background |

Android has them as `@color/brand_*` (`android/app/src/main/res/values/colors.xml`).

## Rules

- No text in the small icon. The names belong on the TV banner and in About.
- Launcher labels, window titles and the Start menu use the Latin name; the TV banner and the
  About pages show both.
- Display name and IDs are separate. Changing `brand.ini` renames what users see, but these
  stay fixed once released, or upgrades break:
  - Android `applicationId` `dev.madhyam` (`android/app/build.gradle.kts`)
  - Windows installer `AppId` (`windows/installer/Madhyam.iss`; still Madhyam 0.4.x's id, so
    0.9.0 upgrades those installs), `Madhyam.exe`, and the data folder
    `%LOCALAPPDATA%\Madhyam` (`App.DataDir`; the app moves 0.4.x's `Madhyam` folder there)

## Where each platform uses it

| Platform | Name | Icon |
|---|---|---|
| Android TV | `app/build.gradle.kts` turns `brand.ini` into `@string/app_name` and `BuildConfig.APP_NAME` / `APP_NATIVE_NAME` (Kotlin: `Brand`). | Adaptive icon: `res/drawable-anydpi-v26/ic_launcher.xml` with `drawable/ic_launcher_foreground.xml` and `ic_launcher_monochrome.xml` (vector copies of `icon.svg`'s geometry); `drawable/ic_launcher.xml` before Android 8; TV banner `res/drawable-xhdpi/banner.png` (rendered). |
| Windows | `Madhyam.Windows.csproj` reads `brand.ini` into `Product` and the `BrandNativeName` assembly metadata (C#: `Brand`); the installer reads it with `ReadIni`. | `Assets/app.ico` (16–256 px, exe, window and installer), `Assets/app-64.png`, `app-256.png` (all rendered). |
