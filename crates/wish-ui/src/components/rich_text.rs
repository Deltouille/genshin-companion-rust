use gpui_kit::{FontWeight, HighlightStyle, SharedString, StyledText};

/// Game text where `**…**` marks a term the game highlights: the markers go,
/// the terms turn semibold.
pub fn rich_text(text: &str) -> StyledText {
    let mut plain = String::with_capacity(text.len());
    let mut bold = Vec::new();
    for (i, part) in text.split("**").enumerate() {
        let start = plain.len();
        plain.push_str(part);
        // Odd parts sit between a pair of markers.
        if i % 2 == 1 && !part.is_empty() {
            bold.push((
                start..plain.len(),
                HighlightStyle {
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(SharedString::from(plain)).with_highlights(bold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_go() {
        // Only the text is checked: the highlights need a window to render.
        let mut plain = String::new();
        for part in "Uses **Windborne Sword** at its **highest** level".split("**") {
            plain.push_str(part);
        }
        assert_eq!(plain, "Uses Windborne Sword at its highest level");
        let _ = rich_text("no markers");
    }
}
