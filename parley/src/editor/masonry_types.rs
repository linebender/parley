use dpi::{LogicalPosition, PhysicalPosition};
use std::collections::HashSet;
use winit::{
    event::{Ime, KeyEvent, Modifiers},
    keyboard::ModifiersState,
};

// TODO - Clipboard Paste?
// TODO skip is_synthetic=true events
#[derive(Debug, Clone)]
pub enum TextEvent {
    KeyboardKey(KeyEvent, ModifiersState),
    Ime(Ime),
    ModifierChange(ModifiersState),
    // TODO - Document difference with Lifecycle focus change
    FocusChange(bool),
}

/// An indicator of which pointer button was pressed.
#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
#[repr(u8)]
pub enum PointerButton {
    /// No mouse button.
    None,
    /// Primary button, commonly the left mouse button, touch contact, pen contact.
    Primary,
    /// Secondary button, commonly the right mouse button, pen barrel button.
    Secondary,
    /// Auxiliary button, commonly the middle mouse button.
    Auxiliary,
    /// X1 (back) Mouse.
    X1,
    /// X2 (forward) Mouse.
    X2,
    /// Other mouse button. This isn't fleshed out yet.
    Other,
}

#[derive(Debug, Clone)]
pub struct PointerState {
    pub physical_position: PhysicalPosition<f64>,
    pub position: LogicalPosition<f64>,
    pub buttons: HashSet<PointerButton>,
    pub mods: Modifiers,
    pub count: u8,
    pub focus: bool,
}

/// An enum for specifying whether an event was handled.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Handled {
    /// An event was already handled, and shouldn't be propagated to other event handlers.
    Yes,
    /// An event has not yet been handled.
    No,
}

impl Handled {
    /// Has the event been handled yet?
    pub fn is_handled(self) -> bool {
        self == Handled::Yes
    }
}

impl From<bool> for Handled {
    /// Returns `Handled::Yes` if `handled` is true, and `Handled::No` otherwise.
    fn from(handled: bool) -> Handled {
        if handled {
            Handled::Yes
        } else {
            Handled::No
        }
    }
}
