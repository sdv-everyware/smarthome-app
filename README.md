# smarthome-app

The smart home panel on harry (1920 x 1080): the time and the weather, the rooms as tiles (the
lights, a dimmer, the fan, the climate), the camera (a still every few seconds in its tile, live
over the whole screen from its open button; drawn by the camera-app process into two zones,
`src/camera.rs`), the intercom, the front door, the agenda (a tap on an event shows its details at
the right), the Wi-Fi settings and the settings in sheets,
and a game in a dialog: DOOM, run by the doom-app process, its picture in a zone of the dialog
and the dialog's buttons its keys over it (`src/game.rs`). The status row has the game's button
only in an image with the game (`ro.vendor.everyware.game=true`, set by doom-app's
`EVERYWARE_GAME=true`). It is written against the EveryWare UI library (`vendor/sdv-everyware/ui-lib`); the
house's data comes from the dashboard bundle over binder (`src/screen_binder.rs`). How the app is built, step by step: [`docs/harry-app-anatomy.md`](docs/harry-app-anatomy.md).

## The design

`data/smarthome.dcf` is the document the app ships. `design/*-patch.js` draw the dashboard, the
sheets, the keyboard and the screens on the Library with the kiwi tools of everyware;
`design/build.sh` builds the `.fig` for a review in Figma and the `.dcf`. The names of the
layers (`#home/*`, `#device/*`, `#panel/*`, `#wifi/*`, ...) are the contract with `src/ui_model.rs`:
rename both or neither.

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
camera tile and its full view; the game's picture; the browser panel only in a build with the experimental browser, `EVERYWARE_BROWSER=true`, which nothing on the dashboard opens) through harry's Wayland bridge; `--zone-fps <n>` caps a client's
frame rate; `--zone-plane` puts a client on a display plane of its own (a board whose display
has no planes for that, such as Cuttlefish, turns it off with `ro.vendor.everyware.zone_plane=false`;
the dashboard then draws the zone, still only on a new client frame); `--zone-redraw
timer|frames` picks what redraws the dashboard for a zone. `--touch-size WxH` maps the touch
screen's units to the display. `SMARTHOME_LOG=debug|trace` for a diagnostic run.
