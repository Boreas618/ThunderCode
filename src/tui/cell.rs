//! Packed cell representation for terminal screen buffers.
//!
//! Each cell occupies 2 contiguous `u32` values in a flat array:
//! - word0: `char_id` (full 32 bits, index into [`CharPool`](crate::tui::pools::CharPool))
//! - word1: `style_id[31:17] | hyperlink_id[16:2] | width[1:0]`
//!
//! This layout eliminates per-cell heap objects and enables fast integer
//! comparisons for screen diffing.

/// Bit-shift for style_id in word1.
pub const STYLE_SHIFT: u32 = 17;

/// Bit-shift for hyperlink_id in word1.
pub const HYPERLINK_SHIFT: u32 = 2;

/// Bitmask for hyperlink_id (15 bits).
pub const HYPERLINK_MASK: u32 = 0x7fff;

/// Bitmask for width (2 bits).
pub const WIDTH_MASK: u32 = 3;

/// Well-known char pool indices.
pub const EMPTY_CHAR_INDEX: u32 = 0; // ' ' (space)
pub const SPACER_CHAR_INDEX: u32 = 1; // '' (empty string for spacer cells)

/// Cell width classification for handling double-wide characters (CJK, emoji).
///
/// We use explicit spacer cells rather than inferring width at render time.
/// This makes the data structure self-describing and simplifies cursor
/// positioning logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CellWidth {
    /// Not a wide character, cell width 1.
    Narrow = 0,
    /// Wide character, cell width 2. This cell contains the actual character.
    Wide = 1,
    /// Spacer occupying the second visual column of a wide character. Do not render.
    SpacerTail = 2,
    /// Spacer at the end of a soft-wrapped line indicating that a wide character
    /// continues on the next line.
    SpacerHead = 3,
}

impl CellWidth {
    /// Convert from the 2-bit packed representation.
    #[inline]
    pub fn from_u32(v: u32) -> Self {
        match v & WIDTH_MASK {
            0 => CellWidth::Narrow,
            1 => CellWidth::Wide,
            2 => CellWidth::SpacerTail,
            3 => CellWidth::SpacerHead,
            _ => unreachable!(),
        }
    }
}

/// Unpacked cell view. Returned by accessors -- a fresh struct each call
/// since cells are stored packed, not as objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub char: String,
    pub style_id: u32,
    pub width: CellWidth,
    pub hyperlink: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: " ".into(),
            style_id: 0,
            width: CellWidth::Narrow,
            hyperlink: None,
        }
    }
}

/// Packed cell stored as 2x u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedCell {
    pub words: [u32; 2],
}

impl PackedCell {
    /// Create a packed cell from an empty/space cell.
    pub const EMPTY: Self = Self { words: [0, 0] };

    /// Create a new packed cell.
    #[inline]
    pub fn new(char_id: u32, style_id: u32, hyperlink_id: u32, width: CellWidth) -> Self {
        Self {
            words: [char_id, pack_word1(style_id, hyperlink_id, width as u32)],
        }
    }

    /// Get the char_id (word0).
    #[inline]
    pub fn char_id(&self) -> u32 {
        self.words[0]
    }

    /// Get the style_id from word1.
    #[inline]
    pub fn style_id(&self) -> u32 {
        self.words[1] >> STYLE_SHIFT
    }

    /// Get the hyperlink_id from word1.
    #[inline]
    pub fn hyperlink_id(&self) -> u32 {
        (self.words[1] >> HYPERLINK_SHIFT) & HYPERLINK_MASK
    }

    /// Get the cell width from word1.
    #[inline]
    pub fn width(&self) -> CellWidth {
        CellWidth::from_u32(self.words[1])
    }

    /// Check if this is an empty/unwritten cell (both words are 0).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words[0] == 0 && self.words[1] == 0
    }
}

/// Pack style_id, hyperlink_id, and width into a single u32 (word1).
#[inline]
pub fn pack_word1(style_id: u32, hyperlink_id: u32, width: u32) -> u32 {
    (style_id << STYLE_SHIFT) | (hyperlink_id << HYPERLINK_SHIFT) | width
}

/// Unpack style_id from word1.
#[inline]
pub fn unpack_style_id(word1: u32) -> u32 {
    word1 >> STYLE_SHIFT
}

/// Unpack hyperlink_id from word1.
#[inline]
pub fn unpack_hyperlink_id(word1: u32) -> u32 {
    (word1 >> HYPERLINK_SHIFT) & HYPERLINK_MASK
}

/// Unpack width from word1.
#[inline]
pub fn unpack_width(word1: u32) -> CellWidth {
    CellWidth::from_u32(word1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let style_id = 42u32;
        let hyperlink_id = 7u32;
        let width = CellWidth::Wide;

        let packed = PackedCell::new(100, style_id, hyperlink_id, width);

        assert_eq!(packed.char_id(), 100);
        assert_eq!(packed.style_id(), style_id);
        assert_eq!(packed.hyperlink_id(), hyperlink_id);
        assert_eq!(packed.width(), CellWidth::Wide);
    }

    #[test]
    fn test_empty_cell() {
        let cell = PackedCell::EMPTY;
        assert!(cell.is_empty());
        assert_eq!(cell.char_id(), EMPTY_CHAR_INDEX);
        assert_eq!(cell.style_id(), 0);
        assert_eq!(cell.hyperlink_id(), 0);
        assert_eq!(cell.width(), CellWidth::Narrow);
    }

    #[test]
    fn test_pack_word1_bitfields() {
        // Max style_id that fits in 15 bits (32-17=15)
        let style_id = (1u32 << 15) - 1; // 32767
        let hyperlink_id = (1u32 << 15) - 1; // 32767
        let width = CellWidth::SpacerHead as u32; // 3

        let word1 = pack_word1(style_id, hyperlink_id, width);
        assert_eq!(unpack_style_id(word1), style_id);
        assert_eq!(unpack_hyperlink_id(word1), hyperlink_id);
        assert_eq!(unpack_width(word1), CellWidth::SpacerHead);
    }

    #[test]
    fn test_style_id_bit0_visibility_flag() {
        // In the ref, bit 0 of style_id encodes whether the style has a
        // visible effect on space characters. Even IDs = fg-only; odd = visible.
        // This test verifies that packing preserves even/odd distinction.
        let even_style = 4u32; // fg-only
        let odd_style = 5u32; // visible on spaces (bg, inverse, underline, etc.)

        let w1_even = pack_word1(even_style, 0, 0);
        let w1_odd = pack_word1(odd_style, 0, 0);

        assert_eq!(unpack_style_id(w1_even) & 1, 0);
        assert_eq!(unpack_style_id(w1_odd) & 1, 1);
    }

    #[test]
    fn test_cell_width_from_u32() {
        assert_eq!(CellWidth::from_u32(0), CellWidth::Narrow);
        assert_eq!(CellWidth::from_u32(1), CellWidth::Wide);
        assert_eq!(CellWidth::from_u32(2), CellWidth::SpacerTail);
        assert_eq!(CellWidth::from_u32(3), CellWidth::SpacerHead);
        // Mask ensures only bottom 2 bits matter
        assert_eq!(CellWidth::from_u32(4), CellWidth::Narrow);
        assert_eq!(CellWidth::from_u32(7), CellWidth::SpacerHead);
    }
}
