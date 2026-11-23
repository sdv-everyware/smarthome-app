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

// looks-common.js: what every look of the panel shares. A look is another screen for the same
// app: a new #smarthome-root on the Library's Page 1 that carries the panel's node names
// (smarthome-app src/ui_model.rs), so the app drives it without a change. The sheets (Wi-Fi,
// device, settings, event) come from the panel's own patches after the look's.
//
//   const L = require('./looks-common.js')(msg, helpers, 'poster', { page: [r, g, b] });
//   L.text(L.root, '#home/time', ...); L.slot(L.root, '#entryway/light', x, y, L.variant('#toggle', 'state=default'));
module.exports = function (msg, { key, C, FONT, authoring }, tag, opts) {
  const A = authoring(msg, tag);
  const { nodes, kids, frame, text, instance, solid } = A;
  // the panel's own pictures of the weather (design/weather-pictures.js), the set the app picks from
  require('../weather-pictures.js')(A, C, '#smarthome-weather');

  const page = nodes.find(n => n.type === 'CANVAS' && n.name === 'Page 1');
  if (!page) throw new Error('no Page 1: build on the EveryWare Library .fig');
  if (nodes.some(n => n.name === '#smarthome-root')) throw new Error('#smarthome-root exists already');
  const root = frame(page, '#smarthome-root', -2400, 0, 1920, 1080, { fillPaints: [solid(opts.page)], frameMaskDisabled: false });

  const variant = (set, name) => {
    const s = nodes.find(n => n.isStateGroup && n.name === set);
    const v = s && kids(key(s.guid)).find(n => n.type === 'SYMBOL' && n.name === name);
    if (!v) throw new Error(`no variant ${set} / ${name}`);
    return v;
  };
  const deep = (n, name) => { for (const c of kids(key(n.guid))) { if (c.name === name) return c; const d = deep(c, name); if (d) return d; } return null; };
  // a slot: a frame named for the app, of the variant's size, with a preview instance inside
  const slot = (parent, name, x, y, v, overrides = [], extra) => {
    const f = frame(parent, name, x, y, v.size.x, v.size.y, Object.assign({
      bordersTakeSpace: false, stackMode: 'HORIZONTAL', stackPrimaryAlignItems: 'CENTER', stackCounterAlignItems: 'CENTER',
      stackPrimarySizing: 'FIXED', stackCounterSizing: 'FIXED', stackLayoutVersion: 1,
    }, extra || {}));
    instance(f, `${name}/preview`, 0, 0, v, null, overrides);
    return f;
  };
  const label = (v, part, chars) => {
    const t = deep(v, part); if (!t) throw new Error(`no ${part} in ${v.name}`);
    return [{ guidPath: { guids: [t.guid] }, textData: { characters: chars, lines: t.textData.lines } }];
  };
  const inter = style => ({ family: 'Inter', style, postscript: 'Inter-' + style.replace(' ', '') });
  const F = { regular: FONT.regular, medium: FONT.medium, semibold: FONT.semibold, bold: inter('Bold'), black: inter('Black'),
    mono: { family: 'IBM Plex Mono', style: 'Regular', postscript: 'IBMPlexMono-Regular' } };
  const one = { maxLines: 1, textTruncation: 'ENDING' };

  // the controls every look places somewhere
  const V = {
    toggle: () => variant('#toggle', 'state=default'),
    slider: () => variant('#slider', 'state=default'),
    segmented: () => variant('#segmented', 'segments=3, state=default'),
    wifi: () => variant('#icon-button', 'kind=glass, state=default, icon=wifi, size=compact'),
    settings: () => variant('#icon-button', 'kind=tonal, state=default, icon=settings-2, size=regular'),
    talk: () => variant('#button', 'kind=primary, state=default, size=compact'),
    presence: () => variant('#dropdown', 'state=default'),
    chart: () => variant('#bar-chart', 'bars=24'),
    weather: () => variant('#smarthome-weather', 'kind=partly'),
  };
  const talk = (parent, x, y) => { const v = V.talk(); return slot(parent, '#intercom/talk', x, y, v, label(v, '#button/label', 'Talk')); };
  // the presence dropdown: only its closed pill (48) takes space, the list hangs below it
  const presence = (parent, x, y) => {
    const v = V.presence();
    const f = slot(parent, '#status/presence', x, y, v, [], { stackPrimaryAlignItems: 'MIN', stackCounterAlignItems: 'MIN' });
    f.size = { x: v.size.x, y: 48 };
    return f;
  };
  const weatherIcon = (parent, x, y) => slot(parent, '#weather/icon', x, y, V.weather(), [], { frameMaskDisabled: true });

  // the agenda: the list (490 wide, the rows of size column), its scrollbar, the link card over
  // it, the status line and the account button; as the dashboard has it (dashboard-patch.js)
  const agenda = (parent, x, y, h, ink, ink2, cardRgb) => {
    const LIST_W = 490, SB_GAP = 16, HEAD_H = 28, HEAD_GAP = 16, LIST_H = h - HEAD_H - HEAD_GAP;
    const box = frame(parent, 'Agenda', x, y, LIST_W + SB_GAP + 6, h);
    const acc = variant('#icon-button', 'kind=tonal, state=default, icon=settings-2, size=compact');
    text(box, '#agenda/status', 0, 0, LIST_W - 60, HEAD_H, ' ', 15, F.regular, ink2, 'LEFT', 'CENTER', one);
    slot(box, '#agenda/account', LIST_W + SB_GAP + 6 - 24 - acc.size.x / 2, (HEAD_H - acc.size.y) / 2, acc);
    const body = frame(box, 'Agenda content', 0, HEAD_H + HEAD_GAP, LIST_W + SB_GAP + 6, LIST_H);
    const list = frame(body, '#agenda/list', 0, 0, LIST_W, LIST_H, { frameMaskDisabled: false });
    const sub = variant('#list-item', 'state=default, kind=subheader, lines=one, size=column');
    instance(list, '#agenda/list/preview-1', 0, 0, sub, {}, [...label(sub, '#list-item/title', 'TODAY'), ...label(sub, '#list-item/trailing-text', ' ')]);
    A.scrollbarSlot(body, '#agenda/scrollbar', LIST_W + SB_GAP, 0, LIST_H);
    const QR = Math.min(264, LIST_H - 110), TEXT_W = LIST_W - 64;
    const linkY = Math.round((LIST_H - (QR + 16 + 28 + 8 + 22)) / 2);
    const link = frame(body, '#agenda/link', 0, 0, LIST_W, LIST_H, {
      opacity: 0, fillPaints: [solid(cardRgb)], cornerRadius: 16, frameMaskDisabled: false,
      bordersTakeSpace: false, stackMode: 'VERTICAL', stackSpacing: 0, stackPrimaryAlignItems: 'CENTER', stackCounterAlignItems: 'CENTER',
      stackPrimarySizing: 'FIXED', stackCounterSizing: 'FIXED', stackLayoutVersion: 1,
    });
    frame(link, '#agenda/qr', (LIST_W - QR) / 2, linkY, QR, QR, { fillPaints: [solid(C.line)] });
    text(link, '#agenda/link-title', 32, linkY + QR, TEXT_W, 16 + 28, 'Scan to connect Google Calendar', 20, F.semibold, ink, 'CENTER', 'BOTTOM', one);
    text(link, '#agenda/link-hint', 32, linkY + QR + 16 + 28, TEXT_W, 8 + 22, 'Your phone must be on the same Wi-Fi', 15, F.regular, ink2, 'CENTER', 'BOTTOM', one);
    return box;
  };

  // the camera's full view over the screen, drawn closed (below it), as the dashboard has it
  // (camera-patch.js): the picture is the zone of the live video, over it a band with the
  // LIVE chip, the room and the close button. A tap on a node named #camera/tap opens it.
  // Its type is a white of its own, which no theme maps.
  const cameraView = () => {
    const DARK = [0x1d / 255, 0x27 / 255, 0x33 / 255], WHITE = [0.988, 0.988, 0.988];
    const layer = frame(root, 'Camera layer', 0, 0, 1920, 1080, { stackPositioning: 'ABSOLUTE' });
    const view = frame(layer, '#camera-view', 0, 1080 + 40, 1920, 1080, { fillPaints: [solid(DARK)], frameMaskDisabled: false });
    frame(view, '#camera-view/picture', 0, 0, 1920, 1080, { fillPaints: [solid(DARK)], frameMaskDisabled: false });
    const off = frame(view, '#camera-view/offline', 0, 480, 1920, 128);
    text(off, 'Title', 0, 0, 1920, 52, 'Camera offline', 40, F.semibold, WHITE, 'CENTER');
    text(off, 'Detail', 0, 64, 1920, 36, 'The camera app is not running', 27, F.regular, WHITE, 'CENTER', 'TOP', { opacity: 0.72 });
    A.rect(view, 'Band', 0, 0, 1920, 112, [0, 0, 0], 0, { opacity: 0.35 });
    const live = frame(view, '#camera-view/live', 40, 40, 72, 32, { cornerRadius: 16, frameMaskDisabled: false });
    A.rect(live, 'bg', 0, 0, 72, 32, [0.1255, 0.1608, 0.1333], 16, { opacity: 0.65 });
    A.ellipse(live, 'Live lamp', 14, 13, 6, 6, [0.9216, 0.8314, 0.6471]);
    text(live, 'State', 28, 8, 40, 16, 'LIVE', 13, F.semibold, WHITE);
    text(view, 'Room', 128, 36, 600, 40, 'Living room', 28, F.semibold, WHITE, 'LEFT', 'CENTER');
    const close = variant('#icon-button', 'kind=scrim, state=default, icon=x, size=regular');
    slot(view, '#camera-view/close', 1920 - 40 - close.size.x, (112 - close.size.y) / 2, close);
  };

  return { A, C, F, root, page, variant, slot, label, deep, text, frame, instance, solid, one, V, talk, presence, weatherIcon, agenda, cameraView, log: A.log, rect: A.rect };
};
