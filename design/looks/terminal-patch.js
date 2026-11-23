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
// terminal-patch.js: the look "Terminal" of the panel: a black screen in one monospaced face,
// as a console. A header line, the time with its seconds, the weather, the house as lines
// (a name, a dotted leader, what it is, the state in a field), the agenda at the right, a
// prompt and the buttons. A line of a room is its header: a tap opens the device sheet with
// the control. The theme (terminal-theme.js) gives the colours.
module.exports = function (msg, helpers) {
  const { C } = helpers;
  const BRIGHT = [0.886, 1, 0.914];
  const L = require('./looks-common.js')(msg, helpers, 'terminal', { page: C.page });
  const { root, text, frame, slot, V, one, rect, F } = L;
  const MONO = F.mono;
  const line = h => ({ lineHeight: { value: h, units: 'PIXELS' } });
  // a text of the mono face: its box as wide as its characters (0.6 em each) unless given
  const t = (parent, name, x, y, s, size, rgb, w, align = 'LEFT') =>
    text(parent, name, x, y, w || Math.ceil(s.length * size * 0.6) + 4, Math.round(size * 1.3), s, size, MONO, rgb, align, 'CENTER', Object.assign(line(Math.round(size * 1.3)), one));

  // ---- the header line
  t(root, 'Header', 72, 48, '> EVERYWARE :: HOME NODE', 28, C.ink);
  t(root, '#home/date', 800, 48, 'SUN, 22 NOV', 28, C.ink, 440, 'RIGHT');

  // ---- the time with its seconds, the weather beside it
  text(root, '#home/time', 60, 96, 780, 270, '22:41', 250, MONO, BRIGHT, 'LEFT', 'CENTER', Object.assign(line(270), one));
  text(root, '#home/sec', 830, 232, 240, 120, ':07', 96, MONO, C.ink, 'LEFT', 'CENTER', Object.assign(line(120), one));
  L.weatherIcon(root, 72, 396);
  t(root, '#weather/temp', 128, 386, '-12°', 44, BRIGHT, 150);
  t(root, '#weather/condition', 290, 386, 'Overcast', 44, C.ink, 950);
  t(root, '#weather/place', 72, 446, 'Berlin', 26, C.ink2, 260);
  t(root, '#weather/details', 340, 446, 'Feels -1° · 77 % · 13 km/h', 26, C.ink2, 900);

  // ---- the house, a line each: NAME ......... WHAT [ state ]
  const X = 72, W = 1168, ROW = 78, TOP = 516, FIELD = 300, WHAT = 290;
  const dots = '.'.repeat(60);
  const row = (i, parent0, name, what, stateName, state, bright) => {
    const y = TOP + i * ROW;
    const p = parent0 ? frame(root, parent0, X, y, W, ROW - 8) : frame(root, 'Line', X, y, W, ROW - 8);
    t(p, 'Name', 0, 8, name, 40, BRIGHT, 330);
    t(p, 'Leader', 330, 8, dots, 40, C.muted, W - 330 - WHAT - FIELD - 40);
    t(p, 'What', W - WHAT - FIELD - 20, 8, what, 40, C.ink, WHAT);
    const f = frame(p, 'Field', W - FIELD, 6, FIELD, ROW - 20, Object.assign(L.A.stroke(C.ink, 2)));
    (Array.isArray(stateName) ? stateName : [[stateName, state, bright]]).forEach(([n, s, b]) =>
      t(f, n, 0, 3, s, 36, b ? BRIGHT : C.ink, FIELD, 'CENTER'));
  };
  row(0, '#entryway/header', 'ENTRYWAY', 'LIGHTS', '#entryway/state', 'On');
  row(1, '#bedroom/header', 'BEDROOM', 'LIGHTS', '#bedroom/level', '65%');
  row(2, '#office/header', 'OFFICE', 'FAN', '#office/state', 'On · Low');
  row(3, null, 'FRONT DOOR', 'LOCK', 'Door state', 'Locked');
  row(4, '#camera/tap', 'CAMERA', 'LIVING ROOM', [['#camera/live', 'Live', true], ['#camera/offline', 'Offline', false]]);

  // ---- the prompt
  t(root, 'Prompt', 72, 984, '>', 40, C.ink);
  rect(root, 'Cursor', 112, 990, 24, 44, C.accent, 0);

  // ---- the right: the agenda, and under it the buttons
  t(root, 'Agenda heading', 1320, 48, '> AGENDA', 28, C.ink);
  L.agenda(root, 1320, 100, 840, C.ink, C.ink2, C.white);
  const wifi = V.wifi(), set = V.settings(), talk = V.talk();
  const BY = 1080 - 44 - wifi.size.y;
  slot(root, '#status/wifi', 1320, BY, wifi);
  slot(root, '#settings', 1320 + wifi.size.x + 12, BY + (wifi.size.y - set.size.y) / 2, set);
  L.talk(root, 1320 + 512 - talk.size.x, BY + (wifi.size.y - talk.size.y) / 2);
  L.cameraView();
  L.log('terminal: the header, the time with its seconds, the weather, five lines, the agenda, the buttons');
};
