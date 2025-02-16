use alloc::vec::Vec;
use alloc::collections::BTreeMap;

mod widgets;
mod text;

pub use widgets::button::ButtonConfig;
pub use widgets::dynamic_canvas::TileRenderer;
pub use widgets::progress_bar::ProgressBarConfig;
pub use widgets::static_canvas::set_autoscroll;
pub use widgets::graph::{GraphConfig, GraphSeries, GraphAggMode};
pub use widgets::horiz_bar::{BarValue, HorizBarConfig};
pub use widgets::text_box::{FormattableText, TextBoxState, EditableRichText};
pub use widgets::choice_buttons::{ChoiceConfig, ChoiceButtonsConfig};
pub use text::{render_rich_text, string_input};

pub use crate::content::{ContentId, UuidProvider};
use crate::{InputState, StyleSheet};
use crate::{FbViewMut, Framebuffer, OwnedPixels};
use crate::drawing::text::{Font, FontFamily, DEFAULT_FONT_FAMILY};


const TILE_CACHE_MAX_SIZE: usize = 10_000_000; // in bytes

struct CachedTile {
    fb: Framebuffer<OwnedPixels>,
    last_used_time: f64,
}

pub struct TileCache {
    tiles: BTreeMap<ContentId, CachedTile>,
}

impl TileCache {

    fn new() -> Self {
        Self {
            tiles: BTreeMap::new(),
        }
    }

    fn fetch_or_create<F>(&mut self, content_id: ContentId, time: f64, create_func: F) -> &Framebuffer<OwnedPixels> 
        where F: FnOnce() -> Framebuffer<OwnedPixels>
    
    {
        let cached_tile = self.tiles.entry(content_id).or_insert_with(|| {
            let tile_fb = create_func();
            CachedTile { fb: tile_fb, last_used_time: time }
        });

        cached_tile.last_used_time = time;

        &cached_tile.fb
    }

    fn cleanup(&mut self) {

        let mut pairs = Vec::with_capacity(self.tiles.len());
        while let Some((key, tile)) = self.tiles.pop_last() {
            pairs.push((key, tile));
        }

        pairs.sort_unstable_by(|(_, tile_1), (_, tile_2)| {
            tile_2.last_used_time.partial_cmp(&tile_1.last_used_time).unwrap()
        });

        let mut current_size = 0;
        let mut evicted_count = 0;
        let mut evicted_size = 0;
        for (key, tile) in pairs.into_iter() {
            if current_size < TILE_CACHE_MAX_SIZE {
                current_size += tile.fb.size_bytes();
                self.tiles.insert(key, tile);
            } else {
                evicted_count += 1;
                evicted_size += tile.fb.size_bytes();
            }
        }

        // if evicted_count > 0 {
        //     log::debug!(
        //         "Evicted {} tiles from cache ({:.2} MB), {} remaining ({:.2} MB)",
        //         evicted_count,
        //         evicted_size as f64 / 1_000_000.0,
        //         self.tiles.len(),
        //         self.tiles.values().map(|tile| tile.fb.size_bytes()).sum::<usize>() as f64 / 1_000_000.0,
        //     );
        // }
    }
}

pub struct UiContext<'a, F: FbViewMut> {
    pub fb: &'a mut F,

    pub stylesheet: &'a StyleSheet,
    pub input_state: &'a InputState,
    pub uuid_provider: &'a mut UuidProvider,
    pub time: f64,

    pub tile_cache: &'a mut TileCache,
    pub font_family: &'static FontFamily,
}

pub struct UiStore {
    tile_cache: TileCache,
}

impl UiStore {
    pub fn new() -> Self {
        Self {
            tile_cache: TileCache::new(),
        }
    }

    pub fn get_context<'a, F: FbViewMut>(
        &'a mut self,
        fb: &'a mut F,
        stylesheet: &'a StyleSheet,
        input_state: &'a InputState,
        uuid_provider: &'a mut UuidProvider,
        time: f64,
    ) -> UiContext<'a, F> {

        self.tile_cache.cleanup();

        UiContext {
            fb,
            stylesheet,
            tile_cache: &mut self.tile_cache,
            input_state,
            uuid_provider,
            time,
            font_family: &DEFAULT_FONT_FAMILY,
        }
    }
}
