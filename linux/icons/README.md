# Linux app icons

The symbolic icons the Linux app draws, built into the binary (`build.rs` →
GResource), so Madhyam looks the same on every distribution and icon theme. Ubuntu 24.04's
own icon theme lacks about half of them.

They are copies of icons from the GNOME **Adwaita** icon theme (adwaita-icon-theme 50.0),
renamed `madhyam-<original name>.svg` so a system theme never replaces them.

- Copyright © the GNOME Project and Adwaita contributors.
- License: [CC-BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/) or
  [LGPL-3.0](https://www.gnu.org/licenses/lgpl-3.0.html), at your choice
  (as in adwaita-icon-theme). These files keep that license; the MIT license of the rest of
  the repository does not apply to them.
- Source: https://gitlab.gnome.org/GNOME/adwaita-icon-theme

To add one: copy the `<name>-symbolic.svg` from adwaita-icon-theme to
`scalable/actions/madhyam-<name>-symbolic.svg` and use `"madhyam-<name>-symbolic"` in the code.
