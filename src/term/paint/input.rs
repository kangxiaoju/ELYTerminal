use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, ElementInputHandler, InputHandler, Pixels, Point,
    TextInputConfiguration, UTF16Selection, WeakEntity, Window,
};

use super::TerminalView;

// Native windows can retain input handlers past GPUI's entity teardown.
pub(super) struct TerminalInput {
    pub view: WeakEntity<TerminalView>,
    pub bounds: Bounds<Pixels>,
}

impl TerminalInput {
    fn with<R: Default>(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut ElementInputHandler<TerminalView>, &mut App) -> R,
    ) -> R {
        let Some(view) = self.view.upgrade() else {
            return R::default();
        };
        f(&mut ElementInputHandler::new(self.bounds, view), cx)
    }
}

impl InputHandler for TerminalInput {
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.with(cx, |input, cx| input.selected_text_range(ignore_disabled_input, window, cx))
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.with(cx, |input, cx| input.marked_text_range(window, cx))
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.with(cx, |input, cx| input.text_for_range(range, adjusted_range, window, cx))
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.with(cx, |input, cx| input.replace_text_in_range(range, text, window, cx));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.with(cx, |input, cx| {
            input.replace_and_mark_text_in_range(range, text, selected_range, window, cx)
        });
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.with(cx, |input, cx| input.unmark_text(window, cx));
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut App) {
        self.with(cx, |input, cx| input.paste(item, window, cx));
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.with(cx, |input, cx| input.bounds_for_range(range, window, cx))
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Bounds<Pixels>> {
        self.with(cx, |input, cx| input.selection_bounds_for_range(range, window, cx))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.with(cx, |input, cx| input.character_index_for_point(point, window, cx))
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.with(cx, |input, cx| input.set_selected_text_range(range, window, cx));
    }

    fn element_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.with(cx, |input, cx| input.element_bounds(window, cx))
    }

    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut App) -> Option<usize> {
        self.with(cx, |input, cx| input.text_length_utf16(window, cx))
    }

    fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.with(cx, |input, cx| input.accepts_text_input(window, cx))
    }

    fn prefers_ime_for_printable_keys(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.with(cx, |input, cx| input.prefers_ime_for_printable_keys(window, cx))
    }

    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> TextInputConfiguration {
        self.with(cx, |input, cx| input.text_input_configuration(window, cx))
    }

    fn text_input_editable_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.with(cx, |input, cx| input.text_input_editable_range(window, cx))
    }
}
