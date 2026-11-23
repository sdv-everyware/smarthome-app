# smarthome-app

The smart home panel on harry (1920 x 1080): the time and the weather, the rooms as tiles (the
lights, a dimmer, the fan, the climate), the camera (a still every few seconds in its tile, live
over the whole screen from its open button; drawn by the camera-app process into two zones,
`src/camera.rs`), the intercom, the front door, the agenda (a tap on an event shows its details at
the right, with a QR code of its meeting and of its place for the phone (the host the phone will
open under the code; a code only for an https link that reads as the host it opens), and, when
the owner gave the panel write access, Accept, Maybe and Decline on an invitation and Delete on
the account's own event; the agenda's header button opens the calendar's link: Change access and
Disconnect), the Wi-Fi settings and the settings in sheets,
and a game in a dialog: DOOM, run by the doom-app process, its picture in a zone of the dialog
and the dialog's buttons its keys over it (`src/game.rs`). The status row has the game's button
only in an image with the game (`ro.vendor.everyware.game=true`, set by doom-app's
`EVERYWARE_GAME=true`). It is written against the EveryWare UI library (`vendor/sdv-everyware/ui-lib`); the
house's data comes from the dashboard bundle over binder (`src/screen_binder.rs`). How the app is built, step by step: [`docs/harry-app-anatomy.md`](docs/harry-app-anatomy.md).

## The design

`data/smarthome.dcf` is the document the app ships. `design/*-patch.js` draw the dashboard, the
sheets, the keyboard and the screens on the Library with the kiwi tools of everyware;
`design/build.sh` builds `design/smarthome.fig`, the document to open in Figma, and the
`.dcf`; both are in git and change together. The names of the
layers (`#home/*`, `#device/*`, `#panel/*`, `#wifi/*`, ...) are the contract with `src/ui_model.rs`:
rename both or neither.

## Looks

A look is another screen for the same app: a design of its own with the app's layer names, so
the code is one and the picture is another. `design/looks/<look>-patch.js` draws the screen on
the Library, the panel's own patches add the sheets, and `design/looks/<look>-theme.js`
recolours the whole file, the Library's controls too; `design/looks/build.sh <look>` builds
`design/looks/<look>.fig` and `data/looks/<look>/smarthome.dcf`.

| Look | What it is |
|---|---|
| `window` | the camera live over the whole screen, the time and one bar of glass over it |
| `poster` | only type: the hours over the minutes, the house as lines |
| `terminal` | a console in one monospaced face |
| `retro` | a desktop of the nineties: windows, a task bar |

The settings have the row Look (Display): a tap takes the next look. The app keeps the name in
the file `look` of its override directory, ends in order and its service starts it again with
`<data_path>/looks/<name>/smarthome.dcf` (`src/config.rs`, `Looks`). A look draws what it
needs: a layer the design has not is left out, and what the code writes to it goes nowhere.
A design without controls on its screen has them in the device sheet (`#device/light`, ...),
and a tap on `#camera/tap` opens the camera's full view.

The looks are not in the APEX yet: an installed panel has the first look only, and the row
does nothing there. A run from `/data/local/tmp` with `data/looks` next to the document has
them all.

## Build and run on the RB3

```bash
# from the tree's top: build (sdv_media_db845c, out_rb3), deploy and run, with the logs
source device/rb3/sdv_media_db845c/tools/env.sh
vendor/sdv-everyware/everyware/scripts/run-smarthome-rb3.sh all
```

The script pushes the binary and `data/` to `/data/local/tmp`, stops what holds the display and
starts the app with the touch mapped to the HDMI mode. The product ships the panel as the APEX
`com.everyware.smarthome` (`apex/`), started by init; the script's run replaces it until a
reboot. The key in `apex/keys/` is a test key of this public repository, as AOSP's test keys
are: a product signs its APEX with a key of its own, never with this one.

Options: `--wayland-zones` serves Wayland clients and shows them in the zones of the design (the
camera tile and its full view; the game's picture; the browser panel only in a build with the experimental browser, `EVERYWARE_EXPERIMENTAL_BROWSER=true`, which nothing on the dashboard opens) through harry's Wayland bridge; `--zone-fps <n>` caps a client's
frame rate; `--zone-plane` puts a client on a display plane of its own (a board whose display
has no planes for that, such as Cuttlefish, turns it off with `ro.vendor.everyware.zone_plane=false`;
the dashboard then draws the zone, still only on a new client frame); `--zone-redraw
timer|frames` picks what redraws the dashboard for a zone. `--touch-size WxH` maps the touch
screen's units to the display. `SMARTHOME_LOG=debug|trace` for a diagnostic run.
