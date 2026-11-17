#!/bin/bash
# The Smart HOme panel's design, from the designer's export in source/: the component library
# (ui-lib's components-patch.js), then this app's patches: the Wi-Fi sheet (its content in the
# library's #sheet), the dashboard's widgets (which use the library), the tabs and the screens
# Rooms and Automations, the device sheet, the #sheet container with #panel, #prefs, #event and
# #game (with the game button in the status row), the camera's offline state and full view
# (#camera-view) and, for the designer, the sheets as frames with a prototype. Usage, from anywhere:
#
#   build.sh            # -> design/smarthome.fig
#   build.sh --dcf      # and data/smarthome.dcf from it (fig-to-dcf.sh, the previous one kept)
#
# design/smarthome.fig is in git, as data/smarthome.dcf: commit it with every change of the
# design. It is written only when the document changed (a zip carries the time it was made, so
# a build of the same design leaves git clean). The steps before it are in design/build/,
# ignored.
# A new export from the designer replaces source/Smart HOme.fig in a commit. The tools are
# everyware's design/figma (EVERYWARE_FIGMA), the library ui-lib (EVERYWARE_UI), both found in
# the tree by default. Needs node, unzip and zip; --dcf also the tree's importer (cargo).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
TOP=$(cd "$HERE/../../../../../.." && pwd)
FIGMA=${EVERYWARE_FIGMA:-$TOP/vendor/sdv-everyware/everyware/design/figma}
UI=${EVERYWARE_UI:-$TOP/vendor/sdv-everyware/ui-lib}
IN="$HERE/source/Smart HOme.fig"
B="$HERE/build"
mkdir -p "$B/steps"
rt() { node "$FIGMA/roundtrip.js" "$@"; }
rt "$IN" "$B/steps/1.fig" "$UI/design/components-patch.js"
rt "$B/steps/1.fig" "$B/steps/2.fig" "$HERE/keyboard-patch.js"
rt "$B/steps/2.fig" "$B/steps/3.fig" "$HERE/dashboard-patch.js"
rt "$B/steps/3.fig" "$B/steps/4.fig" "$HERE/screens-patch.js"
rt "$B/steps/4.fig" "$B/steps/5.fig" "$HERE/sheet-patch.js"
rt "$B/steps/5.fig" "$B/steps/6.fig" "$HERE/panel-patch.js"
rt "$B/steps/6.fig" "$B/steps/7.fig" "$HERE/prefs-patch.js"
rt "$B/steps/7.fig" "$B/steps/8.fig" "$HERE/event-patch.js"
rt "$B/steps/8.fig" "$B/steps/9.fig" "$HERE/game-patch.js"
rt "$B/steps/9.fig" "$B/steps/10.fig" "$HERE/camera-patch.js"
rt "$B/steps/10.fig" "$B/smarthome.fig" "$HERE/prototype-patch.js"
OUT="$HERE/smarthome.fig"
if [ -f "$OUT" ] && cmp -s <(unzip -p "$OUT") <(unzip -p "$B/smarthome.fig"); then
    echo "built $OUT (the same document: unchanged)"
else
    cp "$B/smarthome.fig" "$OUT"
    echo "built $OUT"
fi
if [ "${1:-}" = --dcf ]; then
    DCF_CHECK='#home/title #camera/feed #camera/live #camera/offline #camera/open #camera-view #camera-view/picture #camera-view/offline #camera-view/live #camera-view/close' \
        "$TOP/vendor/sdv-everyware/everyware/scripts/fig-to-dcf.sh" "$OUT" "$HERE/../data/smarthome.dcf"
fi
# the last line: the file to give the user for a review
echo "review in Figma (File > Import): $OUT"
