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

// poster-patch.js: the look "Poster" of the panel: a black screen, the hours over the minutes
// as a poster's headline, and at the right the house as lines of type: a name at the left,
// what the app says about it at the right. A line of a room is its header: a tap opens the
// device sheet with the control. Only type; the theme (poster-theme.js) gives the colours.
//
// The colours here are the Library's light palette (the theme maps them), except the accent
// of the minutes, which no theme touches.
module.exports = function (msg, helpers) {
  const { C } = helpers;
  const ACCENT = [1, 0.353, 0.212];
  const L = require('./looks-common.js')(msg, helpers, 'poster', { page: C.page });
  const { root, text, frame, slot, V, one, rect } = L;
  const shoulders = style => ({ family: 'Big Shoulders Display', style, postscript: 'BigShouldersDisplay-' + style });
  const BLACK = shoulders('Black'), MEDIUM = shoulders('Medium');
  const line = h => ({ lineHeight: { value: h, units: 'PIXELS' } });

  // ---- the left: the date and the place, the hours over the minutes
  text(root, '#home/date', 64, 48, 330, 44, 'SUN, 22 NOV', 34, BLACK, C.ink, 'LEFT', 'CENTER', one);
  text(root, '#weather/place', 400, 48, 320, 44, 'Berlin', 34, MEDIUM, C.ink2, 'LEFT', 'CENTER', one);
  // (a digit is 0.8 of the size high; the boxes are as high as the face's own line, so they overlap)
  text(root, '#home/hours', 56, 30, 660, 600, '22', 500, BLACK, C.ink, 'LEFT', 'CENTER', Object.assign(line(600), one));
  text(root, '#home/minutes', 56, 460, 660, 600, '41', 500, BLACK, ACCENT, 'LEFT', 'CENTER', Object.assign(line(600), one));

  // ---- the right: lines of type
  const X = 780, W = 1076, ROW = 104, TOP = 40;
  const label = (parent, x, y, s, w = 420) => text(parent, 'Label', x, y, w, ROW - 4, s, 60, BLACK, C.ink, 'LEFT', 'CENTER', Object.assign(line(72), one));
  const value = (parent, name, x, y, w, s, rgb = C.ink) => text(parent, name, x, y, w, ROW - 4, s, 60, MEDIUM, rgb, 'RIGHT', 'CENTER', Object.assign(line(72), one));
  const rule = i => rect(root, 'Line', X, TOP + (i + 1) * ROW - 2, W, 2, C.muted, 0);

  // the weather: its picture and the temperature after the name, the condition at the right
  let y = TOP;
  label(root, X, y, 'WEATHER', 280);
  L.weatherIcon(root, X + 290, y + (ROW - 44) / 2);
  text(root, '#weather/temp', X + 344, y, 220, ROW - 4, '3°', 60, MEDIUM, C.ink, 'LEFT', 'CENTER', Object.assign(line(72), one));
  value(root, '#weather/condition', X + 570, y, W - 570, 'Overcast');
  rule(0);

  // a room: the whole line is its header (the device sheet's tap target)
  const room = (i, id, name, stateName, state) => {
    const head = frame(root, `#${id}/header`, X, TOP + i * ROW, W, ROW - 4);
    label(head, 0, 0, name);
    value(head, stateName, 430, 0, W - 430, state);
    rule(i);
  };
  room(1, 'entryway', 'ENTRYWAY', '#entryway/state', 'On');
  room(2, 'bedroom', 'BEDROOM', '#bedroom/level', '65%');
  room(3, 'office', 'OFFICE', '#office/state', 'On · Low');

  y = TOP + 4 * ROW;
  label(root, X, y, 'FRONT DOOR');
  value(root, 'Door state', X + 430, y, W - 430, 'Locked');
  rule(4);

  y = TOP + 5 * ROW;
  // (the line is the camera's tap target: the full view opens)
  const cam = frame(root, '#camera/tap', X, y, W, ROW - 4);
  label(cam, 0, 0, 'CAMERA');
  value(cam, '#camera/live', 430, 0, W - 430, 'Live', ACCENT);
  value(cam, '#camera/offline', 430, 0, W - 430, 'Offline', C.ink2);
  rule(5);

  // the agenda: its name as a line's, the list at the right where the values are
  y = TOP + 6 * ROW;
  label(root, X, y, 'AGENDA');
  L.agenda(root, X + W - 512, y + 20, 1080 - 130 - (y + 20), C.ink, C.ink2, C.white);

  // ---- the buttons: Wi-Fi, settings, Talk
  const wifi = V.wifi(), set = V.settings(), talk = V.talk();
  const BY = 1080 - 48 - wifi.size.y;
  slot(root, '#status/wifi', X, BY, wifi);
  slot(root, '#settings', X + wifi.size.x + 12, BY + (wifi.size.y - set.size.y) / 2, set);
  L.talk(root, X + wifi.size.x + 12 + set.size.x + 24, BY + (wifi.size.y - talk.size.y) / 2);
  L.cameraView();
  L.log('poster: the hours over the minutes, six lines, the agenda and the buttons');
};
