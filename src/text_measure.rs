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

//! Text measured as harry lays it out: a layout helper of the app's own (made by the same
//! manager as harry's, with the same fonts) measures a paragraph with Impeller's typography, the
//! way harry measures a text node. The agenda counts the lines of a headline with it, so a row is
//! as high as its headline (agenda.rs), and the event's panel measures its texts, so its body
//! scrolls as far as it reaches. Before the helper is set, or when it cannot measure, the height
//! is estimated from the font's size (a warning in the log, once).

use display_list::DisplayListAppearance;
use har_platform_api::looper::LayoutHelper;
use squoosh::drawable::bounds::Rect;
use squoosh::drawable::layout::Dimension;
use squoosh::drawable::style::Style;
use squoosh::drawable::text::LineHeight;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A text style of the design: the family, the weight, the size and the line height in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    pub family: &'static str,
    pub weight: f32,
    pub size: f32,
    pub line_height: f32,
}

static HELPER: Mutex<Option<Box<dyn LayoutHelper + Send>>> = Mutex::new(None);
static ESTIMATED: AtomicBool = AtomicBool::new(false);

/// The helper that measures (main.rs, from the layout helper manager harry gets).
pub fn set_helper(helper: Box<dyn LayoutHelper + Send>) {
    if let Ok(mut h) = HELPER.lock() {
        *h = Some(helper);
    }
}

/// The height of `text` in `font` wrapped to `width`, as harry lays it out.
pub fn height(text: &str, font: &Font, width: f32) -> f32 {
    measured(text, font, width).unwrap_or_else(|| {
        if !ESTIMATED.swap(true, Ordering::Relaxed) {
            log::warn!("smarthome: text not measured by the layout helper: its height is estimated");
        }
        estimate(text, font, width)
    })
}

/// How many lines `text` takes in `font` at `width`: at least one.
pub fn lines(text: &str, font: &Font, width: f32) -> usize {
    ((height(text, font, width) / font.line_height).round() as usize).max(1)
}

fn measured(text: &str, font: &Font, width: f32) -> Option<f32> {
    let mut style = Style {
        opacity: 1.0,
        font_family: Some(font.family.to_string()),
        font_size: font.size,
        font_weight: font.weight,
        line_height: LineHeight::Pixels(font.line_height),
        ..Default::default()
    };
    // the paragraph wraps at the area's width only when the layout has one
    style.layout.size.width = Dimension::Points(width);
    let appearance = DisplayListAppearance::Text { area: Rect::new(0.0, 0.0, width, 100_000.0), text: text.to_string(), style: Arc::new(style) };
    let helper = HELPER.lock().ok()?;
    let size = helper.as_ref()?.measure_appearance(&appearance).ok().flatten()?;
    (size.height > 0.0).then_some(size.height)
}

/// About half an em a character, the lines a paragraph takes; for a text that cannot be
/// measured.
fn estimate(text: &str, font: &Font, width: f32) -> f32 {
    let per_line = (width / (font.size * 0.55)).max(1.0) as usize;
    let lines: usize = text.split('\n').map(|l| l.chars().count().div_ceil(per_line).max(1)).sum();
    lines as f32 * font.line_height
}
