#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanelState {
    pub is_expanded: bool,
    pub enlarged: bool,
}

impl PanelState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn toggle(&mut self) {
        self.is_expanded = !self.is_expanded;
    }
    pub fn expand(&mut self) {
        self.is_expanded = true;
    }
    pub fn collapse(&mut self) {
        self.is_expanded = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collapsing_preserves_enlargement_preference() {
        let mut panel = PanelState::new();
        panel.enlarged = true;
        panel.toggle();
        assert!(panel.is_expanded);
        panel.toggle();
        assert!(!panel.is_expanded);
        assert!(panel.enlarged);
    }
}
