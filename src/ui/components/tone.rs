//! Semantic color roles shared by all components.

use iced::Color;

use crate::ui::theme::Palette;

/// What a piece of UI means, not what color it is. The theme decides the
/// color, so components stay correct in every theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Primary brand / "yours" (local draft, primary action).
    Accent,
    /// Neutral information (upstream changed).
    Info,
    Success,
    Warning,
    /// Errors and conflicts.
    Danger,
    /// Secondary but readable (in review).
    Neutral,
    /// De-emphasised (deleted, placeholders).
    Subtle,
}

impl Tone {
    pub fn color(self) -> Color {
        match self {
            Tone::Accent => Palette::accent(),
            // No info token in the palette yet; the PUT blue is the closest
            // neutral-informative hue. Swap here once 09 adds one.
            Tone::Info => Palette::PUT,
            Tone::Success => Palette::SUCCESS,
            Tone::Warning => Palette::WARNING,
            Tone::Danger => Palette::ERROR,
            Tone::Neutral => Palette::text_muted(),
            Tone::Subtle => Palette::text_subtle(),
        }
    }

    /// The tone mixed into the current surface color, fully opaque, so a
    /// tinted area looks the same whatever is drawn beneath it.
    pub fn tint(self, amount: f32) -> Color {
        mix(Palette::surface(), self.color(), amount)
    }
}

/// `amount` of `over` mixed into `base` (0 = base, 1 = over). Opaque.
pub fn mix(base: Color, over: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    let m = |a: f32, b: f32| a * (1.0 - t) + b * t;
    Color::from_rgb(m(base.r, over.r), m(base.g, over.g), m(base.b, over.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_hits_both_ends_and_stays_opaque() {
        let a = Color::from_rgb(0.0, 0.0, 0.0);
        let b = Color::from_rgb(1.0, 0.5, 0.25);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        let mid = mix(a, b, 0.5);
        assert!((mid.r - 0.5).abs() < 1e-6 && (mid.a - 1.0).abs() < 1e-6);
        assert_eq!(mix(a, b, 7.0), b, "amount is clamped");
    }

    #[test]
    fn status_tones_are_distinct() {
        let tones = [Tone::Accent, Tone::Info, Tone::Danger, Tone::Success];
        for (i, a) in tones.iter().enumerate() {
            for b in &tones[i + 1..] {
                assert_ne!(a.color(), b.color(), "{a:?} vs {b:?}");
            }
        }
    }
}
