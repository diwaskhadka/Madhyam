# Releasing Madhyam

All platforms share one version and one GitHub release.

## Version

- The version lives in **`VERSION`** at the repository root (`MAJOR.MINOR.PATCH`, e.g. `0.9.0`).
- Every platform build reads it:
  - **Windows**: `windows/Madhyam.Windows/Madhyam.Windows.csproj` (`<Version>`), and the
    installer gets it from the release workflow (`/DAppVersion`).
  - **Linux**: `linux/build.rs` passes it to the app (About, `madhyam --version`), and
    `linux/packaging/package.sh` writes it into the `.deb`, the AppImage and the AppStream file.
  - **Android TV**: `android/app/build.gradle.kts` sets `versionName` to it and computes
    `versionCode = MAJOR*10000 + MINOR*100 + PATCH` (0.9.0 → 900). Android installs an update
    only when the version code grows, so never reuse or lower a version.
  - The Rust engine (`crates/madhyam-core`) keeps its own version in `Cargo.toml`; mention it in
    the changelog when it changes.
- Change the version only in `VERSION`.

## Changelog

`CHANGELOG.md` has one section per version, and one subsection per platform under it
(`### Linux`, `### Windows`, `### Android TV`, `### Engine`, …). A platform with no changes still gets a line
("No functional changes"), so every release states what happened on every platform. The
release notes on GitHub are taken from this section.

## Cutting a release

1. On a branch: set `VERSION`, add the `## X.Y.Z` section to `CHANGELOG.md`, open a PR, wait
   for CI, merge.
2. Tag the merge commit on `main` and push the tag:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
3. `.github/workflows/release.yml` then:
   - refuses to run if the tag and `VERSION` differ;
   - builds and tests every platform;
   - publishes **one public release** with the Windows installer, the Android TV APK and the
     Linux `.deb` and AppImage, and the changelog section as its notes;
   - uploads developer files (portable zip, engine DLLs, the Linux engine `.so`, jniLibs) to a
     private draft `dev-vX.Y.Z`;
   - turns all older public releases into drafts, so only the latest stays public (they keep
     their files and can be published again by hand; "Hide old releases" does the same on
     demand).

## Linux packages

The `linux` job builds `linux/` (Rust, GTK 4, libmpv) on `ubuntu-latest` and runs
`linux/packaging/package.sh`, which stages one file tree (`/usr/bin/madhyam`, desktop entry
`dev.madhyam.Madhyam.desktop`, AppStream metadata, hicolor icons rendered from
`branding/icon.svg`) and turns it into:

- `Madhyam-vX.Y.Z-linux-amd64.deb`: dependencies are worked out by `dpkg-shlibdeps`, so the
  package needs the GTK 4 and libmpv of the build system's release or newer (Ubuntu 24.04,
  Debian 13);
- `Madhyam-vX.Y.Z-linux-x86_64.AppImage`: built with linuxdeploy and its GTK plugin, which
  bundle GTK, libmpv and their libraries (not the graphics drivers), so it runs on
  distributions with glibc 2.39 or newer.

The job plays mpv test patterns in a virtual X server and fails when no video frame is drawn,
then installs the `.deb` and starts it. To build the packages by hand on Ubuntu 24.04+:

```bash
sudo apt install libgtk-4-dev libmpv-dev dpkg-dev librsvg2-bin
(cd linux && cargo build --release)
bash linux/packaging/package.sh linux/target/release/madhyam dist
```

Set `LINUXDEPLOY` to a linuxdeploy AppImage (with `linuxdeploy-plugin-gtk.sh` on `PATH`) to
get the AppImage as well. The desktop ID `dev.madhyam.Madhyam` and the data folder
`~/.local/share/Madhyam` are fixed.

## Android signing

Release APKs are signed with one permanent key, so every new version installs over the
previous one. The workflow reads it from these repository secrets:

| Secret | Contents |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | the `.jks` keystore, base64-encoded |
| `ANDROID_KEYSTORE_PASSWORD` | keystore password |
| `ANDROID_KEY_ALIAS` | key alias |
| `ANDROID_KEY_PASSWORD` | key password (for a PKCS12 keystore, the same as the keystore password) |

Keep an offline backup of the keystore and its passwords. **If the key is lost, existing
installs cannot be updated**: users would have to uninstall (losing their data) and install a
newly signed app. Without the secrets (pull requests, forks, local builds) the build falls back
to the debug key; such APKs are for testing only.

## Adding a platform

1. Make its build read `VERSION` (and derive any build number from it, as Android does).
2. Add a job to `release.yml` that builds it and uploads its user-facing file as an artifact,
   and add that file to the "public" list in the `publish` job.
3. Add a `### <Platform>` subsection to every new changelog section.
4. Add its install steps to the release notes text in `release.yml` and to `README.md`.
5. Brand it (next section).

## Branding

Name and icon sources live in [`branding/`](../branding/README.md):

- **Name:** `branding/brand.ini` (`name=Madhyam`, `native_name=माध्यम`). Make the new
  platform's build read it, as Android (`build.gradle.kts`) and Windows (the `.csproj` and the
  installer) do, instead of writing the name in its code. The launcher label uses the Latin
  name; About shows both.
- **Icon:** `branding/icon.svg` is the master; `branding/banner.svg` is the 16:9 TV banner.
  Add the platform's sizes to `branding/build.py` (or draw a vector copy, like Android's
  adaptive icon) and keep the colour tokens from `branding/README.md`.
- **IDs:** pick the platform's bundle or package ID once, before its first release, and never
  change it (Android: `dev.madhyam`). Same for install and data folder names.
- **Release file:** name it `Madhyam-…` like the others.
