use crate::Color;

#[derive(Clone)]
#[repr(C)]
pub struct StyleSheet {
    pub colors: StyleSheetColors,
}

#[derive(Clone)]
#[repr(C)]
pub struct StyleSheetColors {
    pub background: Color,
    pub hover_overlay: Color,
    pub selected_overlay: Color,
    pub red: Color,
    pub yellow: Color,
    pub green: Color,
    pub blue: Color,
    pub element: Color,
    pub text: Color,
    pub accent: Color,
}