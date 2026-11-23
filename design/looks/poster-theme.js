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

// poster-theme.js: the theme of the look "Poster": black, off-white type, one orange accent,
// square corners, every text in Big Shoulders Display.
const shoulders = style => ({ family: 'Big Shoulders Display', style, postscript: 'BigShouldersDisplay-' + style });
module.exports = require('./looks-theme.js')({
  surface: {
    '#ffffff': '#141414',  // cards and sheets
    '#f4f7fb': '#0e0e0e', '#eaf0f7': '#0a0a0a', '#dfe7f3': '#1c1c1c',
    '#e6edf9': '#242424',  // soft accent: icon buttons, selected rows
    '#d3def2': '#2e2e2e', '#c9d6ea': '#333333', '#b9cbe5': '#3a3a3a',
    '#bdcdde': '#3a3a3a', '#bccddd': '#3a3a3a', '#a9bad9': '#6b3526',
    '#a3b1c2': '#4a4a4a',  // dividers, tracks
    '#6c7e93': '#a8a8a4', '#24364c': '#f0f0ee',
    '#4268b2': '#ff5a36',  // the accent
    '#35558f': '#d9441f',
    '#c0392b': '#ff5a36',
  },
  mark: { '#ffffff': '#ffffff', '#24364c': '#f0f0ee', '#6c7e93': '#a8a8a4', '#a3b1c2': '#7a7a76' },
  page: '#0a0a0a',
  radius: () => 0,
  font: { regular: shoulders('Medium'), strong: shoulders('Black') },
  keepFonts: ['IBM Plex Mono'],
});
