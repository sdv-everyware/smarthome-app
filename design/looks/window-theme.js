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
// window-theme.js: the theme of the look "Window": the sheets and the controls as dark glass
// over the camera's picture, light type, one amber accent with dark labels on it.
module.exports = require('./looks-theme.js')({
  surface: {
    '#ffffff': '#12171b',  // cards and sheets
    '#f4f7fb': '#0d1114', '#eaf0f7': '#0b1013', '#dfe7f3': '#1a2126',
    '#e6edf9': '#232b31',  // soft accent: icon buttons, selected rows
    '#d3def2': '#2b343b', '#c9d6ea': '#303a41', '#b9cbe5': '#39444c',
    '#bdcdde': '#39444c', '#bccddd': '#39444c', '#a9bad9': '#6b5330',
    '#a3b1c2': '#46525b',  // dividers, tracks
    '#6c7e93': '#aab4bb', '#24364c': '#f4f6f7',
    '#4268b2': '#ffb454',  // the accent
    '#35558f': '#e09a3c',
    '#c0392b': '#ff6b5e',
  },
  mark: { '#ffffff': '#12171b', '#24364c': '#f4f6f7', '#6c7e93': '#aab4bb', '#a3b1c2': '#7d8a93' },
  page: '#0b1013',
  shadow: 0.3,
});
