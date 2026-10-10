use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{Global, Platform};

pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(3);

/// Shares the application's existing platform, whose cursor hide lasts until pointer movement.
pub(crate) struct PlayerCursor(Rc<dyn Platform>);
impl Global for PlayerCursor {}
impl PlayerCursor {
    pub(crate) fn new(platform: Rc<dyn Platform>) -> Self {
        Self(platform)
    }
    pub(crate) fn hide_until_mouse_moves(&self) {
        self.0.hide_cursor_until_mouse_moves();
    }
}

pub(crate) struct PlayerChromeState {
    last_activity: Instant,
    visible: bool,
    pointer_inside: bool,
}

impl PlayerChromeState {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            last_activity: now,
            visible: true,
            pointer_inside: false,
        }
    }

    pub(crate) fn activity(&mut self, now: Instant) {
        self.last_activity = now;
        self.visible = true;
    }

    pub(crate) fn set_pointer_inside(&mut self, inside: bool) {
        self.pointer_inside = inside;
    }
    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }
    pub(crate) fn should_hide_cursor(&self) -> bool {
        !self.visible && self.pointer_inside
    }

    /// Returns whether the overlays changed, independently of playback progress or pointer location.
    pub(crate) fn expire(&mut self, now: Instant, interacting: bool) -> bool {
        if !self.visible || interacting || now.duration_since(self.last_activity) < IDLE_TIMEOUT {
            return false;
        }
        self.visible = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_overlays_hide_at_three_seconds_but_outside_cursor_stays_visible() {
        let now = Instant::now();
        let mut state = PlayerChromeState::new(now);
        assert!(!state.expire(now + Duration::from_millis(2999), false));
        assert!(state.is_visible());
        assert!(state.expire(now + IDLE_TIMEOUT, false));
        assert!(!state.is_visible());
        assert!(!state.should_hide_cursor());
    }

    #[test]
    fn cursor_hiding_is_scoped_to_the_player_and_movement_restores_chrome() {
        let now = Instant::now();
        let mut state = PlayerChromeState::new(now);
        state.set_pointer_inside(true);
        state.expire(now + IDLE_TIMEOUT, false);
        assert!(state.should_hide_cursor());
        state.set_pointer_inside(false);
        assert!(!state.should_hide_cursor());
        assert!(!state.is_visible());
        state.activity(now + IDLE_TIMEOUT);
        state.set_pointer_inside(true);
        assert!(state.is_visible());
        assert!(!state.should_hide_cursor());
        assert!(!state.expire(now + Duration::from_secs(5), false));
        assert!(state.expire(now + Duration::from_secs(6), false));
    }

    #[test]
    fn open_menu_and_drag_keep_controls_available() {
        let now = Instant::now();
        let mut state = PlayerChromeState::new(now);
        state.set_pointer_inside(true);
        assert!(!state.expire(now + IDLE_TIMEOUT, true));
        assert!(state.is_visible());
        assert!(!state.should_hide_cursor());
        state.activity(now + IDLE_TIMEOUT);
        assert!(!state.expire(now + Duration::from_secs(5), false));
        assert!(state.expire(now + Duration::from_secs(6), false));
    }
}
