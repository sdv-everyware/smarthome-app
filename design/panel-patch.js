// panel-patch.js: the panel on the dashboard, an instance of the library's #sheet
// (components-patch.js) at the right edge with its own content component, at the kiwi level.
// Run after sheet-patch.js (build.sh), so the panel's layer is above the other sheets.
//
// Adds:
// - #panel-content in the Library: the panel's content, the size of the right panel's content
//   area: a title bar (#panel/title, one line with an ellipsis; #panel/close, a tonal icon
//   button) and under it #panel/browser, a plain white frame with its own clip: the app turns it
//   into a Wayland zone (harry draws an external image only on a plain frame, as #camera/feed),
//   where the browser shows a page. Over it #panel/loading, a white frame
//   with #skeleton slots (#panel/skeleton-1..4: a heading, text, a picture, text) that the app
//   shows and sweeps while the page loads and fades out when it shows.
// - #panel over #smarthome-root: #sheet layout=right, state=closed, in a plain absolute frame
//   (Figma's import drops "absolute position" on an instance, keyboard-patch.js), its content
//   swapped for #panel-content and its parts named #panel/scrim, /card, /content
//   (fig-helpers.js sheetInstance). The app keeps it open and moves the card itself.

const path = require('path');
// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080, PAD = 40;
// the right panel's content area (components-patch.js: half the width, full height, 40 inside)
const CONTENT_W = SCREEN_W / 2 - 2 * PAD, CONTENT_H = SCREEN_H - 2 * PAD;
// the title bar's height; the browser takes the rest (880 x 936)
const BAR_H = 64;

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'panel');
  const { nodes, kids, log, frame } = A;
  if (nodes.some(n => n.name === '#panel' || n.name === '#panel-content')) throw new Error('#panel already present');
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  const lib = nodes.find(n => n.name === 'Library' && n.type === 'FRAME');
  if (!lib) throw new Error('Library missing: run components-patch.js first');
  const sheetSet = nodes.find(n => n.isStateGroup && n.name === '#sheet');
  if (!sheetSet) throw new Error('#sheet missing: run components-patch.js first');
  const closedRight = kids(key(sheetSet.guid)).find(n => n.name === 'layout=right, state=closed');
  const iconSet = nodes.find(n => n.isStateGroup && n.name === '#icon-button');
  const close = iconSet && kids(key(iconSet.guid)).find(n => n.name === 'kind=tonal, state=default, icon=x, size=regular');
  if (!close) throw new Error('#icon-button tonal x missing: run components-patch.js first');
  const skSet = nodes.find(n => n.isStateGroup && n.name === '#skeleton');
  const sk = kind => skSet && kids(key(skSet.guid)).find(n => n.name === `kind=${kind}, size=wide`);
  if (!sk('text')) throw new Error('#skeleton missing: run components-patch.js first');

  const y = lib.size.y + 80;
  const panelContent = A.componentSet(lib, '#panel-content', 80, y, [['state', ['default']]], CONTENT_W, CONTENT_H, 0, 0, (s) => {
    const box = A.hugBox(s, CONTENT_W, CONTENT_H);
    // the title bar: the page's title, close at the right (as #device/close)
    A.text(box, '#panel/title', 0, 0, CONTENT_W - close.size.x - 16, 40, 'A page on the web', 28, FONT.semibold, C.ink,
      'LEFT', 'CENTER', { maxLines: 1, textTruncation: 'ENDING' });
    const slot = frame(box, '#panel/close', CONTENT_W - close.size.x + 12, -16, close.size.x, close.size.y);
    A.instance(slot, '#panel/close/preview', 0, 0, close);
    // the browser: a plain frame, white until the page's first frame
    frame(box, '#panel/browser', 0, BAR_H, CONTENT_W, CONTENT_H - BAR_H, {
      fillPaints: [{ type: 'SOLID', color: { r: 1, g: 1, b: 1, a: 1 }, opacity: 1, visible: true, blendMode: 'NORMAL' }],
      frameMaskDisabled: false,
    });
    // the loader over it: skeleton slots (frames named for the app with a preview instance),
    // the page's shape while it loads
    const loading = frame(box, '#panel/loading', 0, BAR_H, CONTENT_W, CONTENT_H - BAR_H, {
      fillPaints: [{ type: 'SOLID', color: { r: 1, g: 1, b: 1, a: 1 }, opacity: 1, visible: true, blendMode: 'NORMAL' }],
      frameMaskDisabled: false,
    });
    let y = 24;
    [['heading', 24], ['text', 36], ['media', 32], ['text', 0]].forEach(([kind, gap], i) => {
      const v = sk(kind), name = `#panel/skeleton-${i + 1}`;
      // centred: 24 a side for the wide variant (832 in 880)
      const slot = frame(loading, name, (CONTENT_W - v.size.x) / 2, y, v.size.x, v.size.y);
      A.instance(slot, name + '/preview', 0, 0, v);
      y += v.size.y + gap;
    });
  }, { rows: true });
  lib.size = { x: lib.size.x, y: y + panelContent.set.size.y + 80 };

  const layer = frame(root, 'Panel layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#panel', 0, 0, closedRight, panelContent.symbols['state=default']);

  log(`added ${A.count()} nodes: #panel-content in the Library, #panel (#sheet right, closed, content swapped) over ${ROOT}`);
  return msg;
};
