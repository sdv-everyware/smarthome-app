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

/// (family, weight, file) for every face the design uses (Figma file "Smart HOme": Inter
/// 400/500/600, IBM Plex Mono 400). Inter 500/600 are not in the tree yet: until their files
/// are in data/, the nearest faces we have stand in for them.
pub const FONTS: &[(&str, u16, &[u8])] = &[
    ("Inter", 400, INTER_REGULAR),
    ("Inter", 500, INTER_REGULAR),
    ("Inter", 600, INTER_BOLD),
    ("IBM Plex Mono", 400, PLEX_MONO_REGULAR),
];

#[derive(Default)]
pub struct ResourceManager;

impl RendererHelper for ResourceManager {
    fn initialize_renderer(&self, renderer: &mut dyn Renderer) -> Result<(), Error> {
        // the pictures components show (everyware_ui::images: the news feed's)
        renderer.replace_raw_image_storage(everyware_ui::images::storage());
        for (family, weight, data) in FONTS {
            // SAFETY: OpenType fonts are supported by the renderer.
            unsafe { renderer.load_font(vec![FontInfo::new(family, *weight)], data)? };
        }
        Ok(())
    }
}
