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
// window-patch.js: the look "Window" of the panel: the camera's picture is the screen. The
// zone of the camera fills it; over it the time (the hours heavy, the minutes light, the
// seconds in the accent), the weather, the agenda as a pane of glass at the right and one bar
// at the bottom: a room a field (its header: a tap opens the device sheet with the control),
// the door, Talk. The theme (window-theme.js) gives the sheets' and the controls' colours.
//
// The type over the picture is a white of its own (#fcfcfc), which the theme leaves alone.
module.exports = function (msg, helpers) {
  const { C } = helpers;
  const INK = [0.988, 0.988, 0.988], ACCENT = [1, 0.706, 0.329], DARK = [0.043, 0.063, 0.075];
  const L = require('./looks-common.js')(msg, helpers, 'window', { page: C.page });
  const { root, text, frame, slot, V, one, rect, F } = L;
  const line = h => ({ lineHeight: { value: h, units: 'PIXELS' } });
  const paint = (rgb, a) => ({ type: 'SOLID', color: { r: rgb[0], g: rgb[1], b: rgb[2], a: 1 }, opacity: a, visible: true, blendMode: 'NORMAL' });
  const glass = (parent, name, x, y, w, h, r, a = 0.62) => frame(parent, name, x, y, w, h, Object.assign({
    fillPaints: [paint(DARK, a)], cornerRadius: r, frameMaskDisabled: false }, L.A.stroke(INK, 1), { strokePaints: [paint(INK, 0.16)] }));

  // ---- the picture: the camera live over the whole screen (#camera/live-feed, the zone of
  // the live video: a plain frame without children, camera-app draws into it; the app keeps
  // the camera in live video for a design that has this node), and a shade over it so that
  // the type stays readable
  frame(root, '#camera/live-feed', 0, 0, 1920, 1080, { fillPaints: [paint([0.114, 0.153, 0.2], 1)], frameMaskDisabled: false });
  frame(root, 'Shade', 0, 0, 1920, 1080, { fillPaints: [{
    type: 'GRADIENT_LINEAR', opacity: 1, visible: true, blendMode: 'NORMAL',
    transform: { m00: 0, m01: 1, m02: 0, m10: -1, m11: 0, m12: 1 },
    stops: [[0, 0.55], [0.24, 0.1], [0.52, 0.2], [1, 0.86]].map(([position, a]) => ({ position, color: { r: 0.024, g: 0.039, b: 0.047, a } })),
  }] });

  // ---- the top: what the picture is, and the status controls
  // (a tap on it opens the camera's full view: the live video without the panel over it)
  const tap = frame(root, '#camera/tap', 40, 36, 560, 80);
  const live = frame(tap, '#camera/live', 16, 20, 110, 40);
  rect(live, 'Dot', 0, 14, 12, 12, ACCENT, 6);
  text(live, 'Live', 26, 0, 84, 40, 'LIVE', 20, F.semibold, INK, 'LEFT', 'CENTER', one);
  text(tap, '#camera/offline', 16, 20, 110, 40, 'OFFLINE', 20, F.semibold, INK, 'LEFT', 'CENTER', one);
  text(tap, 'Camera name', 136, 20, 400, 40, 'Living room camera', 20, F.regular, INK, 'LEFT', 'CENTER', one);
  const wifi = V.wifi(), set = V.settings();
  slot(root, '#settings', 1920 - 56 - set.size.x, 24, set);
  slot(root, '#status/wifi', 1920 - 56 - set.size.x - 12 - wifi.size.x, 24 + (set.size.y - wifi.size.y) / 2, wifi);
  L.presence(root, 1920 - 56 - set.size.x - 12 - wifi.size.x - 24 - V.presence().size.x, 24 + (set.size.y - 48) / 2);

  // ---- the time: the hours, a colon, the minutes, the seconds; the date over it
  const TY = 470, TH = 330;
  text(root, '#home/date', 60, TY - 44, 700, 40, 'SUN, 22 NOV', 26, F.semibold, INK, 'LEFT', 'CENTER', one);
  text(root, '#home/hours', 20, TY, 400, TH, '22', 300, F.black, INK, 'RIGHT', 'CENTER', Object.assign(line(TH), one));
  text(root, 'Colon', 420, TY - 14, 90, TH, ':', 300, F.regular, INK, 'CENTER', 'CENTER', Object.assign(line(TH), one));
  text(root, '#home/minutes', 510, TY, 400, TH, '41', 300, F.regular, INK, 'LEFT', 'CENTER', Object.assign(line(TH), one));
  text(root, '#home/sec', 880, TY + 150, 200, 110, ':07', 84, F.regular, ACCENT, 'LEFT', 'CENTER', Object.assign(line(110), one));

  // ---- the weather, at the right of the time
  L.weatherIcon(root, 1040, TY + 96);
  text(root, '#weather/temp', 1092, TY + 60, 220, 110, '3°', 96, F.regular, INK, 'LEFT', 'CENTER', Object.assign(line(110), one));
  text(root, '#weather/condition', 1040, TY + 176, 290, 44, 'Overcast', 32, F.regular, INK, 'LEFT', 'CENTER', one);
  text(root, '#weather/place', 1040, TY + 224, 290, 30, 'Berlin', 20, F.regular, INK, 'LEFT', 'CENTER', one);
  text(root, '#weather/details', 1040, TY + 254, 290, 30, 'Feels -1° · 77 % · 13 km/h', 20, F.regular, INK, 'LEFT', 'CENTER', one);

  // ---- the agenda: a pane of glass at the right
  const pane = glass(root, 'Agenda pane', 1920 - 56 - 520, 190, 520, 650, 32, 0.5);
  text(pane, 'Agenda heading', 28, 20, 300, 36, 'Agenda', 24, F.semibold, INK, 'LEFT', 'CENTER', one);
  L.agenda(pane, 4, 64, 562, INK, INK, DARK);

  // ---- the bar: a field a room, the door, Talk
  const BAR_Y = 1080 - 48 - 148;
  const bar = glass(root, 'Bar', 56, BAR_Y, 1808, 148, 36);
  const talk = V.talk();
  const FW = 330, GAP = 12;
  const field = (i, name, title, stateName, state, lit) => {
    const f = frame(bar, name, 12 + i * (FW + GAP), 12, FW, 124, { fillPaints: [paint(lit ? ACCENT : INK, lit ? 0.16 : 0.07)], cornerRadius: 26, frameMaskDisabled: false });
    text(f, 'Title', 28, 26, FW - 56, 36, title, 28, F.semibold, INK, 'LEFT', 'CENTER', one);
    text(f, stateName, 28, 66, FW - 56, 30, state, 20, F.regular, INK, 'LEFT', 'CENTER', one);
  };
  field(0, '#entryway/header', 'Entryway', '#entryway/state', 'On');
  field(1, '#bedroom/header', 'Bedroom', '#bedroom/level', '65%');
  field(2, '#office/header', 'Office', '#office/state', 'On · Low');
  field(3, 'Front door', 'Front door', 'Door state', 'Locked');
  L.talk(bar, 1808 - 24 - talk.size.x, (148 - talk.size.y) / 2);
  L.cameraView();
  L.log('window: the camera over the whole screen, the time in three parts, the weather, the agenda pane, the bar');
};
