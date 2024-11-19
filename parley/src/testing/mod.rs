use crate::layout::{cursor::VisualMode, Cursor};

struct TestLayout {
    pub text: String,
    pub layout: Layout,
    // ...
}

#[test]
fn next_visual() {
    let text = "Lorem ipsum dolor sit amet";
    let layout = TestLayout::new(text);

    let mut cursor: Cursor = layout.cursor_before("dolor");
    cursor = cursor.next_visual(layout.layout, VisualMode::Strong);

    layout.assert_cursor_is_after(cursor, "ipsum d");
}
