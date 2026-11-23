#!/bin/bash
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# A look of the panel: another screen for the same app. The look's patch draws a new
# #smarthome-root on the EveryWare Library with the panel's node names (src/ui_model.rs), the
# panel's own patches add the sheets (Wi-Fi and the keyboard, the device, the settings, the
# agenda's event), and the look's theme recolours the whole file. Usage, from anywhere:
#
#   looks/build.sh poster     # -> design/looks/poster.fig and data/looks/poster/smarthome.dcf
#
# design/looks/<look>.fig is in git, as design/smarthome.fig: written only when the document
# changed. The steps before it are in design/build/looks/<look>/.
set -euo pipefail
LOOK=${1:?usage: build.sh <look>}
HERE=$(cd "$(dirname "$0")" && pwd)
DESIGN=$(dirname "$HERE")
TOP=$(cd "$HERE/../../../../../../.." && pwd)
FIGMA=${EVERYWARE_FIGMA:-$TOP/vendor/sdv-everyware/everyware/design/figma}
UI=${EVERYWARE_UI:-$TOP/vendor/sdv-everyware/ui-lib}
[ -f "$HERE/$LOOK-patch.js" ] || { echo "no look $LOOK: $HERE/$LOOK-patch.js is missing" >&2; exit 1; }
B="$DESIGN/build/looks/$LOOK"
mkdir -p "$B/out"
rt() { node "$FIGMA/roundtrip.js" "$@"; }

"$UI/design/build-library.sh"
rt "$UI/design/build/library.fig" "$B/1.fig" "$HERE/$LOOK-patch.js"
i=1
for p in keyboard sheet prefs event; do rt "$B/$i.fig" "$B/$((i + 1)).fig" "$DESIGN/$p-patch.js"; i=$((i + 1)); done
rt "$B/$i.fig" "$B/$LOOK.fig" "$HERE/$LOOK-theme.js"

OUT="$HERE/$LOOK.fig"
if [ -f "$OUT" ] && cmp -s <(unzip -p "$OUT") <(unzip -p "$B/$LOOK.fig"); then
    echo "built $OUT (the same document: unchanged)"
else
    cp "$B/$LOOK.fig" "$OUT"
    echo "built $OUT"
fi
# the importer names the root after the output's file (smarthome.dcf -> #smarthome-root)
cp "$OUT" "$B/out/smarthome.fig"
DCF_CHECK="${CHECK:-#home/date #settings #status/wifi #wifi/card #device/card #prefs/card #event/card}" \
    "$TOP/vendor/sdv-everyware/everyware/scripts/fig-to-dcf.sh" "$B/out/smarthome.fig" "$B/out/smarthome.dcf"
# a look is a directory with the document under the panel's own name (src/config.rs Looks)
mkdir -p "$DESIGN/../data/looks/$LOOK"
cp "$B/out/smarthome.dcf" "$DESIGN/../data/looks/$LOOK/smarthome.dcf"
echo "built $DESIGN/../data/looks/$LOOK/smarthome.dcf"
echo "review in Figma (File > Import): $OUT"
