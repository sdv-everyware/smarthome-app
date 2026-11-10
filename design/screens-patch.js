// screens-patch.js: the dashboard's tabs and the screens Rooms and Automations, at the kiwi
// level. Run after dashboard-patch.js (build.sh) and before sheet-patch.js: the sheets'
// layers stay above the screens.
//
// The header's drawn tab bar (Home status / View selector) becomes the slot #home/tabs with
// the library's #tabs (tabs=3: Overview, Rooms, Automations). The Overview is the dashboard as
// it was, its content frame named #overview. Rooms and Automations are screens of the app's
// design, like the sheets: a component set each (#rooms-screen, #automations-screen) with
// state=hidden (the content at opacity 0) and state=shown, and an instance of it (#rooms,
// #automations) over #overview in a plain absolute frame of #smarthome-root. The app shows a
// tab by its variant (harry fades the change) and hides the other screens, #overview among
// them (harry's visibility customization); the header and the bottom bar stay. Nothing is
// hidden in Figma: the screens are visible frames in prototype-patch.js.
//
// Rooms: the room tabs (#tabs, 4), the room's name and summary, up to 8 device tiles (#tile in
// the slots #rooms/tile-<i>, 4 a row; a tap opens the device's sheet), the room's scenes
// (#button secondary). Automations: the scenes and the recent activity as the user's original
// design draws them (the Backup block on the Components page, copied), and the routines, a
// list of #list-item kind toggle with "+ Add".
//
// Node names are the contract with the smarthome app: change both.

const path = require('path');
// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
let { key, authoring, C, FONT } = {};

const ROOT = '#smarthome-root';
const SCREEN_W = 1920, SCREEN_H = 1080;
// the content area of the dashboard: Home console, below the header, above the bottom bar
const CONTENT_X = 48, CONTENT_Y = 128, CONTENT_W = 1824, CONTENT_H = 788;
// Rooms: tiles 438 x 248, 4 a row, 24 apart
const TILE_W = 438, TILE_H = 248, TILE_GAP = 24, TILES = 8, TILES_Y = 168;
const SCENES_Y = TILES_Y + 2 * TILE_H + TILE_GAP + 28;
// Automations: the Backup's column (512) on the left, the routines (#list-item wide, 960) on
// the right; rows 72 + 8, the list as high as the column below its heading (744, #scrollbar)
const LEFT_W = 512, RIGHT_X = 592, LIST_W = 960, ROW_H = 72, ROW_GAP = 8, LIST_Y = 44, LIST_H = 744;

const ROOM_TABS = ['Living room', 'Entryway', 'Bedroom', 'Office'];
// the preview: the Bedroom, consistent with the dashboard and tools/matter-generator
const BEDROOM = [
  { kind: 'slider', symbol: 'bed-double', title: 'Ceiling light', subtitle: 'On · dimmable', value: '65 %' },
  { kind: 'toggle', symbol: 'bed-double', title: 'Bedside lamp', subtitle: 'On/off light', value: 'Off' },
  { kind: 'stepper', symbol: 'house', title: 'Bedroom climate', subtitle: 'Heating to 22°', value: '22°' },
];
const ROOM_SCENES = ['Evening', 'Reading', 'All off'];
const ROUTINES = [
  ['Sunset', 'Living room lamp on'],
  ['22:30', 'All lights off'],
  ['Leaving home', 'Thermostat to 18°'],
  ['Humidity over 60 %', 'Office fan High'],
  ['Good morning', 'Bedroom light 40 %, thermostat 21°'],
  ['Front door opened after sunset', 'Entryway light on for 5 min'],
];

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'screens');
  const { nodes, kids, log, frame, rect, at } = A;
  if (nodes.some(n => /^#(rooms|automations|home\/tabs)(\/|-screen|$)/.test(n.name || ''))) throw new Error('screens already present');
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');
  const lib = nodes.find(n => n.name === 'Library' && n.type === 'FRAME');
  if (!lib) throw new Error('Library missing: run components-patch.js first');
  const byName = name => {
    const n = nodes.find(x => x.name === name);
    if (!n) throw new Error(`${name} not found`);
    return n;
  };
  const child = (parent, name) => {
    const n = kids(key(parent.guid)).find(c => c.name === name);
    if (!n) throw new Error(`${parent.name} / ${name} not found`);
    return n;
  };
  const variantOf = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    if (!s) throw new Error(`${set} missing: run components-patch.js first`);
    const v = kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set}: no variant ${name}`);
    return v;
  };
  // the node named `layer` in a variant (breadth first)
  const findIn = (variant, layer) => {
    const stack = [...kids(key(variant.guid))];
    while (stack.length) {
      const n = stack.shift();
      if (n.name === layer) return n;
      stack.push(...kids(key(n.guid)));
    }
    throw new Error(`${variant.name}: no ${layer}`);
  };
  const textOverride = (variant, layer, s) => {
    const n = findIn(variant, layer);
    return { guidPath: { guids: [n.guid] }, textData: { characters: s, lines: n.textData.lines } };
  };
  // the opacity overrides that light tab `on` (1-based) of a #tabs variant with n tabs
  const nodeIn = (variant, layer) => ({ guids: [findIn(variant, layer).guid] });
  const lit = (variant, n, on) => Array.from({ length: n }, (_, i) => i + 1).flatMap(i => [
    { guidPath: nodeIn(variant, `#tabs/tab-${i}/selected`), opacity: i === on ? 0.72 : 0 },
    { guidPath: nodeIn(variant, `#tabs/tab-${i}/label`), opacity: i === on ? 0 : 1 },
    { guidPath: nodeIn(variant, `#tabs/tab-${i}/label-selected`), opacity: i === on ? 1 : 0 },
  ]);
  // a slot: a frame named for the app with a preview instance of the library component
  const slotted = (parent, name, x, y, variant, overrides = []) => {
    const slot = frame(parent, name, x, y, variant.size.x, variant.size.y);
    A.instance(slot, name + '/preview', 0, 0, variant, undefined, overrides);
    return slot;
  };
  const text = (parent, name, x, y, w, h, s, size, fnt, rgb, align = 'LEFT', valign = 'TOP', extra) => A.text(parent, name, x, y, w, h, s, size, fnt, rgb, align, valign, extra);
  // The deep copy of a design node (texts too: structuredClone keeps their typed arrays, which a
  // JSON copy turns into objects kiwi cannot encode), without its prototype (interaction ids
  // are unique).
  const copy = (src, parent, x, y) => {
    const dup = structuredClone(src);
    for (const f of ['guid', 'parentIndex', 'phase', 'transform', 'size', 'type', 'name', 'prototypeInteractions']) delete dup[f];
    const n = A.base(parent, src.type, src.name, x, y, src.size.x, src.size.y, dup);
    for (const c of kids(key(src.guid))) copy(c, n, c.transform.m02, c.transform.m12);
    return n;
  };

  // ---- 1. the tabs in the header: the drawn View selector becomes the slot #home/tabs
  const status = byName('Home status');
  const selector = child(status, 'View selector');
  const tabs3 = variantOf('#tabs', 'tabs=3, state=default');
  for (const c of kids(key(selector.guid))) {
    for (const g of kids(key(c.guid))) nodes.splice(nodes.indexOf(g), 1);
    nodes.splice(nodes.indexOf(c), 1);
  }
  Object.assign(selector, { name: '#home/tabs', fillPaints: [], strokePaints: [], cornerRadius: 0, frameMaskDisabled: true, stackPrimarySizing: 'FIXED', stackCounterSizing: 'FIXED' });
  for (const f of ['stackHorizontalPadding', 'stackVerticalPadding', 'stackPaddingRight', 'stackPaddingBottom', 'stackSpacing']) delete selector[f];
  selector.size = { x: tabs3.size.x, y: tabs3.size.y };
  A.instance(selector, '#home/tabs/preview', 0, 0, tabs3);
  log(`#home/tabs: the View selector as a slot of #tabs tabs=3 (${tabs3.size.x}x${tabs3.size.y})`);

  // ---- 2. the Overview's content frame, by name for the app
  const overview = byName('Home console');
  overview.name = '#overview';

  // ---- 3. the screens' sets in the Library, below what is there
  const tabs4 = variantOf('#tabs', 'tabs=4, state=default');
  const scene = variantOf('#button', 'kind=secondary, state=default, size=regular');
  const add = variantOf('#button', 'kind=ghost, state=default, size=compact');
  const routineRow = variantOf('#list-item', 'state=default, kind=toggle, lines=two, size=wide');
  const bar = variantOf('#scrollbar', `length=${LIST_H}`);
  const tileOf = kind => variantOf('#tile', `kind=${kind}, state=default`);

  let y = lib.size.y + 80;
  const roomsSet = A.componentSet(lib, '#rooms-screen', 80, y, [['state', ['hidden', 'shown']]], CONTENT_W, CONTENT_H, 0, 48, (s, v) => {
    const c = frame(s, '#rooms/content', 0, 0, CONTENT_W, CONTENT_H, { opacity: v.state === 'shown' ? 1 : 0 });
    slotted(c, '#rooms/tabs', 0, 0, tabs4, [...ROOM_TABS.map((t, i) => [
      textOverride(tabs4, `#tabs/tab-${i + 1}/label`, t), textOverride(tabs4, `#tabs/tab-${i + 1}/label-selected`, t)]).flat(), ...lit(tabs4, 4, 3)]);
    text(c, '#rooms/title', 0, 84, 900, 34, 'Bedroom', 28, FONT.semibold, C.ink);
    text(c, '#rooms/summary', 0, 124, 900, 20, '21° · 46 % · 2 of 3 on', 16, FONT.regular, C.ink2);
    // the tiles: slots 4 a row; the preview fills the Bedroom's three, the code fills and hides the rest
    for (let i = 0; i < TILES; i++) {
      const tx = (i % 4) * (TILE_W + TILE_GAP), ty = TILES_Y + Math.floor(i / 4) * (TILE_H + TILE_GAP);
      const d = BEDROOM[i];
      if (!d) { frame(c, `#rooms/tile-${i + 1}`, tx, ty, TILE_W, TILE_H); continue; }
      const t = tileOf(d.kind);
      // the device's symbol lit, the others at 0 (the tile stacks every #symbol glyph)
      const symbols = ['house', 'door-open', 'bed-double', 'armchair', 'droplets'].map(icon => ({ guidPath: { guids: [findIn(t, `#tile/symbol/${icon}`).guid] }, opacity: icon === d.symbol ? 1 : 0 }));
      slotted(c, `#rooms/tile-${i + 1}`, tx, ty, t, [
        textOverride(t, '#tile/title', d.title), textOverride(t, '#tile/subtitle', d.subtitle), textOverride(t, '#tile/value', d.value), ...symbols]);
    }
    text(c, 'Scenes label', 0, SCENES_Y - 28, 300, 20, 'Scenes', 16, FONT.regular, C.ink2);
    ROOM_SCENES.forEach((label, i) => slotted(c, `#rooms/scene-${i + 1}`, i * (scene.size.x + 16), SCENES_Y, scene, [textOverride(scene, '#button/label', label)]));
  }, { rows: true });
  y += roomsSet.set.size.y + 80;

  // the user's Scenes and Recent activity (Components page, Backup), copied into the screen
  const backup = byName('Scenes and home activity');
  const homeScenes = child(backup, 'Home scenes');
  const activity = child(backup, 'Recent home activity');
  const automationsSet = A.componentSet(lib, '#automations-screen', 80, y, [['state', ['hidden', 'shown']]], CONTENT_W, CONTENT_H, 0, 48, (s, v) => {
    const c = frame(s, '#automations/content', 0, 0, CONTENT_W, CONTENT_H, { opacity: v.state === 'shown' ? 1 : 0 });
    const sc = copy(homeScenes, c, 0, 0);
    sc.name = '#automations/scenes';
    const grid = kids(key(sc.guid)).find(n => n.name === 'Scene controls');
    kids(key(grid.guid)).filter(n => n.name === 'Scene').forEach((card, i) => {
      const base = `#automations/scene-${i + 1}`;
      card.name = base;
      const [act, id] = ['Scene action', 'Scene identity'].map(nm => kids(key(card.guid)).find(n => n.name === nm));
      const run = kids(key(act.guid)).find(n => n.type === 'TEXT');
      if (run) run.name = `${base}/run`;
      const [title, sub] = kids(key(id.guid)).filter(n => n.type === 'TEXT');
      title.name = `${base}/title`;
      sub.name = `${base}/subtitle`;
    });
    const ac = copy(activity, c, 0, homeScenes.size.y + 24);
    ac.name = '#automations/activity';
    const head = kids(key(ac.guid)).find(n => n.name === 'Section heading');
    const viewAll = kids(key(head.guid)).filter(n => n.type === 'TEXT')[1];
    if (viewAll) viewAll.name = '#automations/activity-all';
    const list = kids(key(ac.guid)).find(n => n.name === 'Activity list');
    kids(key(list.guid)).filter(n => n.name === 'Home event').forEach((ev, i) => {
      ev.name = `#automations/activity-${i + 1}`;
      const [event, time] = kids(key(ev.guid)).filter(n => n.type === 'TEXT');
      event.name = `#automations/activity-${i + 1}/event`;
      time.name = `#automations/activity-${i + 1}/time`;
    });
    // the routines: a heading with "+ Add", the list (#list-item kind toggle, lines two) and its scrollbar
    text(c, 'Routines heading', RIGHT_X, 0, 400, 28, 'Routines', 23, FONT.semibold, C.ink);
    slotted(c, '#automations/add', RIGHT_X + LIST_W - add.size.x, -14, add, [textOverride(add, '#button/label', '+ Add')]);
    const l = frame(c, '#automations/routines', RIGHT_X, LIST_Y, LIST_W, LIST_H, { frameMaskDisabled: false });
    ROUTINES.forEach(([when, what], i) => A.instance(l, `#automations/routines/preview-${i + 1}`, 0, i * (ROW_H + ROW_GAP), routineRow, undefined,
      [textOverride(routineRow, '#list-item/title', when), textOverride(routineRow, '#list-item/subtitle', what)]));
    slotted(c, '#automations/routines-scrollbar', RIGHT_X + LIST_W + 16, LIST_Y, bar);
  }, { rows: true });
  y += automationsSet.set.size.y + 80;
  lib.size = { x: lib.size.x, y };

  // ---- 4. the screens on the dashboard: hidden, over #overview, below the sheets (the sheets'
  // layers sort after them: "~...", see keyboard-patch.js and sheet-patch.js)
  [['Rooms layer', '#rooms', roomsSet, '$'], ['Automations layer', '#automations', automationsSet, '%']].forEach(([layerName, name, set, pos]) => {
    const layer = frame(root, layerName, 0, 0, SCREEN_W, SCREEN_H, { stackPositioning: 'ABSOLUTE' });
    layer.parentIndex.position = pos;
    A.instance(layer, name, CONTENT_X, CONTENT_Y, set.symbols['state=hidden']);
  });

  log(`added ${A.count()} nodes: #home/tabs, #overview, #rooms-screen and #automations-screen (hidden, shown) in the Library, #rooms and #automations over #overview`);
  return msg;
};

module.exports.CONTENT = { x: CONTENT_X, y: CONTENT_Y, w: CONTENT_W, h: CONTENT_H };
module.exports.ROOM_TABS = ROOM_TABS;
module.exports.BEDROOM = BEDROOM;
