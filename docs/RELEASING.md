# Releasing Madhyam

All platforms share one version and one GitHub release.

## Version

- The version lives in **`VERSION`** at the repository root (`MAJOR.MINOR.PATCH`, e.g. `0.9.0`).
- Every platform build reads it:
  - **Windows**: `windows/Madhyam.Windows/Madhyam.Windows.csproj` (`<Version>`), and the
    installer gets it from the release workflow (`/DAppVersion`).
  - **Android TV**: `android/app/build.gradle.kts` sets `versionName` to it and computes
    `versionCode = MAJOR*10000 + MINOR*100 + PATCH` (0.9.0 → 900). Android installs an update
    only when the version code grows, so never reuse or lower a version.
  - The Rust engine (`crates/madhyam-core`) keeps its own version in `Cargo.toml`; mention it in
    the changelog when it changes.
- Change the version only in `VERSION`.

## Changelog

`CHANGELOG.md` has one section per version, and one subsection per platform under it
(`### Windows`, `### Android TV`, `### Engine`, …). A platform with no changes still gets a line
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
   - publishes **one public release** with the Windows installer and the Android TV APK, and
     the changelog section as its notes;
   - uploads developer files (portable zip, engine DLLs, jniLibs) to a private draft
     `dev-vX.Y.Z`;
   - turns all older public releases into drafts, so only the latest stays public (they keep
     their files and can be published again by hand; "Hide old releases" does the same on
     demand).

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
