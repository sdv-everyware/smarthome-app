// event-patch.js: the panel of an agenda event, an instance of the library's #sheet
// (components-patch.js) at the right edge with its own content component, at the kiwi level.
// Run after prefs-patch.js (build.sh), so the event's layer is above the settings.
// A tap on an event's row of #agenda/list opens it (the app, agenda_card.rs).
//
// Adds:
// - #event-content in the Library: the content, the size of the right panel's content area:
//   #event/close (a tonal icon button) at the top right, and #event/scroll, a frame that clips,
//   with #event/body in it: a column (auto layout, 16 between the parts) of texts that grow with
//   their content: #event/title (the whole title, beside the close button), #event/when (the day
//   and the time), #event/place, #event/link (the meeting's address, as text) and
//   #event/description. The app takes a part out of the column when it is empty (display gone),
//   measures the column's height and moves #event/body up as the finger scrolls it (agenda_card.rs
//   EventPanel; state.rs has the same numbers as here).
// - #event over #smarthome-root: #sheet layout=right, state=closed, in a plain absolute frame,
//   its content swapped for #event-content and its parts named #event/scrim, /card, /content
//   (fig-helpers.js sheetInstance). The app keeps it closed and moves the card itself.

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
  ['#event/link', CONTENT_W, 17, 24, 'regular', 'accent', 'https://meet.google.com/abc-defg-hij'],
  ['#event/description', CONTENT_W, 19, 28, 'regular', 'ink',
    'Bring the insurance card and the list of the medicines you take.\n\nThe practice is on the second floor; the lift is at the back of the courtyard. ' +
    'Please come ten minutes early to fill in the form.\n\nTo move the appointment, call the practice the day before.'],
];
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

  const y0 = lib.size.y + 80;
  const content = A.componentSet(lib, '#event-content', 80, y0, [['state', ['default']]], CONTENT_W, CONTENT_H, 0, 0, (s) => {
    const box = A.hugBox(s, CONTENT_W, CONTENT_H);
    // the place the body scrolls in: it clips; the close button stays above it
    const scroll = frame(box, '#event/scroll', 0, 0, CONTENT_W, CONTENT_H, { frameMaskDisabled: false });
    const heights = PARTS.map(([, w, size, lh, , , text]) => previewH(text, w, size, lh));
    const bodyH = heights.reduce((a, h) => a + h, 0) + GAP * (PARTS.length - 1);
    const body = frame(scroll, '#event/body', 0, 0, CONTENT_W, bodyH, {
      bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: GAP, stackPrimarySizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE',
      stackCounterSizing: 'FIXED', stackLayoutVersion: 1, frameMaskDisabled: true,
    });
    let y = 0;
    PARTS.forEach(([name, w, size, lh, weight, colour, text], i) => {
      // a text as wide as its place that grows down with its lines (harry measures it in layout)
      A.text(body, name, 0, y, w, heights[i], text, size, FONT[weight], C[colour], 'LEFT', 'TOP', {
        lineHeight: { value: lh, units: 'PIXELS' }, textAutoResize: 'HEIGHT',
      });
      y += heights[i] + GAP;
    });
    const slot = frame(box, '#event/close', CONTENT_W - close.size.x + 12, -16, close.size.x, close.size.y);
    A.instance(slot, '#event/close/preview', 0, 0, close);
  }, { rows: true });
  lib.size = { x: lib.size.x, y: y0 + content.set.size.y + 80 };

  const layer = frame(root, 'Event layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#event', 0, 0, closedRight, content.symbols['state=default']);

  log(`added ${A.count()} nodes: #event-content in the Library, #event (#sheet right, closed, content swapped) over ${ROOT}`);
  return msg;
};
