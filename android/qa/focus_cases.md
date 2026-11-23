# Android TV focus test cases

Where the D-pad focus must land, page by page. `run_focus_tests.py` runs every case on a device and
checks the focused element's text against **Expect**; a person can walk the same table by hand.

## How focus moves (the rules these cases check)

1. **Into a column from the side, as TiviMate does.** Left into the menu lands on the page you're
   on. Left into the categories lands on the selected category. Right from the categories lands on
   the item you were last on in that list, else the first one (Continue watching when it shows),
   never on a header button.
2. **Inside a row or grid,** Left and Right go to the item beside; at the row's end they stop.
3. **Up and Down wrap around** every list and column: Up on the first item goes to the last, Down on
   the last goes to the first (the menu, categories, channels, poster grids, Home rows, Favorites,
   Recent, Settings, the TV guide and pop-up lists). Header buttons count as the top of their
   column: Up from the first item goes to the header, Up again to the last item.
4. **Back** climbs one level: content → categories → menu → "Exit?".
5. **Hold OK** opens the options menu; its list wraps too; Back returns to the same item.

## Setup

1. Start the fixture panel: `py android/qa/fixture_panel.py` (port 8099).
2. On the emulator, add the playlist: Settings › Playlists › Add playlist › Xtream Codes, name `QA`,
   server `http://10.0.2.2:8099`, user `qa`, password `qa`. Leave it the selected playlist.
3. Run: `py android/qa/run_focus_tests.py` (all), or `py android/qa/run_focus_tests.py LV` (one group).
   On a TV with its own playlist: `DEV=<ip>:5555 py android/qa/run_focus_tests.py --record` saves
   the screenshots and what had the focus without judging names (they're the fixture's).

Keys: `UP DOWN LEFT RIGHT OK BACK HOLD` (hold = long OK), `WAIT`, and `UPx3` for three presses.
Expect: text the focused element shows (`a || b` = either, `*` = record only). Each group starts
by opening its page from the menu (`start: <menu entry> [keys]`), then runs its cases in order.

## Menu

start: Live TV

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| MN1 | `LEFT LEFT` | Live TV | Left into the menu lands on the page you're on |
| MN2 | `UP` | Home | |
| MN3 | `UP` | QA | the playlist entry is the menu's top |
| MN4 | `UP` | Settings | wraps to the bottom |
| MN5 | `DOWN` | QA | wraps to the top |
| MN6 | `DOWNx2` | Live TV | |
| MN7 | `RIGHT` | All channels | Right from the menu lands on the selected category |

## Live TV

start: Live TV

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| LV1 | `RIGHT` | News 01 | first channel, not the Search button at the same height |
| LV2 | `DOWNx2` | News 03 | |
| LV3 | `LEFT` | All channels | Left lands on the selected category |
| LV4 | `RIGHT` | News 03 | back to the channel you were on |
| LV5 | `UPx2` | News 01 | |
| LV6 | `UP` | Search \|\| TV guide | the header is the top of the column |
| LV7 | `UP` | Kids 25 | wraps to the last channel |
| LV8 | `DOWN` | News 01 | wraps to the first |
| LV9 | `LEFT` | All channels | |
| LV10 | `UP` | Kids | categories wrap (focusing a category opens it) |
| LV11 | `DOWN` | All channels | |
| LV12 | `DOWN` | News | |
| LV13 | `RIGHT` | News 01 | |
| LV14 | `LEFT` | News | |
| LV15 | `LEFT` | Live TV | |
| LV16 | `RIGHT RIGHT HOLD` | Add to favorites \|\| Remove from favorites | hold OK: options |
| LV17 | `UP` | Select multiple | the options list wraps to its last entry |
| LV18 | `DOWN` | favorites | |
| LV19 | `BACK` | News 01 | Back returns to the channel |

## TV guide

start: Live TV RIGHT UP RIGHT OK

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| GD1 | `WAIT` | Show 1. | the guide opens on the channel you were on (News 01) |
| GD2 | `DOWN` | Show 2. | |
| GD3 | `UPx2` | Earlier \|\| Now \|\| Later | the header is the top of the column |
| GD4 | `UP` | No information | wraps to the last channel (Kids 25 has no guide) |
| GD5 | `DOWN` | Show 1. | wraps to the first |
| GD6 | `BACK` | News 01 | back on the channel list |

## Movies

The fixture's films are in Action and Drama; All movies can also show Continue watching (from any
playlist's history), so the poster cases run in Action.

start: Movies DOWN

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| MV1 | `WAIT` | Action | |
| MV2 | `RIGHT` | Action 01 | first poster, not Search or Sort |
| MV3 | `RIGHTx2` | Action 03 | the item beside |
| MV4 | `LEFT` | Action 02 | |
| MV5 | `LEFT LEFT` | Action | Left from the first column: the selected category |
| MV6 | `RIGHT` | Action 01 | Action 01 was the last poster focused |
| MV7 | `DOWN` | !Action 01 | the row below |
| MV8 | `LEFT RIGHT` | !Action 01 | back to the poster you were on, not the first |
| MV9 | `UPx2` | Search \|\| Provider order \|\| rating | the header is the top of the column |
| MV10 | `UP` | Action 30 | wraps to the last poster |
| MV11 | `DOWN` | Action 01 | wraps to the first |
| MV12 | `HOLD` | Add to favorites \|\| Remove from favorites | |
| MV13 | `UP` | Select multiple | the options list wraps to its last entry |
| MV14 | `BACK` | Action 01 | |
| MV15 | `LEFT DOWN` | Drama | Action 01 is in the first column: one Left reaches the categories |
| MV16 | `DOWN` | All movies | categories wrap |
| MV17 | `RIGHT` | !Search | All movies: Continue watching or the first poster |
| MV18 | `LEFT LEFT` | Movies | |

## Series and details

start: Series DOWN

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| SR1 | `WAIT` | Crime | |
| SR2 | `RIGHT` | Crime 01 | |
| SR3 | `OK WAIT` | * | the series page opens |
| SR4 | `BACK WAIT` | Crime 01 | back on the poster that was opened |
| SR5 | `LEFT` | Crime | Crime 01 is in the first column |
| SR6 | `UP RIGHT` | !Search | All series: Continue watching or the first poster |
| SR7 | `LEFT LEFT` | Series | |

## Select multiple

start: Movies DOWN

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| SM1 | `RIGHT HOLD UP OK` | Action 01 \|\| Selected | Select multiple ticks the held poster |
| SM2 | `RIGHT OK` | Selected | OK ticks the next one |
| SM3 | `HOLD` | Untick this one | the options now act on the selection |
| SM4 | `DOWN` | Add 2 to favorites | |
| SM5 | `BACK BACK` | !Selected | Back stops selecting: the ticks go |
| SM6 | `OK WAIT BACK WAIT` | Action 02 | not selecting: OK opens the film again |

## Home

start: Home

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| HM1 | `WAIT` | * | the first row (the big card or the first row of posters) |
| HM2 | `UP` | * | wraps to the last row |
| HM3 | `DOWN` | * | wraps back to the first row |
| HM4 | `LEFT LEFT` | Home | |

## Settings

start: Settings

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| ST1 | `WAIT` | Playlists | |
| ST2 | `UP` | About | wraps to the last page |
| ST3 | `DOWN` | Playlists | wraps to the first |
| ST4 | `DOWN` | Free TV | |
| ST5 | `RIGHT` | Free TV channels | the page's first row |
| ST6 | `LEFT` | Free TV | back on the page you came from |
| ST7 | `LEFT` | Settings | |

## Favorites and Recent

start: Recent

| ID | Keys | Expect | Why |
|----|------|--------|-----|
| RC1 | `WAIT` | * | the first item, or the filters when the list is empty |
| RC2 | `LEFT LEFT` | Recent | |
| RC3 | `UP` | Favorites | |
| RC4 | `OK WAIT` | * | |
| RC5 | `LEFT LEFT` | Favorites | |
