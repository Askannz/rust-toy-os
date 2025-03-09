use alloc::vec::Vec;
use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font};
use crate::uitk::{UiContext, ButtonConfig};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use alloc::borrow::ToOwned;
use alloc::string::String;
use num::traits::float::FloatCore;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn choice_buttons_exclusive(&mut self, config: &ChoiceButtonsConfig, selected: &mut usize) {

        let mut new_selected = *selected;

        let func = |context: &mut Self, i: usize, button_rect: &Rect| {

            let choice: &ChoiceConfig = &config.choices[i];
            let mut active = i == *selected;

            context.button_toggle(
                &ButtonConfig {
                    rect: button_rect.clone(),
                    text: choice.text.clone(),
                    icon: choice.icon,
                    freeze: i == *selected,
                },
                &mut active
            );

            if active && i != *selected {
                new_selected = i;
            }
        };

        
        self.layout_horiz(&config.rect, config.choices.len(), func);

        *selected = new_selected;
    }

    pub fn choice_buttons_multi(&mut self, config: &ChoiceButtonsConfig, selected: &mut Vec<usize>) {

        let Rect { x0: mut x, y0, w, h } = config.rect;
        let button_w = w / (config.choices.len() as u32);

        let mut new_selected = Vec::new();

        for (i, choice) in config.choices.iter().enumerate() {

            let mut active = selected.contains(&i);

            self.button_toggle(
                &ButtonConfig {
                    rect: Rect { x0: x, y0, w: button_w, h },
                    text: choice.text.clone(),
                    icon: choice.icon,
                    freeze: false,
                },
                &mut active
            );

            x += button_w as i64;

            if active {
                new_selected.push(i);
            }
        }

        *selected = new_selected;
    }
}


pub struct ChoiceButtonsConfig {
    pub rect: Rect,
    pub choices: Vec<ChoiceConfig>
}


#[derive(Clone, Default)]
pub struct ChoiceConfig {
    pub text: String,
    pub icon: Option<&'static Framebuffer<OwnedPixels>>,
}
