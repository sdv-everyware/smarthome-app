// game-patch.js: the game on the dashboard, at the kiwi level: a button in the status row and a
// dialog (the library's #sheet) with the game's picture and its keys. Run after event-patch.js
// and before prototype-patch.js (build.sh).
//
// Adds:
// - #status/game in the status row ('Status controls', dashboard-patch.js), left of
//   #status/wifi: a slot with a glass #icon-button, the gamepad glyph, as the Wi-Fi indicator.
// - #game-content in the Library: the dialog's content. A title bar (the game's name; #game/close,
//   a tonal icon button), under it the picture with the keys at its sides: #game/up, /left,
//   /right, /down (a direction pad of tonal icon buttons with chevrons) at the left, #game/fire
//   and #game/use (#button, compact) at the right. The picture is #game/picture, a plain frame
//   that clips: the app turns it into a Wayland zone (harry draws an external image only on a
//   plain frame, as #camera/feed) where the game process shows its frames, 4:3 as DOOM is
//   shown. Under it #game/screen, dark, with #game/status, which the app writes until the
//   game's first frame covers it.
// - #game over #smarthome-root: #sheet layout=dialog, state=closed, in a plain absolute frame,
//   its content swapped for #game-content and its parts named #game/scrim, /card, /content
//   (fig-helpers.js sheetInstance). The app keeps it closed and moves the card itself.
//
// The layout is the user's to decide: every place is a constant below.

let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080;
// the content of the library's dialog (components-patch.js #sheet: 1120 wide, 40 inside)
const CONTENT_W = 1120 - 80;
// the title bar's height; the picture and the keys under it
const BAR_H = 64;
// an icon button's touch target (#icon-button: 72, the circle 56 in it): the pad is 3 x 3 of them
const CELL = 72, PAD_W = 3 * CELL;
// #button compact: 116 x 56
const BTN_W = 116, BTN_H = 56, BTN_GAP = 24;
// the picture between the pad and the buttons, 4:3 (DOOM's 320 x 200 is shown 4:3)
const GAP = 24;
const PIC_W = CONTENT_W - PAD_W - BTN_W - 2 * GAP, PIC_H = Math.round(PIC_W * 3 / 4);
const PIC_X = PAD_W + GAP, BODY_Y = BAR_H;
const CONTENT_H = BAR_H + PIC_H;
// the status row's frame and the slot the game button goes before (dashboard-patch.js)
const STATUS_ROW = 'Status controls', BEFORE = '#status/wifi', STATUS_GAP = 12;

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'game');
  const { nodes, kids, log, frame, rect } = A;
  if (nodes.some(n => /^#game(\/|-content|$)/.test(n.name || '') || n.name === '#status/game')) throw new Error('#game already present');
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  const lib = nodes.find(n => n.name === 'Library' && n.type === 'FRAME');
  if (!lib) throw new Error('Library missing: run components-patch.js first');
  const variantOf = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    if (!s) throw new Error(`${set} missing: run components-patch.js first`);
    const v = kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set}: no variant ${name}`);
    return v;
  };
  const labelled = (variant, s) => {
    const label = kids(key(variant.guid)).find(n => n.name === '#button/label');
    if (!label) throw new Error(`${variant.name}: no #button/label`);
    return [{ guidPath: { guids: [label.guid] }, textData: { characters: s, lines: label.textData.lines } }];
  };
  // a slot: a frame named for the app with a preview instance of the library component inside
  // (ui-lib harry/src/slot.rs clones the variant the app picks into it)
  const slotted = (parent, name, x, y, variant, overrides = []) => {
    const slot = frame(parent, name, x, y, variant.size.x, variant.size.y);
    A.instance(slot, name + '/preview', 0, 0, variant, undefined, overrides);
    return slot;
  };
  const padButton = icon => variantOf('#icon-button', `kind=tonal, state=default, icon=${icon}, size=regular`);
  const close = variantOf('#icon-button', 'kind=tonal, state=default, icon=x, size=regular');
  const fire = variantOf('#button', 'kind=primary, state=default, size=compact');
  const use = variantOf('#button', 'kind=secondary, state=default, size=compact');

  // 1. the game button in the status row, left of the Wi-Fi indicator
  const row = nodes.find(n => n.name === STATUS_ROW);
  const before = row && kids(key(row.guid)).find(n => n.name === BEFORE);
  if (!before) throw new Error(`${STATUS_ROW} / ${BEFORE} not found: run dashboard-patch.js first`);
  const button = frame(row, '#status/game', 0, 0, CELL, CELL, {
    bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackPrimarySizing: 'FIXED', stackPrimaryAlignItems: 'CENTER',
    stackCounterAlignItems: 'CENTER', stackLayoutVersion: 1,
  });
  // in the row's order just before the Wi-Fi slot: a position between its earlier sibling's and its own
  const earlier = kids(key(row.guid)).map(n => n.parentIndex.position).filter(p => p < before.parentIndex.position).sort().pop() || '';
  button.parentIndex = { guid: row.guid, position: between(earlier, before.parentIndex.position) };
  A.instance(button, '#status/game/preview', 0, 0, variantOf('#icon-button', 'kind=glass, state=default, icon=gamepad, size=compact'));
  row.size = { x: row.size.x + CELL + STATUS_GAP, y: row.size.y };

  // 2. the dialog's content
  const y = lib.size.y + 80;
  const content = A.componentSet(lib, '#game-content', 80, y, [['state', ['default']]], CONTENT_W, CONTENT_H, 0, 0, (s) => {
    const box = A.hugBox(s, CONTENT_W, CONTENT_H);
    // the title bar: the game's name, close at the right (as #device/close)
    A.text(box, '#game/title', 0, 0, CONTENT_W - close.size.x - 16, 40, 'Freedoom', 28, FONT.semibold, C.ink,
      'LEFT', 'CENTER', { maxLines: 1, textTruncation: 'ENDING' });
    slotted(box, '#game/close', CONTENT_W - close.size.x + 12, -16, close);
    // the direction pad, in the middle of the picture's height
    const pad = frame(box, 'Direction pad', 0, BODY_Y + (PIC_H - PAD_W) / 2, PAD_W, PAD_W);
    slotted(pad, '#game/up', CELL, 0, padButton('chevron-up'));
    slotted(pad, '#game/left', 0, CELL, padButton('chevron-left'));
    slotted(pad, '#game/right', 2 * CELL, CELL, padButton('chevron-right'));
    slotted(pad, '#game/down', CELL, 2 * CELL, padButton('chevron-down'));
    // the screen: dark, the status in its middle until the game's frames cover it
    const screen = frame(box, '#game/screen', PIC_X, BODY_Y, PIC_W, PIC_H, {
      fillPaints: [A.solid([0.06, 0.07, 0.09])], cornerRadius: 12, frameMaskDisabled: false,
    });
    A.text(screen, '#game/status', 24, (PIC_H - 32) / 2, PIC_W - 48, 32, 'Starting the game', 17, FONT.regular, C.muted, 'CENTER', 'CENTER',
      { maxLines: 1, textTruncation: 'ENDING' });
    // the picture: a plain frame over the screen, no fill (the zone's frames are the picture)
    frame(box, '#game/picture', PIC_X, BODY_Y, PIC_W, PIC_H, { frameMaskDisabled: false });
    // Fire and Use, in the middle of the picture's height
    const bx = CONTENT_W - BTN_W, by = BODY_Y + (PIC_H - 2 * BTN_H - BTN_GAP) / 2;
    slotted(box, '#game/fire', bx, by, fire, labelled(fire, 'Fire'));
    slotted(box, '#game/use', bx, by + BTN_H + BTN_GAP, use, labelled(use, 'Use'));
  }, { rows: true });
  lib.size = { x: lib.size.x, y: y + content.set.size.y + 80 };

  // 3. the dialog over the dashboard, closed
  const layer = frame(root, 'Game layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#game', 0, 0, variantOf('#sheet', 'layout=dialog, state=closed'), content.symbols['state=default']);

  log(`added ${A.count()} nodes: #status/game before ${BEFORE}; #game-content ${CONTENT_W}x${CONTENT_H} (picture ${PIC_W}x${PIC_H}) in the Library; #game (#sheet dialog, content swapped) over ${ROOT}`);
  return msg;
};

// A fractional index between two (Figma's child order: positions compare as strings). `lo`
// may be empty (before every sibling).
function between(lo, hi) {
  let out = '';
  for (let i = 0; ; i++) {
    const a = i < lo.length ? lo.charCodeAt(i) : 0x20, b = i < hi.length ? hi.charCodeAt(i) : 0x7e;
    if (b - a > 1) return out + String.fromCharCode(a + Math.ceil((b - a) / 2));
    out += String.fromCharCode(a);
  }
}

module.exports.CONTENT = { w: CONTENT_W, h: CONTENT_H };
