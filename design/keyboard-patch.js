// keyboard-patch.js: adds the on-screen keyboard and the Wi-Fi password sheet to the
// "Smart HOme" Figma file, at the kiwi level.
//
//   node roundtrip.js "Smart HOme.fig" smarthome-keyboard.fig keyboard-patch.js
//
// (roundtrip.js decodes a .fig, calls this patch on the message and writes the .fig again.)
//
// Run after components-patch.js: the sheet's components are instances of its sets.
//
// Adds to Page 1:
// - #wifi: inside #smarthome-root, positioned absolutely over the dashboard: scrim, the card
//   with network name, the password field, Cancel/Connect, the reveal toggle and the keyboard
//   docked at the bottom, all library components in slots (the keyboard set #keyboard is
//   drawn by components-patch.js).
//
// Node names are the contract with the smarthome app (ui_model.rs): change both.
// Nothing is hidden in Figma: a node hidden there reaches the .dcf as display:none and harry
// never shows it again; the app hides #wifi until it is needed.

const path = require('path');
const key = g => `${g.sessionID}:${g.localID}`;
const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080;
const KBD_W = 1920, KBD_H = 420;

// The dashboard's palette (Smart HOme.fig).
const C = {
  ink: [0x24 / 255, 0x36 / 255, 0x4c / 255],       // primary text #24364c
  ink2: [0x6c / 255, 0x7e / 255, 0x93 / 255],      // secondary text #6c7e93
  accent: [0x42 / 255, 0x68 / 255, 0xb2 / 255],    // #4268b2
  accentSoft: [0xe6 / 255, 0xed / 255, 0xf9 / 255],// #e6edf9
  line: [0xb9 / 255, 0xcb / 255, 0xe5 / 255],      // #b9cbe5
  white: [1, 1, 1],
};
const font = style => ({ family: 'Inter', style, postscript: 'Inter-' + style.replace(' ', '') });
const FONT = { regular: font('Regular'), medium: font('Medium'), semibold: font('Semi Bold') };


// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
module.exports = function patch(msg, helpers) {
  const log = (...a) => console.log('  [keyboard]', ...a);
  const nodes = msg.nodeChanges;
  const kids = p => nodes.filter(n => n.parentIndex && key(n.parentIndex.guid) === p);
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  if (nodes.some(n => /^#wifi(\/|$)/.test(n.name || ''))) throw new Error('#wifi already present');

  // ---- authoring helpers: one node factory for the whole patch (one GUID session; a second
  // factory in the same patch would hand out the same GUIDs and Figma fails to import)
  const A = helpers.authoring(msg, 'keyboard');
  const { base, frame, rect, shadow } = A;
  const panel = (parent, name, x, y, w, h, rgb, alpha, r, extra) => {
    const f = frame(parent, name, x, y, w, h, extra);
    rect(f, 'bg', 0, 0, w, h, rgb, r, { opacity: alpha, effects: shadow(8, 32, 0.06) });
    return f;
  };
  const text = (parent, name, x, y, w, h, s, size, fnt, rgb, align = 'LEFT', valign = 'TOP') => A.text(parent, name, x, y, w, h, s, size, fnt, rgb, align, valign);

  // =====================================================================
  // 2. the Wi-Fi sheet over the dashboard: the library's #sheet (components-patch.js) as a
  // dialog, with the content component #wifi-content (step networks or password) swapped into
  // its content slot. The dashboard carries #wifi in the closed variant; the app keeps it open
  // and slides the card and fades the scrim itself (ui-lib sheet.rs), and switches the step on
  // #wifi/content. Figma's own prototype transitions are imported but harry does not read them.
  const lib = nodes.find(n => n.name === 'Library' && n.type === 'FRAME');
  if (!lib) throw new Error('Library missing: run components-patch.js first');
  const variantOf = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    if (!s) throw new Error(`${set} missing: run components-patch.js first`);
    const v = kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set}: no variant ${name}`);
    return v;
  };
  // A slot: a frame named for the app with an instance of the library component inside.
  // harry replaces the instance with a clone of the variant the app picks and customizes
  // the clone on its own (ui-lib harry/src/slot.rs). The instance is the preview for the
  // designer and makes the fetch take the component set; its texts are only the preview.
  const inst = (parent, name, x, y, variant, overrides = []) => base(parent, 'INSTANCE', name, x, y, variant.size.x, variant.size.y, {
    symbolData: { symbolID: variant.guid, symbolOverrides: [{ name, guidPath: { guids: [variant.guid] } }, ...overrides], uniformScaleFactor: 1 }, fillPaints: [], frameMaskDisabled: true,
    hasBeenManuallyRenamed: true,   // keeps the name on import (see fig-helpers.js)
  });
  const labelled = (variant, layer, s) => {
    const label = kids(key(variant.guid)).find(n => n.name === layer);
    return [{ guidPath: { guids: [label.guid] }, textData: { characters: s, lines: label.textData.lines } }];
  };
  const slotted = (parent, name, x, y, variant, overrides = []) => {
    const slot = frame(parent, name, x, y, variant.size.x, variant.size.y);
    inst(slot, name + '/preview', 0, 0, variant, overrides);
    return slot;
  };

  // The content of the Wi-Fi sheet: a component set #wifi-content, step networks or password
  // (the app's), as wide as the dialog's content area (components-patch.js #sheet: 1120, 40
  // inside) and as high as the step needs: the dialog's card hugs it. The sheet itself is the library's #sheet (dialog, in from the bottom); #wifi on
  // the dashboard is an instance of it with this content swapped in (sheetInstance). The
  // keyboard is not its: it is the screen's keyboard dock below, open while the password field
  // has the focus (the card stays where it is; the dock covers the empty lower part of it).
  const CONTENT_W = 1120 - 80;
  const BTN_H = 56, BTN_W = 200;
  // the rows: #list-item size=wide (960 x 72), 8 between them, 6 visible
  const NET_W = 960, NET_H = 72, NET_GAP = 8, NET_ROWS = 6, LIST_H = NET_ROWS * NET_H + (NET_ROWS - 1) * NET_GAP;
  // where the buttons sit in each step (the old card's height minus its padding and the button)
  const BTN_Y = { networks: 64 + LIST_H + 24, password: 256 };
  const CONTENT_H = { networks: BTN_Y.networks + BTN_H, password: BTN_Y.password + BTN_H };
  const networkRow = () => variantOf('#list-item', 'state=default, kind=icon, lines=two, size=wide');
  const primary = variantOf('#button', 'kind=primary, state=default, size=regular');
  const secondary = variantOf('#button', 'kind=secondary, state=default, size=regular');
  const SHEET_Y = lib.size.y + 80;
  const contentSet = A.componentSet(lib, '#wifi-content', 80, SHEET_Y, [['step', ['networks', 'password']]], CONTENT_W, CONTENT_H.networks, 48, 0, (s, v) => {
    const btnY = BTN_Y[v.step];
    // the component hugs this box: the step's own height (fig-helpers hugBox)
    const card = A.hugBox(s, CONTENT_W, CONTENT_H[v.step]);
    text(card, 'Title', 0, 0, 400, 32, 'Wi-Fi', 25, FONT.semibold, C.ink);
    slotted(card, '#wifi/cancel', CONTENT_W - BTN_W - 16 - BTN_W, btnY, secondary, labelled(secondary, '#button/label', 'Cancel'));
    if (v.step === 'networks') {
      // the list: a fixed slot, 6 rows of #network (harry puts the rows at absolute positions)
      text(card, 'Networks label', 0, 40, 200, 20, 'Networks', 15, FONT.regular, C.ink2);
      // The list slot clips (Clip content); the rows are #list-item kind=icon, 6 of them
      // visible, placed by harry at their scrolled y (ui-lib ListView).
      const list = frame(card, '#wifi/networks', 0, 64, NET_W, LIST_H, { frameMaskDisabled: false });
      for (let i = 0; i < NET_ROWS; i++) inst(list, `#wifi/networks/preview-${i + 1}`, 0, i * (NET_H + NET_GAP), networkRow());
      // the scrollbar to the right of the list, as high as it
      slotted(card, '#wifi/scrollbar', NET_W + 16, 64, variantOf('#scrollbar', `length=${LIST_H}`));
      slotted(card, '#wifi/rescan', CONTENT_W - BTN_W, btnY, secondary, labelled(secondary, '#button/label', 'Rescan'));
    } else {
      text(card, 'Network label', 0, 40, 200, 20, 'Network', 15, FONT.regular, C.ink2);
      text(card, '#wifi/ssid', 0, 62, 600, 30, 'faraway', 23, FONT.medium, C.ink);
      text(card, 'Password label', 0, 112, 200, 20, 'Password', 15, FONT.regular, C.ink2);
      slotted(card, '#wifi/connect', CONTENT_W - BTN_W, btnY, primary, labelled(primary, '#button/label', 'Connect'));
      // The password: the library's input field (880 wide), its helper line under it carries
      // the hint and the errors.
      slotted(card, '#wifi/password', 0, 136, variantOf('#input-field', 'state=default'));
      // Show the password: a toggle, so no label changes at runtime.
      slotted(card, '#wifi/reveal', 0, btnY + 8, variantOf('#toggle', 'state=default'));
      text(card, 'Reveal label', 84, btnY, 240, BTN_H, 'Show password', 17, FONT.regular, C.ink, 'LEFT', 'CENTER');
    }
  });
  const sheetSet = { set: contentSet.set };
  // The keyboard dock: the screen's one keyboard (as a system keyboard, not a part of each
  // sheet), closed below the screen or open at the bottom; the library's #keyboard sits in
  // its slot #osk/keys. The app opens it while a field has the focus (ui-lib KeyboardDock).
  const dockY = SHEET_Y + sheetSet.set.size.y + 80;
  const dockSet = A.componentSet(lib, '#keyboard-dock', 80, dockY, [['state', ['closed', 'open']]], SCREEN_W, SCREEN_H, 0, 48, (s, v) => {
    const open = v.state === 'open';
    slotted(s, '#osk/keys', 0, open ? SCREEN_H - KBD_H : SCREEN_H + 40, variantOf('#keyboard', 'state=default'));
  }, { rows: true });
  // the Library grows for the two sets
  lib.size = { x: lib.size.x, y: dockY + dockSet.set.size.y + 80 };
  // The sheet and the dock on the dashboard: the closed variants, over everything, the dock
  // above the sheet. #smarthome-root is a vertical auto layout frame; Figma's import keeps
  // "absolute position" on a frame but drops it on an instance (v22: the instance landed in
  // the column's flow, below the screen), so each instance sits in a plain absolute frame and
  // is positioned in it by its constraints.
  const layer = frame(root, 'Wi-Fi sheet layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#wifi', 0, 0, variantOf('#sheet', 'layout=dialog, state=closed'), contentSet.symbols['step=networks']);
  const dockLayer = frame(root, 'Keyboard layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.instance(dockLayer, '#osk', 0, 0, dockSet.symbols['state=closed']);

  log(`added ${A.count()} nodes: #wifi-content (step) and #keyboard-dock (state) in the Library, #wifi (#sheet dialog, content swapped) and #osk over ${ROOT}`);
  return msg;
};
