// sheet-patch.js: adds the device sheet to the "Smart HOme" Figma file, at the kiwi level.
// Run after components-patch.js (its #list-item kind text, #symbol, #icon-button icons x and
// pencil, #input-field, #button, #scrollbar) and after keyboard-patch.js / dashboard-patch.js (build.sh).
//
// A tap on a tile's header (#climate/header, #entryway/header, ...: dashboard-patch.js) opens
// the sheet of that device, as Apple's Home opens an accessory's card: the device's state large
// at the top, under it the details as grouped label and value rows, State on the left, Device
// on the right; "Show more" adds the diagnostics to the Device list. The pencil beside ✕ (or a
// tap on the name) puts an input field in the name's place: the house's name for the device.
//
// Adds:
// - #device-content in the Library: the sheet's content, the size of the dialog's content area
//   of the library's #sheet (components-patch.js), in two variants: state=default with the name
//   as a text and the pencil, state=rename with the field #device/name in their place.
// - #device on the dashboard: an instance of #sheet (dialog, closed) over #smarthome-root with
//   this content swapped in and its parts named #device/scrim, /card, /content (sheetInstance);
//   the app keeps it open and slides the card itself (ui-lib sheet.rs).
//
// Node names are the contract with the smarthome app: change both. Nothing is hidden in Figma
// (a node hidden there reaches the .dcf as display:none); the app hides #device until needed.

const path = require('path');
// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080;
// The card: two columns of #list-item size=column (490), 6 rows each (72 + 8: 472, the length of
// #scrollbar), the scrollbar 16 to the right of each list.
const ROW_W = 490, ROW_H = 72, ROW_GAP = 8, ROWS = 6, LIST_H = ROWS * ROW_H + (ROWS - 1) * ROW_GAP;
const COL_GAP = 50, SB_GAP = 16;
// the content of the library's dialog (components-patch.js #sheet: 1120 x 840, 40 inside)
const CONTENT_W = 1120 - 80, CONTENT_H = 840 - 80, LIST_Y = 216, MORE_Y = LIST_Y + LIST_H + 16, BTN_H = 56;
const COL2_X = ROW_W + COL_GAP;

// The preview: the thermostat as the generator starts (tools/matter-generator).
const STATE_ROWS = [['Room temperature', '21.4 °C'], ['Heating setpoint', '22.0 °C'], ['Cooling setpoint', '26.0 °C'], ['Mode', 'Heat'], ['Running', 'Heating'], ['Heating limits', '7.0 – 30.0 °C']];
const DEVICE_ROWS = [['Vendor', 'EveryWare'], ['Product', 'Virtual Home'], ['Serial number', 'EW-VH-0001'], ['Firmware', '0.1.0'], ['Online', '2 s ago'], ['Node · endpoint', '0x12 · 1']];

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'sheet');
  const { nodes, kids, log, frame, rect, shadow } = A;
  if (nodes.some(n => /^#device(\/|-content|$)/.test(n.name || ''))) throw new Error('#device already present');
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
  // the text of `layer` in a variant, for an instance's preview
  const textOverride = (variant, layer, s) => {
    const label = kids(key(variant.guid)).find(n => n.name === layer);
    if (!label) throw new Error(`${variant.name}: no ${layer}`);
    return { guidPath: { guids: [label.guid] }, textData: { characters: s, lines: label.textData.lines } };
  };
  // A slot: a frame named for the app with a preview instance of the library component inside
  // (ui-lib harry/src/slot.rs clones the variant the app picks into it).
  const slotted = (parent, name, x, y, variant, overrides = []) => {
    const slot = frame(parent, name, x, y, variant.size.x, variant.size.y);
    A.instance(slot, name + '/preview', 0, 0, variant, undefined, overrides);
    return slot;
  };
  const text = (parent, name, x, y, w, h, s, size, fnt, rgb, align = 'LEFT', valign = 'TOP', extra) => A.text(parent, name, x, y, w, h, s, size, fnt, rgb, align, valign, extra);

  const row = variantOf('#list-item', 'state=default, kind=text, lines=one, size=column');
  const close = variantOf('#icon-button', 'kind=tonal, state=default, icon=x, size=regular');
  const more = variantOf('#button', 'kind=ghost, state=default, size=regular');
  const symbol = variantOf('#symbol', 'icon=house');
  const edit = variantOf('#icon-button', 'kind=tonal, state=default, icon=pencil, size=regular');
  const field = variantOf('#input-field', 'state=default');

  // a list: a clipping slot with 6 preview rows (harry places the rows at their scrolled y,
  // ui-lib ListView), its scrollbar to the right
  const list = (card, name, x, rows) => {
    const l = frame(card, name, x, LIST_Y, ROW_W, LIST_H, { frameMaskDisabled: false });
    rows.forEach(([k, v], i) => A.instance(l, `${name}/preview-${i + 1}`, 0, i * (ROW_H + ROW_GAP), row, undefined,
      [textOverride(row, '#list-item/title', k), textOverride(row, '#list-item/trailing-text', v)]));
    A.scrollbarSlot(card, `${name}-scrollbar`, x + ROW_W + SB_GAP, LIST_Y, LIST_H);
  };

  const SHEET_Y = lib.size.y + 80;
  const contentSet = A.componentSet(lib, '#device-content', 80, SHEET_Y, [['state', ['default', 'rename']]], CONTENT_W, CONTENT_H, 80, 0, (s, v) => {
    const card = A.hugBox(s, CONTENT_W, CONTENT_H);
    // the header: the symbol, the name, the kind and the model; close at the right
    slotted(card, '#device/symbol', 0, 0, symbol);
    const closeX = CONTENT_W - close.size.x + 12;
    if (v.state === 'rename') {
      // the name as a field, where the name and the model were: the field's box (76 high) is
      // centred on the symbol, its helper line stays empty (the app's) above the state
      slotted(card, '#device/name', 64, -14, field, [textOverride(field, '#input-field/placeholder', 'Name')]);
    } else {
      text(card, '#device/title', 64, 0, 700, 32, 'Home climate', 25, FONT.semibold, C.ink, 'LEFT', 'TOP', { maxLines: 1, textTruncation: 'ENDING' });
      text(card, '#device/subtitle', 64, 34, 700, 20, 'Thermostat · EveryWare Virtual Home', 15, FONT.regular, C.ink2);
      // rename: the pencil, left of ✕
      slotted(card, '#device/edit', closeX - edit.size.x + 8, -16, edit);
    }
    slotted(card, '#device/close', closeX, -16, close);
    // the state, large: what the device does now
    text(card, '#device/state', 0, 72, 900, 56, '21.4 °C', 44, FONT.medium, C.ink);
    text(card, '#device/state-detail', 0, 132, 1000, 24, 'Heating to 22.0 °C · 45 % humidity', 17, FONT.regular, C.ink2);
    // the details: State (what the device reports about its function) and Device (what it is)
    text(card, 'State label', 0, 184, 300, 20, 'State', 15, FONT.regular, C.ink2);
    text(card, 'Device label', COL2_X, 184, 300, 20, 'Device', 15, FONT.regular, C.ink2);
    list(card, '#device/state-list', 0, STATE_ROWS);
    list(card, '#device/info-list', COL2_X, DEVICE_ROWS);
    // the diagnostics (uptime, last reboot, network) join the Device list on "Show more"
    slotted(card, '#device/more', COL2_X, MORE_Y, more, [textOverride(more, '#button/label', 'Show more')]);
  }, { rows: true });
  lib.size = { x: lib.size.x, y: SHEET_Y + contentSet.set.size.y + 80 };

  // On the dashboard: the library's #sheet, dialog and closed, with this content, in a plain
  // absolute frame (Figma's import drops "absolute position" on an instance, keyboard-patch.js).
  const layer = frame(root, 'Device sheet layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#device', 0, 0, variantOf('#sheet', 'layout=dialog, state=closed'), contentSet.symbols['state=default']);

  log(`added ${A.count()} nodes: #device-content ${CONTENT_W}x${CONTENT_H} in the Library, #device (#sheet dialog, content swapped) over ${ROOT}`);
  return msg;
};

// the preview rows, for prototype-patch.js
module.exports.STATE_ROWS = STATE_ROWS;
module.exports.DEVICE_ROWS = DEVICE_ROWS;
module.exports.ROWS = ROWS;
