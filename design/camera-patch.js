// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// camera-patch.js: the camera on the dashboard, at the kiwi level: the tile's offline state and
// the full view that its open button (#camera/open) shows. Run after game-patch.js and before
// prototype-patch.js (build.sh). camera-app draws the pictures into two zones (src/camera.rs);
// this file draws what the panel shows around them and while camera-app is not there.
//
// Adds:
// - #camera/offline in the tile (Living room camera), over #camera/feed: "Camera offline" and
//   a line under it, in the middle above the caption. The app shows it while no camera app is
//   connected (the zone is off then and the feed shows its dark fill); camera-app draws its own
//   offline state into the zone when the camera is missing.
// - #camera-view over #smarthome-root: a frame of the whole screen in a plain absolute frame,
//   drawn closed: below the screen, as a closed sheet's card waits behind its edge (the
//   dashboard stays in sight in the file). The app moves it as a sheet's card (a position
//   override with the library's open and close transitions). In it:
//   - #camera-view/picture: a plain frame of the whole screen, dark, its own clip: the app turns
//     it into the Wayland zone #zone/camera-live, where camera-app shows live video;
//   - #camera-view/offline: "Camera offline" in the middle, while no camera app is connected;
//   - a dark band at the top, over it #camera-view/live (the tile's LIVE chip drawn again with
//     its values: a copy of the designer's node does not encode), the room's name and
//     #camera-view/close (#icon-button, scrim, x) at the right.
//   The parts come after the picture: harry paints in child order, and a touch on a part drawn
//   over a zone is the panel's, not the zone's.
//
// Every place is a constant below.

let { key, authoring, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080;
// the dark camera-app draws while it has no picture (its camera-patch.js)
const DARK = [0x1d / 255, 0x27 / 255, 0x33 / 255];
const WHITE = [1, 1, 1];
// the tile (dashboard-patch.js: Living room camera, 904 x 360) and its offline text, above the
// caption (which starts 273 down)
const TILE = 'Living room camera', TILE_W = 904, TILE_OFFLINE_Y = 120;
// the view's top band, its padding, the chip's place; where the closed view waits (camera.rs
// CLOSED_Y)
const BAND_H = 112, PAD = 40, CLOSED_Y = SCREEN_H + 40;

module.exports = function patch(msg, helpers) {
  ({ key, authoring, FONT } = helpers);
  const A = authoring(msg, 'camera');
  const { nodes, kids, log, frame, text, rect } = A;
  if (nodes.some(n => /^#camera-view(\/|$)/.test(n.name || '') || n.name === '#camera/offline')) throw new Error('#camera-view already present');
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  const tile = nodes.find(n => n.name === TILE && kids(key(n.guid)).some(c => c.name === '#camera/feed'));
  if (!tile) throw new Error(`${TILE} with #camera/feed not found: run dashboard-patch.js first`);
  const chip = kids(key(tile.guid)).find(n => n.name === '#camera/live');
  if (!chip) throw new Error('#camera/live not found: run dashboard-patch.js first');
  const iconSet = nodes.find(n => n.isStateGroup && n.name === '#icon-button');
  const close = iconSet && kids(key(iconSet.guid)).find(n => n.name === 'kind=scrim, state=default, icon=x, size=regular');
  if (!close) throw new Error('#icon-button scrim x missing: run components-patch.js first');
  const solid = rgb => [{ type: 'SOLID', color: { r: rgb[0], g: rgb[1], b: rgb[2], a: 1 }, opacity: 1, visible: true, blendMode: 'NORMAL' }];
  // "Camera offline" and the line under it, `w` wide from y: the same words as camera-app's
  const offline = (parent, name, y, w, size) => {
    const g = frame(parent, name, 0, y, w, Math.round(size * 3.2));
    text(g, 'Title', 0, 0, w, Math.round(size * 1.3), 'Camera offline', size, FONT.semibold, WHITE, 'CENTER');
    text(g, 'Detail', 0, Math.round(size * 1.6), w, Math.round(size * 0.9), 'The camera app is not running', Math.round(size * 0.67),
      FONT.regular, WHITE, 'CENTER', 'TOP', { opacity: 0.72 });
    return g;
  };

  // 1. the tile's offline state, after everything in the tile (above the feed and its scrim)
  offline(tile, '#camera/offline', TILE_OFFLINE_Y, TILE_W, 24);

  // 2. the full view over the dashboard, after every other layer
  const layer = frame(root, 'Camera layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  const view = frame(layer, '#camera-view', 0, CLOSED_Y, SCREEN_W, SCREEN_H, { fillPaints: solid(DARK), frameMaskDisabled: false });
  frame(view, '#camera-view/picture', 0, 0, SCREEN_W, SCREEN_H, { fillPaints: solid(DARK), frameMaskDisabled: false });
  offline(view, '#camera-view/offline', SCREEN_H / 2 - 60, SCREEN_W, 40);
  // the band: black 35 % (a node's opacity: Figma drops a paint's alpha on import)
  rect(view, 'Band', 0, 0, SCREEN_W, BAND_H, [0, 0, 0], 0, { opacity: 0.35 });
  // the chip as the tile's (Smart HOme.fig, Live status): green-black 65 %, a sand lamp, LIVE
  const live = frame(view, '#camera-view/live', PAD, (BAND_H - chip.size.y) / 2, chip.size.x, chip.size.y,
    { cornerRadius: chip.size.y / 2, frameMaskDisabled: false });
  rect(live, 'bg', 0, 0, chip.size.x, chip.size.y, [0.1255, 0.1608, 0.1333], chip.size.y / 2, { opacity: 0.65 });
  A.ellipse(live, 'Live lamp', 14, 14, 6, 6, [0.9216, 0.8314, 0.6471]);
  text(live, 'State', 28, 9, 32, 16, 'LIVE', 13, FONT.semibold, WHITE, 'LEFT', 'TOP', { letterSpacing: { value: 1.1, units: 'PIXELS' } });
  text(view, 'Room', PAD + chip.size.x + 16, (BAND_H - 40) / 2, 600, 40, 'Living room', 28, FONT.semibold, WHITE, 'LEFT', 'CENTER');
  const slot = frame(view, '#camera-view/close', SCREEN_W - PAD - close.size.x, (BAND_H - close.size.y) / 2, close.size.x, close.size.y);
  A.instance(slot, '#camera-view/close/preview', 0, 0, close);

  log(`added ${A.count()} nodes: #camera/offline in ${TILE}; #camera-view ${SCREEN_W}x${SCREEN_H} over ${ROOT} (picture, offline, band, live chip, room, close)`);
  return msg;
};
