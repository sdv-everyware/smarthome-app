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

// looks-theme.js: a look's theme. The whole file of a look is recoloured, the screen, the
// sheets and its copy of the Library alike (as ui-lib's dark-theme-patch.js does it), so the
// buttons, toggles, rows and sheets take the look too. A look gives its tables:
//
//   module.exports = require('./looks-theme.js')({ surface: {...}, mark: {...}, page: '#0a0a0a',
//       radius: r => 0, font: { regular: {...}, strong: {...} } });
//
// surface: light palette colour -> the look's colour, for frames, rectangles and instances.
// mark:    the same for texts and glyphs (defaults to surface). page: the root's fill.
// radius:  the corner radius of a node from its own (thumbs keep theirs). font: the faces of
// every text that the look's own patch did not set (regular for weights up to medium, strong
// above).
const MARK_TYPES = new Set(['TEXT', 'VECTOR', 'ELLIPSE', 'LINE', 'STAR', 'REGULAR_POLYGON', 'BOOLEAN_OPERATION']);
const hex = c => '#' + [c.r, c.g, c.b].map(v => Math.round(v * 255).toString(16).padStart(2, '0')).join('');
const rgb = (h, a) => ({ r: parseInt(h.slice(1, 3), 16) / 255, g: parseInt(h.slice(3, 5), 16) / 255, b: parseInt(h.slice(5, 7), 16) / 255, a });

function recolour(paints, table) {
  if (!paints) return paints;
  return paints.map(p => {
    if (p.type === 'SOLID' && p.color) {
      const to = table[hex(p.color)];
      return to ? { ...p, color: rgb(to, p.color.a ?? 1), colorVar: undefined } : p;
    }
    if (p.stops) {
      const stops = p.stops.map(s => { const to = table[hex(s.color)]; return to ? { ...s, color: rgb(to, s.color.a ?? 1) } : s; });
      return { ...p, stops, stopsVar: undefined };
    }
    return p;
  });
}

module.exports = look => function (msg) {
  const SURFACE = look.surface, MARK = { ...SURFACE, ...(look.mark || {}) };
  const thumb = look.thumb || '#ffffff';
  const tableOf = n => {
    const name = n.name || '';
    if (/\/thumb$/.test(name)) return { ...MARK, '#ffffff': thumb };
    if (/\/scrim$/.test(name)) return Object.fromEntries(Object.keys(SURFACE).map(k => [k, '#000000']));
    if (name === 'bg') return SURFACE;
    return MARK_TYPES.has(n.type) ? MARK : SURFACE;
  };
  const keep = new Set(look.keepFonts || []);
  for (const n of msg.nodeChanges) {
    const table = tableOf(n);
    n.fillPaints = recolour(n.fillPaints, table);
    n.strokePaints = recolour(n.strokePaints, table);
    if (n.effects) n.effects = n.effects.map(e => e.type === 'DROP_SHADOW' ? { ...e, color: { r: 0, g: 0, b: 0, a: look.shadow ?? 0 } } : e);
    for (const o of (n.symbolData && n.symbolData.symbolOverrides) || []) {
      if (o.fillPaints) o.fillPaints = recolour(o.fillPaints, MARK);
      if (o.strokePaints) o.strokePaints = recolour(o.strokePaints, MARK);
    }
    if (look.radius && n.type !== 'TEXT' && !/\/thumb$/.test(n.name || '')) {
      if (typeof n.cornerRadius === 'number' && n.cornerRadius > 0) n.cornerRadius = look.radius(n.cornerRadius, n);
      for (const k of ['rectangleTopLeftCornerRadius', 'rectangleTopRightCornerRadius', 'rectangleBottomLeftCornerRadius', 'rectangleBottomRightCornerRadius']) {
        if (typeof n[k] === 'number' && n[k] > 0) n[k] = look.radius(n[k], n);
      }
    }
    if (look.font && n.type === 'TEXT' && n.fontName && !keep.has(n.fontName.family)) {
      const strong = /Semi Bold|Bold|Black/.test(n.fontName.style || '');
      n.fontName = strong ? look.font.strong : look.font.regular;
      delete n.derivedTextData;
    }
  }
  const root = msg.nodeChanges.find(n => n.name === '#smarthome-root' && n.fillPaints && n.fillPaints.length);
  if (root && look.page) root.fillPaints = [{ type: 'SOLID', color: rgb(look.page, 1), opacity: 1, visible: true, blendMode: 'NORMAL' }];
};
