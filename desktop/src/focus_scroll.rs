//! Reveal newly focused form rows without pinning them during manual scrolling.
use gpui::{prelude::*, *};

/// Draw a visible outline around a component whose focus handle is internal.
/// The wrapper observes descendants without adding a keyboard stop of its own.
#[derive(IntoElement)]
pub struct FocusRing {
    id: ElementId,
    child: AnyElement,
}

impl FocusRing {
    pub fn new(id: impl Into<ElementId>, child: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
        }
    }
}

impl RenderOnce for FocusRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| (cx.focus_handle(), false));
        let (focus, active) = state.read(cx).clone();
        let observed = state.clone();
        window.defer(cx, move |window, cx| {
            let active = observed.read(cx).0.contains_focused(window, cx);
            if active != observed.read(cx).1 {
                observed.update(cx, |state, _| state.1 = active);
                window.refresh();
            }
        });
        div()
            .id(self.id)
            .track_focus(&focus)
            .tab_stop(false)
            .flex_shrink_0()
            .border_2()
            .border_color(if active {
                crate::theme::color(crate::theme::INK).into()
            } else {
                transparent_black()
            })
            .rounded_full()
            .child(self.child)
    }
}

#[derive(Clone)]
enum RevealScroll {
    Handle(ScrollHandle),
    List(ListState),
}

impl RevealScroll {
    fn viewport(&self) -> Bounds<Pixels> {
        match self {
            RevealScroll::Handle(scroll) => scroll.bounds(),
            RevealScroll::List(state) => state.viewport_bounds(),
        }
    }

    fn scroll_by(&self, delta: Pixels) {
        match self {
            RevealScroll::Handle(scroll) => {
                scroll.set_offset(scroll.offset() + point(px(0.), delta));
            }
            RevealScroll::List(state) => state.scroll_by(delta),
        }
    }
}

#[derive(IntoElement)]
pub struct RevealFocus {
    id: ElementId,
    child: AnyElement,
    scroll: RevealScroll,
    full_width: bool,
}

struct State {
    container: FocusHandle,
    focused: Option<FocusHandle>,
    geometry: Option<(Pixels, Size<Pixels>)>,
}

impl RevealFocus {
    pub fn new(id: impl Into<ElementId>, child: impl IntoElement, scroll: ScrollHandle) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
            scroll: RevealScroll::Handle(scroll),
            full_width: true,
        }
    }

    /// Same reveal behavior against a variable-height `list` viewport.
    pub fn in_list(id: impl Into<ElementId>, child: impl IntoElement, list: ListState) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
            scroll: RevealScroll::List(list),
            full_width: true,
        }
    }

    pub fn inline(mut self) -> Self {
        self.full_width = false;
        self
    }
}

impl RenderOnce for RevealFocus {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| State {
            container: cx.focus_handle(),
            focused: None,
            geometry: None,
        });
        let focus = state.read(cx).container.clone();
        div()
            .on_children_prepainted(move |bounds, window, cx| {
                let Some(bounds) = bounds.first().copied() else {
                    return;
                };
                let state = state.clone();
                let scroll = self.scroll.clone();
                let geometry = (window.rem_size(), window.bounds().size);
                // Focus ancestry is only reliable after this frame has committed:
                // prepaint moves reused subtrees out of the previous dispatch tree.
                // This also covers a focus change while a saved-status row disappears.
                window.defer(cx, move |window, cx| {
                    let focused = window
                        .focused(cx)
                        .filter(|_| state.read(cx).container.contains_focused(window, cx));
                    state.update(cx, |state, _| {
                        let reveal = focused.is_some()
                            && (state.focused != focused || state.geometry != Some(geometry));
                        state.focused = focused;
                        state.geometry = Some(geometry);
                        if !reveal {
                            return;
                        }
                        let viewport = scroll.viewport();
                        if viewport.size.height <= px(0.) {
                            return;
                        }
                        let delta = reveal_delta(
                            f32::from(bounds.top()),
                            f32::from(bounds.bottom()),
                            f32::from(viewport.top()) + 8.,
                            f32::from(viewport.bottom()) - 8.,
                        );
                        if delta != 0. {
                            scroll.scroll_by(px(delta));
                            window.refresh();
                        }
                    });
                });
            })
            .id(self.id)
            .track_focus(&focus)
            .tab_stop(false)
            .flex_shrink_0()
            .when(self.full_width, |view| view.w_full())
            .min_w_0()
            .child(self.child)
    }
}

fn reveal_delta(top: f32, bottom: f32, visible_top: f32, visible_bottom: f32) -> f32 {
    if top < visible_top || bottom - top > visible_bottom - visible_top {
        visible_top - top
    } else if bottom > visible_bottom {
        visible_bottom - bottom
    } else {
        0.
    }
}

#[cfg(test)]
mod tests {
    use super::reveal_delta;

    #[test]
    fn reveals_clipped_controls_and_the_start_of_oversized_rows() {
        assert_eq!(reveal_delta(100., 160., 80., 400.), 0.);
        assert_eq!(reveal_delta(30., 100., 80., 400.), 50.);
        assert_eq!(reveal_delta(380., 440., 80., 400.), -40.);
        assert_eq!(reveal_delta(300., 900., 80., 400.), -220.);
    }
}
