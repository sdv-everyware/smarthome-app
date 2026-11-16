// prefs-patch.js: the settings of the panel, an instance of the library's #sheet
// (components-patch.js) at the right edge with its own content component, at the kiwi level.
// Run after panel-patch.js (build.sh), so the settings' layer is above the browser's panel.
// The names start with #prefs: #settings is the button at the bottom that opens it.
//
// Adds:
// - #prefs-content in the Library: the content, the size of the right panel's content area:
//   a title bar (Settings, #prefs/close, a tonal icon button), then the rows by section. A row
//   is a label, a value the app writes (#prefs/<row>/value) or the library's control in a slot:
//     Time        #prefs/timezone (the row, a tap target) with #prefs/timezone/value
//                 #prefs/hour24, a #toggle
//     Weather     #prefs/units, a #segmented of two (°C, °F: labels set by the app)
//     Network     #prefs/wifi (the row, a tap target) with #prefs/wifi/value
//     About       #prefs/about, lines the app writes (version, address)
//   on the page #prefs/page-main; the page #prefs/page-timezone is the time zone list (as Car Settings'
//   picker): #prefs/back, a ghost #button "‹ Back", the title, then #prefs/zones, a clipping
//   list slot of #list-item size=panel rows (ui-lib ListView), and #prefs/zones-scrollbar beside
//   it. #prefs/close is above both. The app slides the pages (ui-lib screens.rs Pages).
// - #prefs over #smarthome-root: #sheet layout=right, state=closed, in a plain absolute frame,
//   its content swapped for #prefs-content and its parts named #prefs/scrim, /card, /content
//   (fig-helpers.js sheetInstance). The app keeps it closed and moves the card itself.

let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080, PAD = 40;
// the right panel's content area (components-patch.js: half the width, full height, 40 inside)
const CONTENT_W = SCREEN_W / 2 - 2 * PAD, CONTENT_H = SCREEN_H - 2 * PAD;
const ROW_H = 72, SECTION_GAP = 32;

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'prefs');
  const { nodes, kids, log, frame } = A;
  if (nodes.some(n => n.name === '#prefs' || n.name === '#prefs-content')) throw new Error('#prefs already present');
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
  const toggle = variant('#toggle', 'state=default');
  const segmented = variant('#segmented', 'segments=2, state=default');
  const back = variant('#button', 'kind=ghost, state=default, size=compact');
  const zoneRow = (state) => variant('#list-item', `state=${state}, kind=plain, lines=two, size=panel`);
  // the time zone list: rows 72 high, 8 between them (a pitch of 80), from under the title to
  // the bottom of the content (the app reads the height from the design)
  const ZONE_PITCH = 80, ZONES_Y = 72, ZONES_H = CONTENT_H - ZONES_Y;
  const labelled = (v, layer, text) => {
    const l = kids(key(v.guid)).find(n => n.name === layer);
    if (!l) throw new Error(`${v.name}: no ${layer}`);
    return [{ guidPath: { guids: [l.guid] }, textData: { characters: text, lines: l.textData.lines } }];
  };

  // a slot: a frame named for the app with the library's variant inside (use-component skill)
  const slot = (parent, name, x, y, v) => {
    const f = frame(parent, name, x, y, v.size.x, v.size.y);
    A.instance(f, name + '/preview', 0, 0, v);
    return f;
  };
  const label = (parent, name, x, y, w, s, size, fnt, rgb, align = 'LEFT') =>
    A.text(parent, name, x, y, w, ROW_H, s, size, fnt, rgb, align, 'CENTER', { maxLines: 1, textTruncation: 'ENDING' });

  const y0 = lib.size.y + 80;
  const content = A.componentSet(lib, '#prefs-content', 80, y0, [['state', ['default']]], CONTENT_W, CONTENT_H, 0, 0, (s) => {
    const box = A.hugBox(s, CONTENT_W, CONTENT_H);
    // The two pages lie side by side in a frame that cuts what is out of the content's width, the
    // second one out of view to the right: the app slides them (ui-lib screens.rs Pages,
    // Effect::Slide), as Android's Settings goes one page deeper. Both are drawn at full
    // opacity: harry takes no taps on what the design draws transparent. The frame starts 16
    // above the content for the title bar's buttons.
    const pages = frame(box, 'Pages', 0, -16, CONTENT_W, CONTENT_H + 16, { frameMaskDisabled: false });
    {
      // the settings
      const page = frame(pages, '#prefs/page-main', 0, 16, CONTENT_W, CONTENT_H);
      // the title bar, as #panel's
      A.text(page, 'Settings title', 0, 0, CONTENT_W - close.size.x - 16, 40, 'Settings', 28, FONT.semibold, C.ink, 'LEFT', 'CENTER');
      let y = 64;
      const section = (title) => {
        y += SECTION_GAP;
        A.text(page, `${title} section`, 0, y, CONTENT_W, 24, title.toUpperCase(), 15, FONT.medium, C.ink2);
        y += 32;
      };
      const divider = () => A.rect(page, 'Divider', 0, y, CONTENT_W, 1, C.line, 0);
      // a row the user taps: the label, the value at the right, a chevron
      const tapRow = (name, text, value) => {
        const row = frame(page, name, 0, y, CONTENT_W, ROW_H);
        label(row, `${text} label`, 0, 0, 360, text, 21, FONT.regular, C.ink);
        label(row, name + '/value', 360, 0, CONTENT_W - 360 - 40, value, 21, FONT.regular, C.ink2, 'RIGHT');
        label(row, 'Chevron', CONTENT_W - 24, 0, 24, '›', 28, FONT.regular, C.ink2, 'RIGHT');
        y += ROW_H;
        divider();
      };
      // a row with a control in a slot at the right
      const controlRow = (name, text, v) => {
        label(page, `${text} label`, 0, y, CONTENT_W - v.size.x - 16, text, 21, FONT.regular, C.ink);
        slot(page, name, CONTENT_W - v.size.x, y + (ROW_H - v.size.y) / 2, v);
        y += ROW_H;
        divider();
      };

      section('Time');
      tapRow('#prefs/timezone', 'Time zone', 'Europe/Berlin');
      controlRow('#prefs/hour24', '24-hour clock', toggle);
      section('Weather');
      controlRow('#prefs/units', 'Temperature', segmented);
      section('Network');
      tapRow('#prefs/wifi', 'Wi-Fi', 'Not connected');
      section('About');
      A.text(page, '#prefs/about', 0, y, CONTENT_W, 3 * 30, 'Version\nAddress', 19, FONT.regular, C.ink2, 'LEFT', 'TOP');
    }
    {
      // the time zone list (as Car Settings' picker), out of view until a tap on the time zone
      const page = frame(pages, '#prefs/page-timezone', CONTENT_W, 16, CONTENT_W, CONTENT_H);
      // the title bar: back at the left, the title
      const b = frame(page, '#prefs/back', 0, -8, back.size.x, back.size.y);
      A.instance(b, '#prefs/back/preview', 0, 0, back, undefined, labelled(back, '#button/label', '‹ Back'));
      A.text(page, 'Time zone title', back.size.x + 8, 0, CONTENT_W - back.size.x - close.size.x - 24, 40, 'Time zone', 28, FONT.semibold, C.ink, 'LEFT', 'CENTER');
      // the list slot clips; harry places the real rows at their scrolled y (ui-lib ListView)
      const row = zoneRow('default');
      const list = frame(page, '#prefs/zones', 0, ZONES_Y, row.size.x, ZONES_H, { frameMaskDisabled: false });
      const preview = [['Honolulu', 'GMT-10:00 · Pacific'], ['New York', 'GMT-05:00 · America'], ['London', 'GMT+00:00 · Europe'],
        ['UTC', 'GMT+00:00 · Etc'], ['Amsterdam', 'GMT+01:00 · Europe'], ['Helsinki', 'GMT+02:00 · Europe'], ['Moscow', 'GMT+03:00 · Europe'],
        ['Dubai', 'GMT+04:00 · Asia'], ['Kolkata', 'GMT+05:30 · Asia'], ['Tokyo', 'GMT+09:00 · Asia']];
      preview.forEach(([title, sub], i) => {
        const r = zoneRow(title === 'Amsterdam' ? 'selected' : 'default');
        A.instance(list, `#prefs/zones/preview-${i + 1}`, 0, i * ZONE_PITCH, r, undefined,
          [...labelled(r, '#list-item/title', title), ...labelled(r, '#list-item/subtitle', sub)]);
      });
      A.scrollbarSlot(page, '#prefs/zones-scrollbar', CONTENT_W - 6, ZONES_Y, ZONES_H);
    }
    // close, above both pages
    slot(box, '#prefs/close', CONTENT_W - close.size.x + 12, -16, close);
  }, { rows: true });
  lib.size = { x: lib.size.x, y: y0 + content.set.size.y + 80 };

  const layer = frame(root, 'Settings layer', 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
  A.sheetInstance(layer, '#prefs', 0, 0, closedRight, content.symbols['state=default']);

  log(`added ${A.count()} nodes: #prefs-content in the Library, #prefs (#sheet right, closed, content swapped) over ${ROOT}`);
  return msg;
};
