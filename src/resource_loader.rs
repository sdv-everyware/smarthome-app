/*
 * smarthome-app: fonts. harry needs them twice: the renderer draws with its fonts, the layout
 * helper measures text with its own (main.rs); a text whose font the helper lacks is measured
 * ~0 px wide and breaks into single letters.
 */

use har_platform_api::error::Error;
use har_platform_api::graphics::{FontInfo, Renderer};
use har_platform_api::looper::RendererHelper;

const INTER_REGULAR: &[u8] = include_bytes!("../data/inter_regular.otf");
const INTER_BOLD: &[u8] = include_bytes!("../data/inter_bold.otf");
/// IBM Plex Mono Regular (github.com/IBM/plex, SIL OFL 1.1: data/ibm_plex_mono_LICENSE.txt):
/// the clock's digits.
const PLEX_MONO_REGULAR: &[u8] = include_bytes!("../data/ibm_plex_mono_regular.otf");
const INTER_BLACK: &[u8] = include_bytes!("../data/inter_black.otf");
// the headline face of the look "Poster" (Big Shoulders Display, SIL OFL 1.1: two weights cut
// from the variable font, data/big_shoulders_display_LICENSE.txt)
const SHOULDERS_BLACK: &[u8] = include_bytes!("../data/big_shoulders_display_black.ttf");
const SHOULDERS_MEDIUM: &[u8] = include_bytes!("../data/big_shoulders_display_medium.ttf");

/// (family, weight, file) for every face the design uses (Figma file "Smart HOme": Inter
/// 400/500/600, IBM Plex Mono 400). Inter 500/600 are not in the tree yet: until their files
/// are in data/, the nearest faces we have stand in for them.
pub const FONTS: &[(&str, u16, &[u8])] = &[
    ("Inter", 400, INTER_REGULAR),
    ("Inter", 500, INTER_REGULAR),
    ("Inter", 600, INTER_BOLD),
    ("IBM Plex Mono", 400, PLEX_MONO_REGULAR),
    ("Inter", 900, INTER_BLACK),
    ("Big Shoulders Display", 900, SHOULDERS_BLACK),
    ("Big Shoulders Display", 500, SHOULDERS_MEDIUM),
];

#[derive(Default)]
pub struct ResourceManager;

impl RendererHelper for ResourceManager {
    fn initialize_renderer(&self, renderer: &mut dyn Renderer) -> Result<(), Error> {
        // the pictures components show (everyware_ui::images: the agenda's QR code, the bars' colours)
        renderer.replace_raw_image_storage(everyware_ui::images::storage());
        for (family, weight, data) in FONTS {
            // SAFETY: OpenType fonts are supported by the renderer.
            unsafe { renderer.load_font(vec![FontInfo::new(family, *weight)], data)? };
        }
        Ok(())
    }
}
