#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WidgetState {
    #[default]
    Enabled,
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

impl WidgetState {
    /// Returns `true` if the widget is in the [`Disabled`](Self::Disabled) state.
    #[inline]
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_state() {
        assert!(WidgetState::Disabled.is_disabled());
        assert!(!WidgetState::Enabled.is_disabled());
        assert_eq!(WidgetState::default(), WidgetState::Enabled);
    }
}
