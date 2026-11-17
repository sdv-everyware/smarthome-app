// dashboard-patch.js: puts the library components (components-patch.js) into the widgets of
// the dashboard #smarthome-root, at the kiwi level. Run after components-patch.js.
//
// A drawn control becomes a slot: the frame keeps its place in the auto layout of its widget,
// gets a name for the app (#intercom/talk), loses its own drawing and holds a preview instance
// of the library component in the variant of the place's size (size=compact for Talk): on the
// device harry clones that variant into the frame, so the screen is the design.

const path = require('path');
// fig-helpers.js (the node authoring) comes from everyware's roundtrip.js, the patch's second argument
let { key, authoring } = {};

module.exports = function patch(msg, helpers) {
  ({ key, authoring } = helpers);
  const A = authoring(msg, 'dashboard');
  const { nodes, kids, log, instance, frame, at } = A;
const { C, FONT } = helpers;
  // the app's own pictures of the weather, a set next to the Library's (weather-pictures.js)
  require('./weather-pictures.js')(A, C, '#smarthome-weather');
  const root = nodes.find(n => n.name === '#smarthome-root');
  if (!root) throw new Error('#smarthome-root not found');

  const variantOf = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    if (!s) throw new Error(`${set} missing: run components-patch.js first`);
    const v = kids(key(s.guid)).find(n => n.name === name);
    if (!v) throw new Error(`${set}: no variant ${name}`);
    return v;
  };
  // the node `name` under the node `parentName` (names repeat across widgets)
  const find = (parentName, name) => {
    const parent = nodes.find(n => n.name === parentName);
    const node = parent && kids(key(parent.guid)).find(n => n.name === name);
    if (!node) throw new Error(`${parentName} / ${name} not found`);
    return node;
  };
  // removes a node and everything below it
  const drop = node => {
    for (const c of kids(key(node.guid))) drop(c);
    nodes.splice(nodes.indexOf(node), 1);
  };
  const labelOf = (variant, layer, s) => {
    const label = kids(key(variant.guid)).find(n => n.name === layer);
    return [{ guidPath: { guids: [label.guid] }, textData: { characters: s, lines: label.textData.lines } }];
  };
  // New text in a design text node: the characters, one plain line, and no derivedTextData
  // (the glyph layout cache of the old text; Figma shows it until it lays the text out again).
  const setText = (node, chars) => {
    node.textData = { characters: chars, lines: [{ lineType: 'PLAIN', styleId: 0, indentationLevel: 0, sourceDirectionality: 'AUTO', listStartOffset: 0, isFirstLineOfList: false }] };
    delete node.derivedTextData;
    return node;
  };
  const textOf = (src, name, chars) => { src.name = name; return setText(src, chars); };
  // turns the drawn control `node` into the slot `name` with a preview of `variant`
  const slotInto = (node, name, variant, overrides = []) => {
    for (const c of kids(key(node.guid))) drop(c);
    node.name = name;
    node.fillPaints = [];
    node.strokePaints = [];
    node.cornerRadius = 0;
    node.frameMaskDisabled = true;
    // An auto layout frame, fixed on both axes: the clone sits in the slot outside its flow
    // (harry puts a list item at an absolute position), a hugging slot would collapse around
    // it. A plain frame (an icon) becomes one: sizing fields on a frame without stackMode are
    // an invalid node, Figma drops it with its row on import.
    if (!node.stackMode || node.stackMode === 'NONE') {
      Object.assign(node, { bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackPrimaryAlignItems: 'CENTER', stackCounterAlignItems: 'CENTER', stackLayoutVersion: 1 });
    }
    node.stackPrimarySizing = 'FIXED';
    node.stackCounterSizing = 'FIXED';
    for (const f of ['stackHorizontalPadding', 'stackVerticalPadding', 'stackPaddingRight', 'stackPaddingBottom', 'stackSpacing']) delete node[f];
    // The slot takes the variant's size: a touch target larger than the design's icon (the
    // 72 px of #icon-button) grows the slot around its centre, so the visual stays put.
    if (variant.size.x > node.size.x || variant.size.y > node.size.y) {
      const dx = (variant.size.x - node.size.x) / 2, dy = (variant.size.y - node.size.y) / 2;
      node.transform = at(node.transform.m02 - dx, node.transform.m12 - dy);
      node.size = { x: variant.size.x, y: variant.size.y };
    }
    instance(node, `${name}/preview`, 0, 0, variant, { size: { x: node.size.x, y: node.size.y } }, overrides);
    log(`${name}: ${variant.name} at ${node.size.x}x${node.size.y}`);
  };

  const v = (set, name) => variantOf(set, name);
  const DROPDOWN_PILL_H = 48;   // #dropdown's field (components-patch.js DD_H)
  // 1. the status row: Presence becomes the dropdown; the Wi-Fi indicator joins it at the
  // right. Both in one frame, so the row (space between) keeps its three items.
  const row = nodes.find(n => n.name === 'Home status');
  row.frameMaskDisabled = true;   // the dropdown's list opens below the row
  const controls = frame(row, 'Status controls', 0, 0, 153 + 12 + 72, 72, {
    bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackSpacing: 12, stackCounterAlignItems: 'CENTER',
    stackPrimarySizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE', stackCounterSizing: 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE',
    stackLayoutVersion: 1, frameMaskDisabled: true,
  });
  // The row is drawn above the content rows: an absolute child of the root column, after
  // Home console in the tree (harry paints in child order, no z-index), before the bottom
  // bar and the sheet / keyboard layers; the root's top padding takes its place in the flow.
  // (The dropdown's open list reaches into the Scenes tiles below the row.)
  const console_ = nodes.find(n => n.name === 'Home console' && n.parentIndex && key(n.parentIndex.guid) === key(root.guid));
  row.stackPositioning = 'ABSOLUTE';
  row.transform = at(48, 40);
  row.parentIndex = { guid: root.guid, position: console_.parentIndex.position + '!' };
  root.stackVerticalPadding = 40 + row.size.y + 24;
  root.stackPaddingBottom = 40;
  const presence = find('Home status', 'Presence');
  presence.parentIndex = { guid: controls.guid, position: '!' };
  slotInto(presence, '#status/presence', v('#dropdown', 'state=default'));
  // The dropdown component is as tall as its open list (204); in the row only the closed pill
  // (48) takes space and the list hangs below it, over the content (frameMaskDisabled). A slot
  // of the full height would be centred in the row and push the pill above the screen.
  presence.size = { x: presence.size.x, y: DROPDOWN_PILL_H };
  presence.stackPrimaryAlignItems = 'MIN';
  presence.stackCounterAlignItems = 'MIN';
  const wifi = frame(controls, '#status/wifi', 0, 0, 72, 72, {
    bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackPrimarySizing: 'FIXED', stackPrimaryAlignItems: 'CENTER',
    stackCounterAlignItems: 'CENTER', stackLayoutVersion: 1,
  });
  instance(wifi, '#status/wifi/preview', 0, 0, v('#icon-button', 'kind=glass, state=default, icon=wifi, size=compact'));
  log('#status/wifi: new, 72x72 (48 visual) after Presence');

  // the clock: the app writes the local date and time into these two texts every second
  // Laid out by hand, not in auto layout: the date at the top, the time below it and inset,
  // the seconds smaller to its right on the time's baseline (design v30, edited in Figma).
  const clock = nodes.find(n => n.name === 'Clock and outdoor conditions');
  clock.stackMode = 'NONE';
  const date = find('Clock and outdoor conditions', 'Date');
  date.name = '#home/date';
  date.transform = at(0, 10);
  const time = find('Clock and outdoor conditions', 'Time');
  time.name = '#home/time';
  time.transform = at(46, 58);
  // Letter spacing 0: the panel cannot draw any (Impeller has no setter, open-points.md 5), so
  // the boxes take the font's own advance (IBM Plex Mono: 0.6 em a character) and Figma shows
  // what the panel shows.
  const MONO = 0.6, monoWidth = (chars, size) => Math.ceil(chars * size * MONO);
  time.letterSpacing = { value: 0, units: 'PIXELS' };
  time.size = { x: monoWidth(5, 80), y: 80 };
  const SEC_X = 46 + time.size.x - 2;
  A.text(clock, '#home/sec', SEC_X, 92, monoWidth(3, 36), 36, ':22', 36, time.fontName, [time.fillPaints[0].color.r, time.fillPaints[0].color.g, time.fillPaints[0].color.b], 'LEFT', 'TOP', {
    lineHeight: time.lineHeight, letterSpacing: { value: 0, units: 'PIXELS' }, textAutoResize: 'WIDTH_AND_HEIGHT', textUserLayoutVersion: 5,
  });
  log('#home/date, #home/time, #home/sec: the clock laid out by hand, its texts named for the app');

  // 2. climate: - and + around the setpoint, the humidity button
  slotInto(find('Temperature control', 'Decrease temperature'), '#climate/minus', v('#icon-button', 'kind=tonal, state=default, icon=minus, size=regular'));
  slotInto(find('Temperature control', 'Increase temperature'), '#climate/plus', v('#icon-button', 'kind=tonal, state=default, icon=plus, size=regular'));
  slotInto(find('Humidity control', 'Adjust humidity'), '#climate/humidity-adjust', v('#icon-button', 'kind=tonal, state=default, icon=sliders-horizontal, size=compact'));
  // texts the app sets
  const rename = (parent, name, to) => { find(parent, name).name = to; log(`${parent} / ${name} -> ${to}`); };
  rename('Target temperature', 'Setpoint', '#climate/setpoint');
  rename('Target temperature', 'Label', '#climate/target');

  // 3. camera, rooms, door, settings
  slotInto(find('Camera caption', 'Open feed'), '#camera/open', v('#icon-button', 'kind=scrim, state=default, icon=maximize-2, size=regular'));
  // The camera picture becomes a zone: on the device another process (a harry client, later
  // the browser) draws into it over Wayland (smarthome-app --wayland-zones). harry shows an
  // external image only on a plain frame (Shape::Rect); a rectangle layer imports as a vector
  // (VectorRect) and is skipped. So: a frame of the same size made exactly like the working
  // zone of the DOOM HUD (#doom in doom-hud.fig): no children, a white solid fill (where no
  // client frame is, the zone shows white, never a picture that looks like one), its own clip,
  // the same two drop shadows. The tile (Living room camera, radius 32, clips) rounds it.
  {
    const feed = find('Living room camera', 'Camera feed');
    feed.type = 'FRAME';
    feed.name = '#camera/feed';
    delete feed.fillGeometry;
    feed.fillPaints = [{ type: 'SOLID', color: { r: 1, g: 1, b: 1, a: 1 }, opacity: 1, visible: true, blendMode: 'NORMAL',
      authoredColor: { space: 'DEFAULT', c0: 1, c1: 1, c2: 1, c3: 1 } }];
    feed.effects = [
      { type: 'DROP_SHADOW', offset: { x: 0, y: 4 }, radius: 4, visible: true, blendMode: 'NORMAL', spread: 0, showShadowBehindNode: false,
        color: { r: 0, g: 0, b: 0, a: 0.25 }, authoredColor: { space: 'DEFAULT', c0: 0, c1: 0, c2: 0, c3: 0.25 } },
      { type: 'DROP_SHADOW', offset: { x: 25, y: 0 }, radius: 184.10000610351562, visible: true, blendMode: 'NORMAL', spread: 58, showShadowBehindNode: false,
        color: { r: 0, g: 0, b: 0, a: 0.20000000298023224 }, authoredColor: { space: 'DEFAULT', c0: 0, c1: 0, c2: 0, c3: 0.20000000298023224 } },
    ];
    feed.cornerRadius = 0;
    feed.frameMaskDisabled = false;
    log('Living room camera / Camera feed -> #camera/feed (FRAME, the zone)');
  }
  slotInto(find('Light control', 'Light switch'), '#entryway/light', v('#toggle', 'state=default'));
  rename('Light identity', 'State', '#entryway/state');
  slotInto(find('Bedroom lights', 'Dimmer control'), '#bedroom/dimmer', v('#slider', 'state=default'));
  slotInto(find('Office fan', 'Fan speed selector'), '#office/fan', v('#segmented', 'segments=3, state=default'));
  rename('Fan identity', 'State', '#office/state');
  rename('Dimmer label', 'Level', '#bedroom/level');
  slotInto(find('Front door', 'Door details'), '#door/details', v('#icon-button', 'kind=tonal, state=default, icon=chevron-right, size=regular'));
  slotInto(find('Communication and access', 'Settings control'), '#settings', v('#icon-button', 'kind=tonal, state=default, icon=settings-2, size=regular'));

  // 4. weather in place of the energy widget: the same card (heading, a readings row, a
  // chart with its times), the values from Open-Meteo (the app, bundles/weather).
  const energy = nodes.find(n => n.name === 'Energy usage');
  energy.name = '#weather';
  const heading = find('#weather', 'Section heading');
  const [hLeft, hRight] = kids(key(heading.guid));
  setText(hLeft, 'Weather');
  textOf(hRight, '#weather/place', 'Berlin');
  const readings = find('#weather', 'Energy readings');
  const [now, meta] = kids(key(readings.guid));
  const [power, powerLabel] = kids(key(now.guid));
  const [daily, dailyLabel] = kids(key(meta.guid));
  // left: the icon and the temperature side by side; the old "Using now" label becomes the condition
  now.name = 'Now'; now.stackMode = 'HORIZONTAL'; now.stackSpacing = 8; now.stackCounterAlignItems = 'CENTER';
  delete now.stackPrimarySizing; now.stackCounterSizing = 'RESIZE_TO_FIT_WITH_IMPLICIT_SIZE';
  drop(powerLabel);
  const iconSlot = frame(now, '#weather/icon', 0, 0, 40, 40, { bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackPrimarySizing: 'FIXED', stackCounterSizing: 'FIXED', stackLayoutVersion: 1, frameMaskDisabled: true });
  // before the temperature: fractional positions sort as strings
  iconSlot.parentIndex.position = power.parentIndex.position;
  power.parentIndex.position = power.parentIndex.position + '~';
  instance(iconSlot, '#weather/icon/preview', 0, 0, v('#smarthome-weather', 'kind=partly'));
  textOf(power, '#weather/temp', '18°');
  // right: the condition and the details, as the daily reading was
  meta.name = 'Conditions';
  textOf(daily, '#weather/condition', 'Partly cloudy');
  textOf(dailyLabel, '#weather/details', 'Feels 16° · 62 % · 14 km/h');
  dailyLabel.fillPaints = [A.solid(C.ink2)];
  // the chart: the slot of the library's #bar-chart where the bars were; the times stay, named
  const chart = find('#weather', 'Usage chart');
  const [hourly, times] = kids(key(chart.guid));
  slotInto(hourly, '#weather/chart', v('#bar-chart', 'bars=24'));
  kids(key(times.guid)).forEach((t, i) => textOf(t, `#weather/t${i + 1}`, ['13:00', '01:00', '12:00'][i]));
  // the line under the clock showed the weather a second time: gone
  drop(find('Clock and outdoor conditions', 'Outdoor reading'));
  log('#weather: in place of Energy usage (icon slot, temp, condition, details, chart slot, 3 times); the line under the clock dropped');

  // 5. intercom: Talk, the compact primary button (116, the width of Action)
  const primary = v('#button', 'kind=primary, state=default, size=compact');
  slotInto(find('Intercom', 'Action'), '#intercom/talk', primary, labelOf(primary, '#button/label', 'Talk'));

  // the right column: the agenda, a scrolling list of #list-item (size column: a subheader per
  // day, kind subheader, and the day's events, kind leading-text: the start over the duration and
  // a bar in the event's colour) with its scrollbar, in the place of Scenes and home activity;
  // that column is kept whole on the Components page (frame Backup, outside the screen: the
  // fetch takes the root and the Library only). Over the list the card that links the calendar
  // (#agenda/link: a QR code the app makes, a title, a hint): at opacity 0 here, the app shows it
  // while no calendar is linked; the designer's frame "Agenda · connect" (prototype-patch.js)
  // shows it open.
  const scenes = nodes.find(n => n.name === 'Scenes and home activity' && n.parentIndex && key(n.parentIndex.guid) === key(console_.guid));
  if (!scenes) throw new Error('Home console / Scenes and home activity not found');
  const compPage = nodes.find(n => n.type === 'CANVAS' && n.name === 'Components');
  const AG_W = 512, AG_H = scenes.size.y, HEAD_H = 28, HEAD_GAP = 16, SB_W = 6, SB_GAP = 16;
  const LIST_W = AG_W - SB_GAP - SB_W, LIST_H = AG_H - HEAD_H - HEAD_GAP;
  // the rows' pitches (the app's, agenda.rs): a subheader 48, an event 72, 8 between
  const ROW_PITCH = 72 + 8, SUB_PITCH = 48 + 8;
  const agenda = frame(console_, 'Agenda', 0, 0, AG_W, AG_H, {
    bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: HEAD_GAP, stackPrimarySizing: 'FIXED', stackLayoutVersion: 1,
  });
  agenda.parentIndex = { guid: console_.guid, position: scenes.parentIndex.position };
  const backup = frame(compPage, 'Backup', 3400, 0, AG_W + 48, AG_H + 96, { fillPaints: [A.solid(C.page)], frameMaskDisabled: false });
  A.text(backup, 'Heading', 24, 20, AG_W, 28, 'Backup: Scenes and home activity (not on the screen)', 17, FONT.semibold, C.ink);
  scenes.parentIndex = { guid: backup.guid, position: '!' };
  scenes.transform = at(24, 72);
  // the heading, at its right how old the agenda is when it is not fresh (#agenda/status: the app
  // writes it, empty while the agenda is fresh; agenda.rs status), and at the right end the
  // calendar's link (#agenda/account, a tonal icon button the app shows while a calendar is
  // linked; it opens the event's panel on the link). The button's touch target (72) is centred on
  // the header's 28, its circle at the header's right edge; it reaches into the gap above the list.
  const header = frame(agenda, 'Agenda header', 0, 0, AG_W, HEAD_H);
  const accountButton = v('#icon-button', 'kind=tonal, state=default, icon=settings-2, size=compact');
  const ACC_X = AG_W - 24 - accountButton.size.x / 2;
  A.text(header, 'Agenda heading', 0, 0, 160, HEAD_H, 'Agenda', 23, FONT.semibold, C.ink);
  A.text(header, '#agenda/status', 160, 0, AG_W - 160 - 60, HEAD_H, 'Last read 14:02 · no link to the house\'s data', 15, FONT.regular, C.ink2, 'RIGHT', 'CENTER', { maxLines: 1, textTruncation: 'ENDING' });
  const account = frame(header, '#agenda/account', ACC_X, (HEAD_H - accountButton.size.y) / 2, accountButton.size.x, accountButton.size.y);
  instance(account, '#agenda/account/preview', 0, 0, accountButton);
  const body = frame(agenda, 'Agenda content', 0, HEAD_H + HEAD_GAP, AG_W, LIST_H);
  const list = frame(body, '#agenda/list', 0, 0, LIST_W, LIST_H, { frameMaskDisabled: false });
  // the preview: days as the app shows them (texts and the bars' colours as overrides)
  const subRow = variantOf('#list-item', 'state=default, kind=subheader, lines=one, size=column');
  const eventRow = variantOf('#list-item', 'state=default, kind=leading-text, lines=two, size=column');
  const partOf = (variant, name) => {
    const stack = [...kids(key(variant.guid))];
    while (stack.length) { const n = stack.shift(); if (n.name === name) return n; stack.push(...kids(key(n.guid))); }
    throw new Error(`${variant.name}: no ${name}`);
  };
  const said = (variant, texts) => Object.entries(texts).map(([part, chars]) => {
    const t = partOf(variant, part);
    return { guidPath: { guids: [t.guid] }, textData: { characters: chars, lines: t.textData.lines } };
  });
  const hex = h => [parseInt(h.slice(0, 2), 16) / 255, parseInt(h.slice(2, 4), 16) / 255, parseInt(h.slice(4, 6), 16) / 255];
  const PREVIEW = [
    ['TODAY', 'NOV 10'],
    ['8:30 AM', '30 min', 'Team stand-up', 'Video call', '039be5'],
    ['12:00 PM', '1h', 'Lunch with Anna', 'Café Central', '33b679'],
    ['6:30 PM', '1h 30', 'Yoga', 'Studio 5', 'e67c73'],
    ['TOMORROW', 'NOV 11'],
    ['All day', '', 'Birthday of Mum', '', '8e24aa'],
    ['9:00 AM', '2h', 'Dentist', 'Main Street 12', 'f4511e'],
    ['WEDNESDAY', 'NOV 12'],
    ['7:00 PM', '2h', 'Cinema', 'Odeon', '0b8043'],
  ];
  let y = 0;
  PREVIEW.forEach((r, i) => {
    if (y >= LIST_H) return;
    if (r.length === 2) {
      instance(list, `#agenda/list/preview-${i + 1}`, 0, y, subRow, {}, said(subRow, { '#list-item/title': r[0], '#list-item/trailing-text': r[1] }));
      y += SUB_PITCH;
    } else {
      const bar = { guidPath: { guids: [partOf(eventRow, '#list-item/accent').guid] }, fillPaints: [A.solid(hex(r[4]))] };
      instance(list, `#agenda/list/preview-${i + 1}`, 0, y, eventRow, {}, [...said(eventRow, {
        '#list-item/leading-title': r[0], '#list-item/leading-subtitle': r[1] || ' ', '#list-item/title': r[2], '#list-item/subtitle': r[3] || ' ',
      }), bar]);
      y += ROW_PITCH;
    }
  });
  A.scrollbarSlot(body, '#agenda/scrollbar', LIST_W + SB_GAP, 0, LIST_H);
  // The link card, over the list: white as the rows, its content in a centred column. The QR
  // code is a plain frame without children: harry draws a picture on a plain rectangle only,
  // and the app's picture (black on white, the quiet zone in it) fills it; the grey is the
  // placeholder until it comes. With no QR code the app takes the frame out of the column
  // (display gone) and the texts move to the middle.
  const QR = 264, TITLE_H = 28, HINT_H = 22, LINK_GAP = 16, TEXT_W = LIST_W - 64;
  const linkY = Math.round((LIST_H - (QR + LINK_GAP + TITLE_H + 8 + HINT_H)) / 2);
  const link = frame(body, '#agenda/link', 0, 0, LIST_W, LIST_H, {
    opacity: 0, fillPaints: [A.solid(C.white)], cornerRadius: 16, frameMaskDisabled: false,
    bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: 0, stackPrimaryAlignItems: 'CENTER', stackCounterAlignItems: 'CENTER',
    stackPrimarySizing: 'FIXED', stackCounterSizing: 'FIXED', stackLayoutVersion: 1,
  });
  frame(link, '#agenda/qr', (LIST_W - QR) / 2, linkY, QR, QR, { fillPaints: [A.solid(C.line)] });
  // (the gaps as the texts' own top: the frame that goes leaves no gap behind)
  A.text(link, '#agenda/link-title', 32, linkY + QR, TEXT_W, LINK_GAP + TITLE_H, 'Scan to connect Google Calendar', 20, FONT.semibold, C.ink, 'CENTER', 'BOTTOM', { maxLines: 1, textTruncation: 'ENDING' });
  A.text(link, '#agenda/link-hint', 32, linkY + QR + LINK_GAP + TITLE_H, TEXT_W, 8 + HINT_H, 'Your phone must be on the same Wi-Fi', 15, FONT.regular, C.ink2, 'CENTER', 'BOTTOM', { maxLines: 1, textTruncation: 'ENDING' });
  log(`#agenda: a list ${LIST_W}x${LIST_H} (subheaders ${SUB_PITCH}, events ${ROW_PITCH} apart), its scrollbar and the link card (#agenda/qr ${QR}, opacity 0) in place of Scenes and home activity, which is on the Components page (Backup)`);

  // 6. the device sheet's targets (sheet-patch.js): a tap on a tile's header opens its device.
  // The whole header is the target (the chevron alone is 20 x 20, too small to hit); nothing
  // changes in the drawing. The climate's header has no chevron.
  find('Climate control', 'Section heading').name = '#climate/header';
  for (const roomControl of nodes.filter(n => n.name === 'Room control' && n.type === 'FRAME')) {
    const header = kids(key(roomControl.guid)).find(n => n.name === 'Room header');
    const identity = kids(key(roomControl.guid)).find(n => n.name === 'Room identity');
    const room = identity && kids(key(identity.guid)).find(n => n.name === 'Room');
    if (!header || !room) throw new Error('Room control: no Room header / Room identity');
    header.name = `#${room.textData.characters.toLowerCase()}/header`;
    log(`${room.textData.characters}: Room header -> ${header.name}`);
  }

  log(`added ${A.count()} nodes`);
  return msg;
};
