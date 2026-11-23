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
// retro-theme.js: the theme of the look "Retro": the grey of a desktop of the nineties, black
// type, a navy accent with white labels, square corners.
module.exports = require('./looks-theme.js')({
  surface: {
    '#ffffff': '#c0c0c0',  // cards and sheets
    '#f4f7fb': '#c0c0c0', '#eaf0f7': '#c0c0c0', '#dfe7f3': '#b4b4b4',
    '#e6edf9': '#d4d0c8',  // soft accent: icon buttons, selected rows
    '#d3def2': '#a8a8a8', '#c9d6ea': '#a0a0a0', '#b9cbe5': '#808080',
    '#bdcdde': '#808080', '#bccddd': '#808080', '#a9bad9': '#8080b0',
    '#a3b1c2': '#808080',  // dividers, tracks
    '#6c7e93': '#404040', '#24364c': '#000000',
    '#4268b2': '#000080',  // the accent
    '#35558f': '#000060',
    '#c0392b': '#c00000',
  },
  mark: { '#ffffff': '#ffffff', '#24364c': '#000000', '#6c7e93': '#404040', '#a3b1c2': '#808080' },
  page: '#008080',
  radius: () => 0,
});
