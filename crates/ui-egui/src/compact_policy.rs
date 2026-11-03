//! Presentation policy only: never an editor workspace or a document preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preference {
    #[default]
    Auto,
    Compact,
    Desktop,
}

impl Preference {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Compact, Self::Desktop];
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Compact => "compact",
            Self::Desktop => "desktop",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Compact => "Compact",
            Self::Desktop => "Desktop",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "compact" => Self::Compact,
            "desktop" => Self::Desktop,
            _ => Self::Auto,
        }
    }
}

/// Hysteresis uses the native content rectangle in logical points, never the browser's outer size.
pub fn resolve(preference: Preference, was_compact: bool, width: f32, height: f32, gesture: bool) -> bool {
    if gesture || !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return was_compact;
    }
    match preference {
        Preference::Compact => true,
        Preference::Desktop => false,
        Preference::Auto if was_compact => !(width >= 960.0 && height >= 640.0),
        Preference::Auto => width < 900.0 || height < 600.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phone_or_short_content_is_compact() {
        for (w, h) in [(390.0, 747.0), (844.0, 293.0), (1200.0, 599.0)] {
            assert!(resolve(Preference::Auto, false, w, h, false));
        }
        assert!(!resolve(Preference::Auto, false, 900.0, 600.0, false));
    }
    #[test]
    fn desktop_exit_requires_both_hysteresis_thresholds() {
        assert!(resolve(Preference::Auto, true, 959.0, 900.0, false));
        assert!(resolve(Preference::Auto, true, 1200.0, 639.0, false));
        assert!(!resolve(Preference::Auto, true, 960.0, 640.0, false));
    }
    #[test]
    fn held_gesture_and_invalid_size_preserve_presentation() {
        assert!(!resolve(Preference::Compact, false, 390.0, 844.0, true));
        assert!(resolve(Preference::Desktop, true, 1440.0, 960.0, true));
        for w in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            assert!(resolve(Preference::Auto, true, w, 960.0, false));
        }
    }
    #[test]
    fn explicit_choice_and_unknown_storage_are_bounded() {
        assert!(resolve(Preference::Compact, false, 1440.0, 960.0, false));
        assert!(!resolve(Preference::Desktop, true, 390.0, 844.0, false));
        assert_eq!(Preference::parse("malformed"), Preference::Auto);
        for p in Preference::ALL {
            assert_eq!(Preference::parse(p.id()), p);
        }
    }
}
