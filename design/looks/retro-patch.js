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
// retro-patch.js: the look "Retro" of the panel: a desktop of the nineties. A teal ground,
// grey windows with raised edges and navy title bars (Clock, Weather, Agenda, Rooms, Camera,
// Intercom) and a task bar with the time in its tray. The rooms are a table: the name (the
// room's header: a tap opens the device sheet), the state, the control. The picture in the
// Camera window is the camera's zone; a tap on it opens the full view. The theme
// (retro-theme.js) gives the controls and the sheets the same grey.
//
// The colours here are the look's own (none is of the Library's palette, so the theme leaves
// them): the white of a field is #fefefe.
module.exports = function (msg, helpers) {
  const { C } = helpers;
  const GREY = [0.753, 0.753, 0.753], WHITE = [0.996, 0.996, 0.996], DARK = [0.251, 0.251, 0.251], MID = [0.502, 0.502, 0.502];
  const NAVY = [0, 0, 0.502], BLACK = [0, 0, 0], TEAL = [0, 0.502, 0.502], LCD = [0.2, 1, 0.4];
  const L = require('./looks-common.js')(msg, helpers, 'retro', { page: C.page });
  const { root, text, frame, slot, V, one, rect, F } = L;
  const line = h => ({ lineHeight: { value: h, units: 'PIXELS' } });
  const fill = rgb => ({ fillPaints: [L.solid(rgb)] });

  // an edge of two tones, 3 wide: raised (light at the top and the left) or sunken
  const edge = (parent, w, h, raised = true) => {
    const [a, b] = raised ? [WHITE, DARK] : [MID, WHITE];
    rect(parent, 'Edge top', 0, 0, w, 3, a, 0);
    rect(parent, 'Edge left', 0, 0, 3, h, a, 0);
    rect(parent, 'Edge bottom', 0, h - 3, w, 3, b, 0);
    rect(parent, 'Edge right', w - 3, 0, 3, h, b, 0);
  };
  // a window: the grey, its edge, the title bar with a close box; returns its client frame
  const win = (x, y, w, h, title) => {
    const f = frame(root, title + ' window', x, y, w, h, fill(GREY));
    edge(f, w, h);
    const bar = frame(f, 'Title bar', 6, 6, w - 12, 44, fill(NAVY));
    text(bar, 'Title', 12, 0, w - 90, 44, title, 24, F.semibold, WHITE, 'LEFT', 'CENTER', one);
    const box = frame(bar, 'Close box', w - 12 - 42, 6, 36, 32, fill(GREY));
    edge(box, 36, 32);
    text(box, 'x', 0, 0, 36, 30, '×', 24, F.semibold, BLACK, 'CENTER', 'CENTER', one);
    return frame(f, 'Client', 6, 56, w - 12, h - 62);
  };
  // a sunken field in a window
  const field = (parent, x, y, w, h, rgb = WHITE) => {
    const f = frame(parent, 'Field', x, y, w, h, Object.assign(fill(rgb), { frameMaskDisabled: false }));
    return f;
  };
  const sunk = f => { edge(f, f.size.x, f.size.y, false); return f; };

  // ---- Clock: the time and its seconds as a lit display, the date under it
  {
    const c = win(40, 36, 560, 296, 'Clock');
    const d = field(c, 6, 2, 536, 226, BLACK);
    // (the mono face: 0.6 em a character, so the boxes are as wide as their characters)
    text(d, '#home/time', 24, 14, 350, 150, '22:41', 110, F.mono, LCD, 'RIGHT', 'CENTER', Object.assign(line(150), one));
    text(d, '#home/sec', 378, 62, 140, 80, ':07', 56, F.mono, LCD, 'LEFT', 'CENTER', Object.assign(line(80), one));
    text(d, '#home/date', 0, 170, 536, 40, 'SUN, 22 NOV', 28, F.mono, LCD, 'CENTER', 'CENTER', one);
    sunk(d);
  }

  // ---- Weather
  {
    const c = win(40, 352, 560, 236, 'Weather');
    const d = field(c, 6, 2, 536, 166);
    L.weatherIcon(d, 20, 28);
    text(d, '#weather/temp', 72, 10, 190, 80, '3°', 64, F.semibold, BLACK, 'LEFT', 'CENTER', Object.assign(line(80), one));
    text(d, '#weather/condition', 266, 16, 256, 36, 'Overcast', 26, F.semibold, BLACK, 'LEFT', 'CENTER', one);
    text(d, '#weather/place', 266, 52, 256, 32, 'Berlin', 22, F.regular, BLACK, 'LEFT', 'CENTER', one);
    text(d, '#weather/details', 20, 108, 500, 36, 'Feels -1° · 77 % · 13 km/h', 22, F.regular, BLACK, 'LEFT', 'CENTER', one);
    sunk(d);
  }

  // ---- Agenda
  {
    const c = win(40, 608, 560, 380, 'Agenda');
    const d = field(c, 6, 2, 536, 310);
    L.agenda(d, 12, 8, 294, BLACK, DARK, GREY);
    sunk(d);
  }

  // ---- Rooms: a table, a room a row
  {
    const W = 1240, c = win(640, 36, W, 500, 'Rooms');
    const menu = frame(c, 'Menu bar', 0, 0, W - 12, 40);
    ['File', 'View', 'Devices', 'Help'].forEach((m, i) => text(menu, m, 12 + i * 110, 0, 110, 40, m, 22, F.regular, BLACK, 'LEFT', 'CENTER', one));
    const d = field(c, 6, 42, W - 24, 342);
    const COL = [0, 300, 620], CW = [300, 320, W - 24 - 620];
    ['Room', 'State', 'Control'].forEach((h, i) => {
      const cell = frame(d, h + ' heading', COL[i], 0, CW[i], 42, fill(GREY));
      text(cell, 'Heading', 14, 0, CW[i] - 20, 42, h, 22, F.semibold, BLACK, 'LEFT', 'CENTER', one);
      edge(cell, CW[i], 42);
    });
    const room = (i, id, name, stateName, state, control, v) => {
      const y = 42 + i * 100;
      const head = frame(d, `#${id}/header`, COL[0], y, CW[0], 100);
      text(head, 'Name', 14, 0, CW[0] - 20, 100, name, 26, F.semibold, BLACK, 'LEFT', 'CENTER', one);
      text(d, stateName, COL[1] + 14, y, CW[1] - 20, 100, state, 26, F.regular, BLACK, 'LEFT', 'CENTER', one);
      slot(d, control, COL[2] + 14, y + (100 - v.size.y) / 2, v);
    };
    room(0, 'entryway', 'Entryway', '#entryway/state', 'On', '#entryway/light', V.toggle());
    room(1, 'bedroom', 'Bedroom', '#bedroom/level', '65%', '#bedroom/dimmer', V.slider());
    room(2, 'office', 'Office', '#office/state', 'On · Low', '#office/fan', V.segmented());
    sunk(d);
    const status = field(c, 6, 392, W - 24 - 330, 40, GREY);
    text(status, 'Status', 12, 0, 600, 40, '3 rooms', 22, F.regular, BLACK, 'LEFT', 'CENTER', one);
    sunk(status);
    const door = field(c, W - 18 - 320, 392, 320, 40, GREY);
    text(door, 'Door', 12, 0, 300, 40, 'Front door: locked', 22, F.regular, BLACK, 'LEFT', 'CENTER', one);
    sunk(door);
  }

  // ---- Camera: the picture is the camera's zone, a tap on it opens the full view
  {
    const W = 760, H = 432, c = win(640, 556, W, H, 'Camera - Living room');
    const d = frame(c, 'Picture', 6, 2, W - 24, H - 72, Object.assign(fill(BLACK), { frameMaskDisabled: false }));
    frame(d, '#camera/feed', 0, 0, W - 24, H - 72, Object.assign(fill([0.114, 0.153, 0.2]), { frameMaskDisabled: false }));
    text(d, '#camera/offline', 0, (H - 72) / 2 - 20, W - 24, 40, 'Camera offline', 24, F.semibold, WHITE, 'CENTER', 'CENTER', one);
    const live = frame(d, '#camera/live', 14, 12, 84, 34, fill(BLACK));
    text(live, 'Live', 0, 0, 84, 34, 'LIVE', 20, F.semibold, LCD, 'CENTER', 'CENTER', one);
    frame(d, '#camera/tap', 0, 0, W - 24, H - 72);
  }

  // ---- Intercom: Talk, and the panel's own controls
  {
    const W = 440, c = win(1440, 556, W, 432, 'Intercom');
    text(c, 'Question', 0, 24, W - 12, 40, 'Talk to all rooms?', 24, F.regular, BLACK, 'CENTER', 'CENTER', one);
    const talk = V.talk(), wifi = V.wifi(), set = V.settings();
    L.talk(c, (W - 12 - talk.size.x) / 2, 84);
    rect(c, 'Rule', 20, 170, W - 52, 2, MID, 0);
    text(c, 'Panel', 0, 184, W - 12, 36, 'Panel', 22, F.semibold, BLACK, 'CENTER', 'CENTER', one);
    const gx = (W - 12 - wifi.size.x - 16 - set.size.x) / 2;
    slot(c, '#status/wifi', gx, 236 + (set.size.y - wifi.size.y) / 2, wifi);
    slot(c, '#settings', gx + wifi.size.x + 16, 236, set);
  }

  // ---- the task bar: Home, the windows, the tray with the hours and the minutes
  {
    const bar = frame(root, 'Task bar', 0, 1008, 1920, 72, fill(GREY));
    rect(bar, 'Edge top', 0, 0, 1920, 3, WHITE, 0);
    const home = frame(bar, 'Home button', 8, 10, 150, 54, fill(GREY));
    edge(home, 150, 54);
    text(home, 'Home', 0, 0, 150, 52, 'Home', 26, F.semibold, BLACK, 'CENTER', 'CENTER', one);
    ['Rooms', 'Camera', 'Agenda'].forEach((n, i) => {
      const b = frame(bar, n + ' task', 170 + i * 250, 10, 240, 54, fill(GREY));
      edge(b, 240, 54, i !== 0);
      text(b, 'Name', 14, 0, 210, 52, n, 24, i === 0 ? F.semibold : F.regular, BLACK, 'LEFT', 'CENTER', one);
    });
    const tray = frame(bar, 'Tray', 1920 - 8 - 150, 10, 150, 54, fill(GREY));
    edge(tray, 150, 54, false);
    text(tray, '#home/hours', 8, 0, 60, 52, '22', 26, F.regular, BLACK, 'RIGHT', 'CENTER', one);
    text(tray, 'Colon', 68, 0, 12, 50, ':', 26, F.regular, BLACK, 'CENTER', 'CENTER', one);
    text(tray, '#home/minutes', 80, 0, 60, 52, '41', 26, F.regular, BLACK, 'LEFT', 'CENTER', one);
  }
  L.cameraView();
  L.log('retro: six windows and the task bar');
};
