// prototype-patch.js: the sheets for the designer, as DesignCompose does it in Figma. Run last
// (build.sh): it uses #sheet and #keyboard-dock (components-patch.js), #wifi-content
// (keyboard-patch.js), #device-content (sheet-patch.js), #panel-content (panel-patch.js),
// #prefs-content (prefs-patch.js), #event-content (event-patch.js), #game-content and
// #status/game (game-patch.js) and the dashboard's names (dashboard-patch.js).
//
// On the screen the sheets are instances in their closed variant, below the screen edge; the app
// opens them by their variant (harry does not read Figma prototypes). Here each sheet is also a
// visible frame on Page 1, below the dashboard (outside #smarthome-root: the fetch does not take
// it), and the dashboard carries a prototype: Play in Figma shows what the panel does.
//
//   the header's tabs -> Screen · Overview / Rooms / Automations (navigate; the Overview starts)
//   #status/wifi      -> Wi-Fi · networks (overlay); a tap on the list -> Wi-Fi · password + keyboard
//   a tile's header   -> Device sheet · <device> (overlay); in Rooms a device tile, the same
//   #settings         -> Settings · right (overlay from the right, the right half of the screen)
//   #agenda/list      -> Event · right (an event's details, from the right)
//   #status/game      -> Game · dialog (the game's picture and its keys)
//   Cancel, Connect, the close button, a tap outside a card -> close
//
// The buttons inside a card are instances; their taps are transparent hotspot frames on top.

const path = require('path');
// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
let { key, authoring, C, FONT } = {};
const { STATE_ROWS, DEVICE_ROWS, ROWS } = require(path.join(__dirname, 'sheet-patch.js'));
const { CONTENT, BEDROOM } = require(path.join(__dirname, 'screens-patch.js'));

const ROOT = '#smarthome-root';
const GAP = 120;
const MODEL = 'EveryWare Virtual Home';
const DEVICES = [
  { header: '#climate/header', name: 'Home climate', symbol: 'house', kind: 'Thermostat', state: '21.4 °C', detail: 'Heating to 22.0 °C · 45 % humidity', rows: STATE_ROWS, ep: 1 },
  { header: '#entryway/header', name: 'Entryway', symbol: 'door-open', kind: 'On/off light', state: 'On', detail: 'On since 18:42', ep: 2,
    rows: [['On/off', 'On'], ['After a power loss', 'As before'], ['On since', '18:42']] },
  { header: '#bedroom/header', name: 'Bedroom', symbol: 'bed-double', kind: 'Dimmable light', state: '65 %', detail: 'On · fades in 0.5 s', ep: 3,
    rows: [['Level', '65 %'], ['On/off', 'On'], ['Transition', '0.5 s'], ['Level range', '1 – 254'], ['Level at power on', 'As before']] },
  { header: '#office/header', name: 'Office fan', symbol: 'armchair', kind: 'Fan', state: 'Low', detail: '33 % · rocking off', ep: 4,
    rows: [['Mode', 'Low'], ['Speed', '33 %'], ['Modes', 'Off · Low · Med · High'], ['Airflow', 'Forward'], ['Rocking', 'Left – right'], ['Wind', 'Natural']] },
];

module.exports = function patch(msg, helpers) {
  ({ key, authoring, C, FONT } = helpers);
  const A = authoring(msg, 'prototype');
  const { nodes, kids, log, frame } = A;
  const page1 = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  const root = kids(key(page1.guid)).find(n => n.name === ROOT);
  if (!root) throw new Error(ROOT + ' not found on Page 1');

  const variantOf = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    if (!s) throw new Error(`${set} missing`);
    const v = kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set}: no variant ${name}`);
    return v;
  };
  // the first node named `name` below `top` (breadth first)
  const under = (top, name) => {
    const stack = [...kids(key(top.guid))];
    while (stack.length) {
      const n = stack.shift();
      if (n.name === name) return n;
      stack.push(...kids(key(n.guid)));
    }
    throw new Error(`${top.name}: no ${name}`);
  };

  // interactions need GUIDs of their own: a session after every node of this file
  const isess = Math.max(...nodes.map(n => n.guid.sessionID)) + 1;
  let ilocal = 0;
  const onTap = (node, action) => {
    node.prototypeInteractions = [...(node.prototypeInteractions || []), {
      id: { sessionID: isess, localID: ++ilocal }, event: { interactionType: 'ON_CLICK' }, actions: [action], stateManagementVersion: 1,
    }];
  };
  // a sheet opens from behind its edge (the dialog from the bottom), a screen dissolves
  const openOverlay = (target, from = 'MOVE_FROM_BOTTOM') => ({ transitionNodeID: target.guid, transitionType: from, transitionDuration: 0.3, easingType: 'OUT_CUBIC', connectionType: 'INTERNAL_NODE', navigationType: 'OVERLAY' });
  const swapOverlay = target => ({ transitionNodeID: target.guid, transitionType: 'DISSOLVE', transitionDuration: 0.2, easingType: 'OUT_CUBIC', connectionType: 'INTERNAL_NODE', navigationType: 'SWAP' });
  const close = () => ({ connectionType: 'CLOSE' });
  const DIM = { backgroundType: 'SOLID_COLOR', backgroundColor: { r: C.ink[0], g: C.ink[1], b: C.ink[2], a: 0.35 } };

  // the frames in a row below everything on Page 1
  // the screens in a row below everything on Page 1, the overlays in a row below the screens
  const belowY = Math.max(...kids(key(page1.guid)).map(n => n.transform.m12 + n.size.y)) + 200;
  const overlaysY = belowY + 1080 + 200;
  let nextX = root.transform.m02;
  const place = w => { const x = nextX; nextX += w + GAP; return x; };

  // A sheet as an overlay: a frame of the card's size that clips an instance of #sheet's open
  // variant of the layout (the variant is the whole screen; shifted by the card's position,
  // only the card shows), its content swapped in (fig-helpers sheetInstance), the overlay
  // placed where the layout puts the card and coming in from its edge. `overrides` are inside
  // the content (guidPath relative to the content component).
  const PAD = 40;
  const POSITION = { dialog: 'CENTER', left: 'TOP_LEFT', right: 'TOP_RIGHT', top: 'TOP_CENTER', bottom: 'BOTTOM_CENTER' };
  const FROM = { dialog: 'MOVE_FROM_BOTTOM', left: 'MOVE_FROM_LEFT', right: 'MOVE_FROM_RIGHT', top: 'MOVE_FROM_TOP', bottom: 'MOVE_FROM_BOTTOM' };
  const sheetOverlay = (title, layout, content, overrides = []) => {
    const variant = variantOf('#sheet', `layout=${layout}, state=open`);
    const card = under(variant, '#sheet/card');
    // a dialog's card hugs its content (Figma lays the instance out with the swapped content)
    const h = layout === 'dialog' ? content.size.y + 2 * PAD : card.size.y;
    const f = frame(page1, `${title} (overlay)`, place(card.size.x), overlaysY, card.size.x, h, {
      frameMaskDisabled: false, cornerRadius: 32,
      overlayPositionType: POSITION[layout], overlayBackgroundInteraction: 'CLOSE_ON_CLICK_OUTSIDE', overlayBackgroundAppearance: DIM,
    });
    A.sheetInstance(f, title, -card.transform.m02, -card.transform.m12, variant, content, overrides);
    return { overlay: f, card, content, from: FROM[layout] };
  };
  // a transparent tap area over the node `name` of a sheet's content (`dx`, `dy`: where the card
  // is in the frame; the content sits PAD inside the card)
  const hotspot = (overlay, content, name, label, dx = 0, dy = 0) => {
    const n = under(content, name);
    return frame(overlay, `hotspot: ${label}`, dx + PAD + n.transform.m02, dy + PAD + n.transform.m12, n.size.x, n.size.y);
  };

  // ---- Wi-Fi: the networks card, then the password card with the keyboard docked
  const wifiNetworks = variantOf('#wifi-content', 'step=networks');
  const wifiPassword = variantOf('#wifi-content', 'step=password');
  const dialogOpen = variantOf('#sheet', 'layout=dialog, state=open');
  const dockOpen = variantOf('#keyboard-dock', 'state=open');
  const nets = sheetOverlay('Wi-Fi · networks', 'dialog', wifiNetworks);
  // the password step is the whole screen: its sheet and the keyboard below, the scrim drawn in it
  const pw = frame(page1, 'Wi-Fi · password + keyboard (overlay)', place(1920), overlaysY, 1920, 1080, {
    frameMaskDisabled: false, overlayPositionType: 'CENTER', overlayBackgroundInteraction: 'NONE', overlayBackgroundAppearance: { backgroundType: 'NONE' },
  });
  A.sheetInstance(pw, 'Wi-Fi · password', 0, 0, dialogOpen, wifiPassword);
  A.instance(pw, 'Keyboard', 0, 0, dockOpen);
  const pwCard = under(dialogOpen, '#sheet/card');
  onTap(hotspot(nets.overlay, wifiNetworks, '#wifi/networks', 'a network'), swapOverlay(pw));
  onTap(hotspot(nets.overlay, wifiNetworks, '#wifi/cancel', 'Cancel'), close());
  onTap(hotspot(pw, wifiPassword, '#wifi/cancel', 'Cancel', pwCard.transform.m02, pwCard.transform.m12), close());
  onTap(hotspot(pw, wifiPassword, '#wifi/connect', 'Connect', pwCard.transform.m02, pwCard.transform.m12), close());
  const wifiButton = nodes.find(n => n.name === '#status/wifi');
  if (!wifiButton) throw new Error('#status/wifi not found: run dashboard-patch.js first');
  onTap(wifiButton, openOverlay(nets.overlay, nets.from));

  // ---- the device sheets, one per tile: the one content component with the device's data
  // as overrides inside it (texts by guid path, the rows' texts one level deeper)
  const deviceContent = variantOf('#device-content', 'state=default');
  const row = variantOf('#list-item', 'state=default, kind=text, lines=one, size=column');
  const rowTitle = under(row, '#list-item/title'), rowValue = under(row, '#list-item/trailing-text');
  const textAt = (node, s) => ({ guidPath: { guids: [node.guid] }, textData: { characters: s, lines: node.textData.lines } });
  const rowsOf = (listName, rows) => Array.from({ length: ROWS }, (_, i) => {
    const inst = under(deviceContent, `${listName}/preview-${i + 1}`);
    if (!rows[i]) return [{ guidPath: { guids: [inst.guid] }, visible: false }];
    return [
      { guidPath: { guids: [inst.guid, rowTitle.guid] }, textData: { characters: rows[i][0], lines: rowTitle.textData.lines } },
      { guidPath: { guids: [inst.guid, rowValue.guid] }, textData: { characters: rows[i][1], lines: rowValue.textData.lines } },
    ];
  }).flat();
  const deviceOverlay = {};
  for (const d of DEVICES) {
    const { overlay } = sheetOverlay(`Device sheet · ${d.name}`, 'dialog', deviceContent, [
      { guidPath: { guids: [under(deviceContent, '#device/symbol/preview').guid] }, overriddenSymbolID: variantOf('#symbol', `icon=${d.symbol}`).guid },
      textAt(under(deviceContent, '#device/title'), d.name),
      textAt(under(deviceContent, '#device/subtitle'), `${d.kind} · ${MODEL}`),
      textAt(under(deviceContent, '#device/state'), d.state),
      textAt(under(deviceContent, '#device/state-detail'), d.detail),
      ...rowsOf('#device/state-list', d.rows),
      ...rowsOf('#device/info-list', DEVICE_ROWS.map(([k, v]) => k === 'Node · endpoint' ? [k, `0x12 · ${d.ep}`] : [k, v])),
    ]);
    onTap(hotspot(overlay, deviceContent, '#device/close', 'close'), close());
    const header = nodes.find(n => n.name === d.header);
    if (!header) throw new Error(`${d.header} not found: run dashboard-patch.js first`);
    onTap(header, openOverlay(overlay));
    deviceOverlay[d.name] = overlay;
  }

  // ---- the panel: the library's #sheet at the right edge with the panel's content
  // (panel-patch.js); it comes in from the right
  const panelContent = variantOf('#panel-content', 'state=default');
  const panel = sheetOverlay('Panel · right', 'right', panelContent);
  const openPanel = target => openOverlay(target, panel.from);
  // the settings: the same right edge with the settings' content (prefs-patch.js)
  const prefsContent = variantOf('#prefs-content', 'state=default');
  const prefs = sheetOverlay('Settings · right', 'right', prefsContent);
  const settings = nodes.find(n => n.name === '#settings');
  if (!settings) throw new Error('#settings not found: run dashboard-patch.js first');
  onTap(settings, openPanel(prefs.overlay));
  onTap(hotspot(prefs.overlay, prefsContent, '#prefs/close', 'close'), close());
  // the panel's browser: close at the title bar (nothing on the dashboard opens it)
  onTap(hotspot(panel.overlay, panelContent, '#panel/close', 'close'), close());
  // an agenda event's details (event-patch.js), from a tap on the agenda's list; close at the top
  const eventContent = variantOf('#event-content', 'state=default');
  const eventPanel = sheetOverlay('Event · right', 'right', eventContent);
  const agendaList = nodes.find(n => n.name === '#agenda/list');
  if (!agendaList) throw new Error('#agenda/list not found: run dashboard-patch.js first');
  onTap(agendaList, openPanel(eventPanel.overlay));
  onTap(hotspot(eventPanel.overlay, eventContent, '#event/close', 'close'), close());
  // the game (game-patch.js), a dialog from the status row's game button; close at the title bar
  const gameContent = variantOf('#game-content', 'state=default');
  const game = sheetOverlay('Game · dialog', 'dialog', gameContent);
  const gameButton = nodes.find(n => n.name === '#status/game');
  if (!gameButton) throw new Error('#status/game not found: run game-patch.js first');
  onTap(gameButton, openOverlay(game.overlay, game.from));
  onTap(hotspot(game.overlay, gameContent, '#game/close', 'close'), close());

  // ---- the tabs as screens: the header and the bottom bar copied over the root's background,
  // the content the Overview's (copied) or a screen set's shown variant; the header's tabs
  // navigate between the three frames, the selected one lit
  const copy = (src, parent, x, y) => {
    const dup = structuredClone(src);
    for (const f of ['guid', 'parentIndex', 'phase', 'transform', 'size', 'type', 'name', 'prototypeInteractions']) delete dup[f];
    const n = A.base(parent, src.type, src.name, x, y, src.size.x, src.size.y, dup);
    for (const c of kids(key(src.guid))) copy(c, n, c.transform.m02, c.transform.m12);
    return n;
  };
  const within = (top, name) => { try { return under(top, name); } catch (e) { return null; } };
  const status = under(root, 'Home status'), bottom = under(root, 'Communication and access'), overview = under(root, '#overview');
  const tabsSlot = under(status, '#home/tabs');
  const tabs3 = variantOf('#tabs', 'tabs=3, state=default');
  const lit = on => [1, 2, 3].flatMap(i => [
    { guidPath: { guids: [under(tabs3, `#tabs/tab-${i}/selected`).guid] }, opacity: i === on ? 0.72 : 0 },
    { guidPath: { guids: [under(tabs3, `#tabs/tab-${i}/label`).guid] }, opacity: i === on ? 0 : 1 },
    { guidPath: { guids: [under(tabs3, `#tabs/tab-${i}/label-selected`).guid] }, opacity: i === on ? 1 : 0 },
  ]);
  const SCREENS = ['Overview', 'Rooms', 'Automations'];
  let sx = root.transform.m02;
  const screens = SCREENS.map((title, i) => {
    const f = frame(page1, `Screen · ${title}${title === 'Rooms' ? ' (Bedroom)' : ''}`, sx, belowY, 1920, 1080, { fillPaints: root.fillPaints, frameMaskDisabled: false });
    sx += 1920 + GAP;
    const head = copy(status, f, status.transform.m02, status.transform.m12);
    const preview = within(head, '#home/tabs/preview');
    preview.symbolData.symbolOverrides = [...preview.symbolData.symbolOverrides, ...lit(i + 1)];
    copy(bottom, f, bottom.transform.m02, bottom.transform.m12);
    if (title === 'Overview') copy(overview, f, CONTENT.x, CONTENT.y);
    else A.instance(f, `Screen · ${title} content`, CONTENT.x, CONTENT.y, variantOf(`#${title.toLowerCase()}-screen`, 'state=shown'));
    return { f, head };
  });
  // the tabs: one hotspot a tab in every screen's header
  const tabX = i => status.transform.m02 + tabsSlot.transform.m02 + 5 + i * (154 + 4);
  const tabY = status.transform.m12 + tabsSlot.transform.m12 + 5;
  const navigate = target => ({ transitionNodeID: target.guid, transitionType: 'DISSOLVE', transitionDuration: 0.2, easingType: 'OUT_CUBIC', connectionType: 'INTERNAL_NODE', navigationType: 'NAVIGATE' });
  screens.forEach(({ f }, from) => SCREENS.forEach((t, to) => {
    if (to !== from) onTap(frame(f, `hotspot: ${t}`, tabX(to), tabY, 154, 46), navigate(screens[to].f));
  }));
  // every screen's bottom bar: Settings opens the settings
  screens.forEach(({ f }) => onTap(within(f, '#settings'), openPanel(prefs.overlay)));
  // the Overview's copy keeps the dashboard's taps: Wi-Fi and the tiles' headers
  const ov = screens[0].f;
  onTap(within(screens[0].head, '#status/wifi'), openOverlay(nets.overlay, nets.from));
  for (const d of DEVICES) onTap(within(ov, d.header), openOverlay(deviceOverlay[d.name]));
  // The agenda while no calendar is linked: the column with its link card shown (on the screen
  // the app shows it, at opacity 0 in the root), right of the screens
  const agenda = under(root, 'Agenda');
  const linkFrame = frame(page1, 'Agenda · connect', sx, belowY, agenda.size.x + 96, agenda.size.y + 96, { fillPaints: root.fillPaints, frameMaskDisabled: false });
  const agendaCopy = copy(agenda, linkFrame, 48, 48);
  under(agendaCopy, '#agenda/link').opacity = 1;
  // and the Overview's agenda opens an event (Play starts there, not in the root)
  onTap(within(ov, '#agenda/list'), openPanel(eventPanel.overlay));
  // Rooms: the Bedroom's tiles open their device (the light the Bedroom sheet, the climate the
  // thermostat's); the Wi-Fi button too
  onTap(within(screens[1].head, '#status/wifi'), openOverlay(nets.overlay, nets.from));
  onTap(within(screens[2].head, '#status/wifi'), openOverlay(nets.overlay, nets.from));
  const rf = screens[1].f;
  [[0, 'Bedroom'], [2, 'Home climate']].forEach(([i, dev]) => {
    const tx = CONTENT.x + (i % 4) * (438 + 24), ty = CONTENT.y + 168 + Math.floor(i / 4) * (248 + 24);
    onTap(frame(rf, `hotspot: ${BEDROOM[i].title}`, tx, ty, 438, 248), openOverlay(deviceOverlay[dev]));
  });

  // the flow starts at the Overview frame
  delete root.prototypeStartingPoint;
  ov.prototypeStartingPoint = { name: 'Smart Home', description: 'Tap a tab, the Wi-Fi button, a tile header or an agenda event (Overview), or a device tile (Rooms)', position: '!' };
  log(`added ${A.count()} nodes: ${SCREENS.length} screen frames and ${3 + DEVICES.length} overlay frames on Page 1, ${ilocal} taps (tabs, Wi-Fi, tile headers, device tiles, hotspots)`);
  return msg;
};
