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

// event-patch.js: the panel of an agenda event, an instance of the library's #sheet
// (components-patch.js) at the right edge with its own content component, at the kiwi level.
// Run after prefs-patch.js (build.sh), so the event's layer is above the settings.
// A tap on an event's row of #agenda/list opens it, the agenda's header button (#agenda/account)
// opens it on the calendar's link (the app, agenda_card.rs).
//
// Adds:
// - #event-content in the Library: the content, the size of the right panel's content area:
//   #event/close (a tonal icon button) at the top right, above four pages side by side in a
//   frame that clips them (the panel's steps, ui-lib screens.rs Pages: the app slides them,
//   agenda_card.rs EventStep). The order is the steps': the confirmation last, as both the
//   details and the calendar's link go forward to it. Every page is drawn at full opacity:
//   harry takes no taps on what the design draws transparent; the app hides the others.
//   - #event/page-details: #event/scroll, a frame that clips, with #event/body in it: a column (auto layout, 16 between the parts) of texts
//     that grow with their content: #event/title (the whole title, beside the close button),
//     #event/when (the day and the time), #event/place, #event/rsvp (the library's #segmented:
//     Accept, Maybe, Decline), #event/link (the meeting's address, as text) and
//     #event/description. The app takes a part out of the column when it is empty or not
//     offered (display gone), measures the column's height and moves #event/body up as the
//     finger scrolls it (agenda_card.rs EventPanel; state.rs has the same numbers as here). At
//     the bottom #event/actions, a row that closes up, with #event/join and #event/directions
//     (secondary buttons), and #event/delete at the right.
//   - #event/page-qr: #event/qr-title, #event/qr (a plain frame: the app draws the QR code on it), "Scan with
//     your phone", #event/qr-host (the host the phone will open), #event/back.
//   - #event/page-calendar: "Google Calendar", #event/access (the access in words, the app's),
//     what the two actions do, #event/change-access and #event/disconnect.
//   - #event/page-confirm: #event/confirm-title, #event/confirm-text, #event/cancel and
//     #event/confirm (its label the app's).
//   Every page but the QR code's has, above its buttons, #event/lines: #event/status (what the
//   panel waits for) and #event/error (why a change failed), each out of the column when empty
//   (the same names: the app sets them once for all).
// - #event over #smarthome-root: #sheet layout=right, state=closed, in a plain absolute frame,
//   its content swapped for #event-content state=default and its parts named #event/scrim, /card,
//   /content (fig-helpers.js sheetInstance). The app keeps it closed and moves the card itself.

let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080, PAD = 40;
// the right panel's content area (components-patch.js: half the width, full height, 40 inside)
const CONTENT_W = SCREEN_W / 2 - 2 * PAD, CONTENT_H = SCREEN_H - 2 * PAD;
// the column's parts: size, line height, weight (state.rs EVENT_* the same)
const GAP = 16, TITLE_W = CONTENT_W - 72 - 16;
const PARTS = [
  ['#event/title', TITLE_W, 28, 36, 'semibold', 'ink', 'Dentist: check-up and cleaning, then the results of the X-ray with Dr. Weber'],
  ['#event/when', CONTENT_W, 19, 26, 'regular', 'ink2', 'Tue, Nov 17 · 08:30 to 09:30'],
  ['#event/place', CONTENT_W, 19, 26, 'regular', 'ink', 'Main Street 12, 10115 Berlin, second floor'],
  ['#event/rsvp'],
  ['#event/link', CONTENT_W, 17, 24, 'regular', 'accent', 'https://meet.google.com/abc-defg-hij'],
  ['#event/description', CONTENT_W, 19, 28, 'regular', 'ink',
    'Bring the insurance card and the list of the medicines you take.\n\nThe practice is on the second floor; the lift is at the back of the courtyard. ' +
    'Please come ten minutes early to fill in the form.\n\nTo move the appointment, call the practice the day before.'],
];
// the buttons at the bottom, the two lines above them (with 4 between), the body above those
const BTN_W = 200, BTN_H = 56, BTN_GAP = 16, LINE_H = 26, ROW_Y = CONTENT_H - BTN_H, LINES_Y = ROW_Y - GAP - 2 * LINE_H - 4;
const SCROLL_H = LINES_Y - GAP;
// the QR code: square, the app makes its picture this size (agenda_card.rs EVENT_QR_SIDE)
const QR = 480, QR_Y = 110;
// the pages in the order the app slides them (agenda_card.rs EventStep)
const PAGES = ['details', 'qr', 'calendar', 'confirm'];
// a rough height for the preview: about half an em a character
const previewH = (text, w, size, lh) => text.split('\n').reduce((n, l) => n + Math.max(1, Math.ceil(l.length * size * 0.55 / w)), 0) * lh;

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'event');
  const { nodes, kids, log, frame } = A;
  if (nodes.some(n => n.name === '#event' || n.name === '#event-content')) throw new Error('#event already present');
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  const lib = nodes.find(n => n.name === 'Library' && n.type === 'FRAME');
  if (!lib) throw new Error('Library missing: run components-patch.js first');
  const variant = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    const v = s && kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set} ${name} missing: run components-patch.js first`);
    return v;
  };
  const closedRight = variant('#sheet', 'layout=right, state=closed');
  const close = variant('#icon-button', 'kind=tonal, state=default, icon=x, size=regular');
  const secondary = variant('#button', 'kind=secondary, state=default, size=regular');
  // red for what cannot be undone (the library's error tone): Delete, and the confirmation's
  // Delete and Disconnect (the app sets the tone with the label; Change access is primary)
  const danger = variant('#button', 'kind=secondary-error, state=default, size=regular');
  const dangerPrimary = variant('#button', 'kind=primary-error, state=default, size=regular');
  const segmented = variant('#segmented', 'segments=3, state=default');
  // a slot: a frame named for the app with a preview instance of the library's component; a
  // button's label is the preview's (the app sets its own)
  const slotted = (parent, name, x, y, v, label) => {
    const slot = frame(parent, name, x, y, v.size.x, v.size.y);
    const text = label && kids(key(v.guid)).find(n => n.name === '#button/label');
    const overrides = text ? [{ guidPath: { guids: [text.guid] }, textData: { characters: label, lines: text.textData.lines } }] : [];
    A.instance(slot, name + '/preview', 0, 0, v, undefined, overrides);
    return slot;
  };
  const text = (parent, name, x, y, w, h, s, size, weight, colour, extra) =>
    A.text(parent, name, x, y, w, h, s, size, FONT[weight], C[colour], 'LEFT', 'TOP', Object.assign({ maxLines: Math.max(1, Math.floor(h / (size * 1.4))), textTruncation: 'ENDING' }, extra || {}));
  // the two lines above a step's buttons: a column, an empty one out of it (the app's)
  const lines = (box, status, error) => {
    const col = frame(box, '#event/lines', 0, LINES_Y, CONTENT_W, 2 * LINE_H + 4, {
      bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: 4, stackPrimarySizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE',
      stackCounterSizing: 'FIXED', stackLayoutVersion: 1,
    });
    text(col, '#event/status', 0, 0, CONTENT_W, LINE_H, status, 17, 'regular', 'ink2');
    text(col, '#event/error', 0, LINE_H + 4, CONTENT_W, LINE_H, error, 17, 'regular', 'error');
  };

  const y0 = lib.size.y + 80;
  const content = A.componentSet(lib, '#event-content', 80, y0, [['state', ['default']]], CONTENT_W, CONTENT_H, 0, 0, (s) => {
    const content = A.hugBox(s, CONTENT_W, CONTENT_H);
    // the frame that clips the pages, at x 0 of the content: the app reads the first page's
    // place from it (Sizes::part)
    const pages = frame(content, 'Pages', 0, 0, CONTENT_W, CONTENT_H, { frameMaskDisabled: false });
    PAGES.forEach((p, i) => drawPage(frame(pages, `#event/page-${p}`, i * CONTENT_W, 0, CONTENT_W, CONTENT_H), p));
    // close, above every page
    const slot = frame(content, '#event/close', CONTENT_W - close.size.x + 12, -16, close.size.x, close.size.y);
    A.instance(slot, '#event/close/preview', 0, 0, close);
  }, { rows: true });
  lib.size = { x: lib.size.x, y: y0 + content.set.size.y + 80 };

  const layer = frame(root, 'Event layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#event', 0, 0, closedRight, content.symbols['state=default']);

  log(`added ${A.count()} nodes: #event-content (${PAGES.length} pages) in the Library, #event (#sheet right, closed, content swapped) over ${ROOT}`);
  return msg;

  // one page: its parts at their places in the page
  function drawPage(box, page) {
    if (page === 'details') {
      // the place the body scrolls in: it clips; the close button stays above it
      const scroll = frame(box, '#event/scroll', 0, 0, CONTENT_W, SCROLL_H, { frameMaskDisabled: false });
      const heights = PARTS.map(([, w, size, lh, , , t]) => t ? previewH(t, w, size, lh) : segmented.size.y);
      const bodyH = heights.reduce((a, h) => a + h, 0) + GAP * (PARTS.length - 1);
      const body = frame(scroll, '#event/body', 0, 0, CONTENT_W, bodyH, {
        bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: GAP, stackPrimarySizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE',
        stackCounterSizing: 'FIXED', stackLayoutVersion: 1, frameMaskDisabled: true,
      });
      let y = 0;
      PARTS.forEach(([name, w, size, lh, weight, colour, t], i) => {
        if (name === '#event/rsvp') {
          // Accept, Maybe, Decline: the library's segmented button, under the place (the
          // preview's labels; the app sets its own and lights the account's answer, or none)
          const slot = frame(body, name, 0, y, segmented.size.x, segmented.size.y);
          const labels = ['Accept', 'Maybe', 'Decline'].flatMap((label, i) => ['label', 'label-selected', 'label-selected-ink'].map(part => {
            const t = A.under(segmented, `#segmented/segment-${i + 1}/${part}`);
            return { guidPath: { guids: [t.guid] }, textData: { characters: label, lines: t.textData.lines } };
          }));
          A.instance(slot, name + '/preview', 0, 0, segmented, undefined, labels);
        } else {
          // a text as wide as its place that grows down with its lines (harry measures it in layout)
          A.text(body, name, 0, y, w, heights[i], t, size, FONT[weight], C[colour], 'LEFT', 'TOP', {
            lineHeight: { value: lh, units: 'PIXELS' }, textAutoResize: 'HEIGHT',
          });
        }
        y += heights[i] + GAP;
      });
      lines(box, 'Sending the answer…', 'Google could not be reached. Try again.');
      // the bottom row: the codes for the phone at the left (a row that closes up), Delete at the right
      const actions = frame(box, '#event/actions', 0, ROW_Y, 2 * BTN_W + BTN_GAP, BTN_H, {
        bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackSpacing: BTN_GAP, stackPrimarySizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE',
        stackCounterSizing: 'FIXED', stackLayoutVersion: 1,
      });
      slotted(actions, '#event/join', 0, 0, secondary, 'Join on phone');
      slotted(actions, '#event/directions', BTN_W + BTN_GAP, 0, secondary, 'Directions');
      slotted(box, '#event/delete', CONTENT_W - BTN_W, ROW_Y, danger, 'Delete');
    } else if (page === 'qr') {
      text(box, '#event/qr-title', 0, 0, TITLE_W, 36, 'Join on phone', 28, 'semibold', 'ink');
      // a plain frame: harry draws a picture on a plain frame only
      frame(box, '#event/qr', (CONTENT_W - QR) / 2, QR_Y, QR, QR, { fillPaints: [A.solid(C.line)] });
      A.text(box, 'Scan hint', 0, QR_Y + QR + 24, CONTENT_W, 28, 'Scan with your phone', 19, FONT.regular, C.ink2, 'CENTER', 'TOP', { maxLines: 1, textTruncation: 'ENDING' });
      // the host the phone will open: who sent the link, the reader decides
      A.text(box, '#event/qr-host', 0, QR_Y + QR + 24 + 28 + 8, CONTENT_W, 26, 'Opens meet.google.com', 17, FONT.medium, C.ink, 'CENTER', 'TOP', { maxLines: 1, textTruncation: 'ENDING' });
      slotted(box, '#event/back', 0, ROW_Y, secondary, 'Back');
    } else if (page === 'confirm') {
      text(box, '#event/confirm-title', 0, 0, TITLE_W, 36, 'Delete this event?', 28, 'semibold', 'ink');
      text(box, '#event/confirm-text', 0, 60, CONTENT_W, 84, 'Only this occurrence.', 19, 'regular', 'ink');
      lines(box, 'Deleting the event…', 'Google could not be reached. Try again.');
      slotted(box, '#event/cancel', CONTENT_W - 2 * BTN_W - BTN_GAP, ROW_Y, secondary, 'Cancel');
      slotted(box, '#event/confirm', CONTENT_W - BTN_W, ROW_Y, dangerPrimary, 'Delete');
    } else {
      text(box, 'Calendar title', 0, 0, TITLE_W, 36, 'Google Calendar', 28, 'semibold', 'ink');
      text(box, '#event/access', 0, 60, CONTENT_W, 84, 'The panel may read and change this calendar: anyone here can answer its invitations and delete its events.', 19, 'regular', 'ink');
      text(box, 'Calendar hint', 0, 164, CONTENT_W, 96,
        'Change access ends the panel\'s access and shows the setup code: scan it to link the calendar again and choose read only, or read and change. Disconnect ends the access.',
        17, 'regular', 'ink2');
      lines(box, 'Disconnecting the calendar…', 'Google could not be reached. The calendar stays linked; try again.');
      slotted(box, '#event/change-access', 0, ROW_Y, secondary, 'Change access');
      slotted(box, '#event/disconnect', BTN_W + BTN_GAP, ROW_Y, secondary, 'Disconnect');
    }
  }
};
