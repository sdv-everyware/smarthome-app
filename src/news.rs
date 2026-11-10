//! The news feed of the dashboard's right column (#news/list): one ui-lib list item (kind image,
//! lines three) per story of the news bundle's feed (topic `news`, through the dashboard
//! bundle): the headline, the article's summary (else the site, the points and the comments),
//! the age at the end. The picture of a story is the image registered under "news/<id>" (the
//! thumbnails' topic); until then the row keeps the design's placeholder.

use everyware_dashboard_grpc::news::{NewsFeed, NewsStory, NewsThumbnail};
use everyware_ui::images;
use everyware_ui::list_item::{ListItem, ListItemKind, ListItemLines, ListItemSize};
use everyware_ui::list_view::ListView;

/// The list's viewport: the column below its heading (design/dashboard-patch.js).
pub const LIST_H: f32 = 744.0;
/// Row height (lines three) plus the gap.
pub const ROW_PITCH: f32 = 128.0 + 8.0;

/// The feed before the first message: no rows.
pub fn empty() -> ListView {
    ListView::new([], LIST_H, ROW_PITCH)
}

/// The rows of a feed at `now` (Unix time, for the age).
pub fn rows(feed: &NewsFeed, now: i64) -> Vec<ListItem> {
    feed.stories.iter().map(|s| row(s, now)).collect()
}

fn row(story: &NewsStory, now: i64) -> ListItem {
    ListItem::new(ListItemKind::Image, story.title.clone(), supporting(story))
        .lines(ListItemLines::Three)
        .size(ListItemSize::Column)
        .image(picture_key(story.id))
        .trailing_text(age(now - story.time))
}

/// The key of a story's picture (everyware_ui::images).
pub fn picture_key(id: u64) -> String {
    format!("news/{id}")
}

/// A story's thumbnail becomes its picture (the next frame shows it in its row).
pub fn set_picture(thumbnail: &NewsThumbnail) {
    if let Err(err) = images::register_rgb(picture_key(thumbnail.id), &thumbnail.rgb, thumbnail.width, thumbnail.height) {
        log::warn!("smarthome: thumbnail of story {}: {err}", thumbnail.id);
    }
}

/// The pictures of stories that left the feed are freed.
pub fn forget_pictures(feed: &NewsFeed) {
    for key in images::keys("news/") {
        if !feed.stories.iter().any(|s| picture_key(s.id) == key) {
            images::unregister(&key);
        }
    }
}

/// The article's summary, else what Hacker News says about the story.
fn supporting(story: &NewsStory) -> String {
    if !story.summary.is_empty() {
        return story.summary.clone();
    }
    let mut parts = Vec::new();
    if !story.site.is_empty() {
        parts.push(story.site.clone());
    }
    parts.push(format!("{} points", story.points));
    parts.push(format!("{} comments", story.comments));
    parts.join(" · ")
}

/// "35 min", "2 h", "3 d".
fn age(secs: i64) -> String {
    let min = secs.max(0) / 60;
    match min {
        0..=59 => format!("{} min", min.max(1)),
        60..=1439 => format!("{} h", min / 60),
        _ => format!("{} d", min / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_of_a_story() {
        let mut s = NewsStory::new();
        s.id = 9;
        s.title = "T".into();
        s.site = "x.org".into();
        s.points = 120;
        s.comments = 45;
        s.time = 1_000;
        assert_eq!(supporting(&s), "x.org · 120 points · 45 comments");
        s.summary = "An article".into();
        let r = row(&s, 1_000 + 2 * 3600 + 59);
        assert_eq!((r.subtitle.as_str(), r.trailing_text.as_str(), r.image.as_deref()), ("An article", "2 h", Some("news/9")));
        assert_eq!((age(10), age(3 * 86400)), ("1 min".to_string(), "3 d".to_string()));
    }
}
