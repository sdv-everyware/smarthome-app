# smarthome-app

The smart home panel on harry (1920 x 1080): the time and the weather, the rooms as tiles (the
lights, a dimmer, the fan, the climate), the camera, the intercom, the front door, the agenda, and
the Wi-Fi settings in a sheet. It is written against the EveryWare UI library
(`vendor/sdv-everyware/ui-lib`); the house's data comes from the dashboard bundle over gRPC
(`src/house_link.rs`). How the app is built, step by step: [`docs/harry-app-anatomy.md`](docs/harry-app-anatomy.md).

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
camera tile, the browser panel when the image has the experimental browser) through harry's Wayland bridge; `--zone-fps <n>` caps a client's
frame rate; `--zone-plane` puts a client on a display plane of its own (a board whose display
has no planes for that, such as Cuttlefish, turns it off with `ro.vendor.everyware.zone_plane=false`;
the dashboard then draws the zone, still only on a new client frame); `--zone-redraw
timer|frames` picks what redraws the dashboard for a zone. `--touch-size WxH` maps the touch
screen's units to the display. `SMARTHOME_LOG=debug|trace` for a diagnostic run.
