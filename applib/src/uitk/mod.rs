use alloc::vec::Vec;
use alloc::collections::BTreeMap;

mod button;
mod dyn_scrollable_canvas;
mod progress_bar;
mod scrollable_canvas;
mod text;

pub use button::ButtonConfig;
pub use dyn_scrollable_canvas::{TileRenderer};
pub use progress_bar::ProgressBarConfig;
pub use scrollable_canvas::set_autoscroll;
pub use text::{render_rich_text, string_input, EditableTextConfig, ScrollableTextConfig};

pub use crate::content::{ContentId, UuidProvider};
use crate::InputState;
use crate::{FbViewMut, Framebuffer, OwnedPixels};

pub struct CachedTile {
    fb: Framebuffer<OwnedPixels>,
    time: f64,
}

pub struct TileCache {
    pub tiles: BTreeMap<ContentId, CachedTile>,
}

impl TileCache {
    fn new() -> Self {
        Self {
            tiles: BTreeMap::new(),
        }
    }
}

pub struct UiContext<'a, F: FbViewMut> {
    pub fb: &'a mut F,
    pub tile_cache: &'a mut TileCache,
    pub input_state: &'a InputState,
    pub uuid_provider: &'a mut UuidProvider,
    pub time: f64,
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
        input_state: &'a InputState,
        uuid_provider: &'a mut UuidProvider,
        time: f64,
    ) -> UiContext<'a, F> {

        // TODO: need to rethink this
        self.cleanup_tile_cache();

        UiContext {
            fb,
            tile_cache: &mut self.tile_cache,
            input_state,
            uuid_provider,
            time,
        }
    }

    fn cleanup_tile_cache(&mut self) {

        const TO_KEEP: usize = 8;

        let tiles = &mut self.tile_cache.tiles;

        let mut pairs = Vec::with_capacity(tiles.len());
        while let Some((key, tile)) = tiles.pop_last() {
            pairs.push((key, tile));
        }

        pairs.sort_unstable_by(|(_, tile_1), (_, tile_2)| tile_2.time.partial_cmp(&tile_1.time).unwrap());

        for (key, tile) in pairs.into_iter().take(TO_KEEP) {
            tiles.insert(key, tile);
        }
    }
}
