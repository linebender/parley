/// Identifier for a family in a font library.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Debug)]
pub struct FamilyId(pub(crate) u32);

/// Identifier for a font in a font library.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Debug)]
pub struct FontId(pub(crate) u32);

/// Identifier for a source in a font library.
///
/// This corresponds to a file name which can be combined with
/// [`SourcePaths`](super::SourcePaths) to locate a font file, a full
/// path to a font file, or a user registered buffer containing font data.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Debug)]
pub struct SourceId(pub(crate) u32);

const INDEX_MASK: u32 = 0x7FFFFFFF;
const USER_BIT: u32 = 0x80000000;

macro_rules! impl_id {
    ($name: ident) => {
        impl $name {
            pub(crate) const fn new(index: u32) -> Self {
                Self(index)
            }

            pub(crate) const fn new_user(index: u32) -> Self {
                Self(index | USER_BIT)
            }

            pub(crate) fn alloc(index: usize, is_user: bool) -> Option<Self> {
                if index >= i32::MAX as usize {
                    return None;
                }
                let mut id = index as u32;
                if is_user {
                    id |= USER_BIT;
                }
                Some(Self(id))
            }

            /// Returns true if the identifier represents a dynamically
            /// registered user font.
            pub fn is_user_font(self) -> bool {
                self.0 & USER_BIT != 0
            }

            pub(crate) fn to_usize(self) -> usize {
                (self.0 & INDEX_MASK) as usize
            }
        }
    };
}

impl_id!(FamilyId);
impl_id!(FontId);
impl_id!(SourceId);
