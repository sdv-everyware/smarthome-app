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
// terminal-theme.js: the theme of the look "Terminal": a black screen, phosphor green type
// and lines, square corners, every text in IBM Plex Mono (the one weight the panel has).
const mono = { family: 'IBM Plex Mono', style: 'Regular', postscript: 'IBMPlexMono-Regular' };
module.exports = require('./looks-theme.js')({
  surface: {
    '#ffffff': '#031207',  // cards and sheets
    '#f4f7fb': '#020d05', '#eaf0f7': '#000300', '#dfe7f3': '#06200c',
    '#e6edf9': '#073012',  // soft accent: icon buttons, selected rows
    '#d3def2': '#0a3d18', '#c9d6ea': '#0b4a1c', '#b9cbe5': '#0c5a21',
    '#bdcdde': '#0c5a21', '#bccddd': '#0c5a21', '#a9bad9': '#0a6b25',
    '#a3b1c2': '#0f6a28',  // dividers, tracks
    '#6c7e93': '#00b32c', '#24364c': '#00ff41',
    '#4268b2': '#00ff41',  // the accent
    '#35558f': '#00c236',
    '#c0392b': '#ff5a36',
  },
  // marks: a label on the accent is dark, as type on a lit field of a terminal
  mark: { '#ffffff': '#001a06', '#24364c': '#00ff41', '#6c7e93': '#00b32c', '#a3b1c2': '#008f25' },
  thumb: '#e2ffe9',
  page: '#000300',
  radius: () => 0,
  font: { regular: mono, strong: mono },
});
