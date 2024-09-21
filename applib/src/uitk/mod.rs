use alloc::collections::BTreeMap;

mod button;
mod dyn_scrollable_canvas;
mod progress_bar;
mod scrollable_canvas;
mod text;

pub use button::ButtonConfig;
pub use dyn_scrollable_canvas::{TileRenderContext, TileRenderer};
pub use progress_bar::ProgressBarConfig;
pub use scrollable_canvas::set_autoscroll;
pub use text::{render_rich_text, string_input, EditableTextConfig, ScrollableTextConfig};

pub use crate::content::{ContentId, UuidProvider};
use crate::{InputState};
use crate::{FbViewMut, Framebuffer, OwnedPixels};

pub struct TileCache {
    pub tiles: BTreeMap<ContentId, Framebuffer<OwnedPixels>>,
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
    ) -> UiContext<'a, F> {
        UiContext {
            fb,
            tile_cache: &mut self.tile_cache,
            input_state,
            uuid_provider,
        }
    }
}
