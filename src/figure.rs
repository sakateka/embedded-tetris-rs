#![cfg_attr(not(test), allow(dead_code))]

use no_std_strings::str32;
use smart_leds::RGB8;

type Painter = fn(&mut [RGB8], u8, u8, RGB8) -> bool;

#[derive(Default, Copy, Clone, PartialEq, Debug)]
pub struct Figure {
    pub data: u16,
    pub wh: u8,
}

impl Figure {
    pub fn from_str(figure: &str) -> Self {
        let mut data = 0;
        let mut width = 0;
        let mut height = 0;
        let mut cursor = 1;
        for (idx, line) in figure.lines().enumerate() {
            if idx == 0 {
                width = line.len() as u8;
            }
            height += 1;
            for ch in line.chars() {
                if ch == '#' {
                    data |= cursor;
                }
                cursor <<= 1;
            }
        }
        Self {
            data,
            wh: width << 4 | height,
        }
    }

    pub fn width(&self) -> u8 {
        self.wh >> 4
    }

    pub fn height(&self) -> u8 {
        self.wh & 0x0f
    }

    pub fn len(&self) -> u8 {
        self.height() * self.width()
    }

    pub fn rotate(&self) -> Self {
        let mut rotated: u16 = 0;
        let height = self.height();
        let width = self.width();

        let mut data: u16 = self.data;
        let mut row = 0;
        let mut ch = height - 1;
        while data != 0 {
            let new_ch_idx = row * height + ch;
            if data & 1 == 1 {
                rotated |= 1 << new_ch_idx;
            }
            row += 1;
            if row == width {
                row = 0;
                ch = ch.saturating_sub(1);
            }
            data >>= 1;
        }

        Self {
            data: rotated,
            wh: height << 4 | width, // flip
        }
    }

    pub fn str(&self) -> str32 {
        let mut repr = str32::new();
        let mut cursor: u16 = 1;

        let mut ch_idx = 0;
        let signs = self.width() * self.height();
        while cursor.trailing_zeros() as u8 != signs {
            let ch = if self.data & cursor != 0 { "#" } else { " " };
            repr.push(ch);

            ch_idx += 1;
            if ch_idx == self.width() {
                repr.push("\n");
                ch_idx = 0;
            }
            cursor <<= 1;
        }
        repr
    }

    pub fn get_bit(&self, col: u8, row: u8) -> bool {
        if col >= self.width() || row >= self.height() {
            return false;
        }
        let bit_idx = (self.height() - 1 - row) * self.width() + (self.width() - 1 - col); // Flip both row and column order
        let cursor = 1u16 << bit_idx;
        self.data & cursor != 0
    }

    pub fn draw(&self, m: &mut [RGB8], x: u8, y: u8, color: RGB8, paniter: Painter) -> bool {
        let mut row: u8 = 0;
        let mut col: u8 = 0;
        let mut cursor: u16 = 1;
        let signs = self.width() * self.height();
        while cursor.trailing_zeros() as u8 != signs {
            if self.data & cursor != 0 && !paniter(m, x + col, y + row, color) {
                return false;
            }
            col += 1;
            if col == self.width() {
                row += 1;
                col = 0;
            }
            cursor <<= 1;
        }
        true
    }
}

#[derive(Default)]
pub struct Digits([Figure; 10]);

impl Digits {
    pub const fn new(data: [Figure; 10]) -> Self {
        Digits(data)
    }

    pub fn wrapping_at(&self, idx: u8) -> Figure {
        let idx: usize = idx as usize % self.0.len();
        self.0[idx]
    }
}

#[derive(Default)]
pub struct Tetramino([Figure; 7]);

impl Tetramino {
    pub const fn new(data: [Figure; 7]) -> Self {
        Tetramino(data)
    }

    pub fn wrapping_at(&self, idx: u8) -> Figure {
        let idx: usize = idx as usize % self.0.len();
        self.0[idx]
    }
}

// Standard Tetris tetraminoes (I, O, T, S, Z, J, L)
pub const TETRAMINO: Tetramino = Tetramino::new([
    // I: ####
    Figure {
        data: 0b1111,
        wh: 4 << 4 | 1,
    },
    // O: ##
    //    ##
    Figure {
        data: 0b11_11,
        wh: 2 << 4 | 2,
    },
    // T: ###
    //      #
    Figure {
        data: 0b111_010,
        wh: 3 << 4 | 2,
    },
    // S:  ##
    //    ##
    Figure {
        data: 0b011_110,
        wh: 3 << 4 | 2,
    },
    // Z: ##
    //     ##
    Figure {
        data: 0b110_011,
        wh: 3 << 4 | 2,
    },
    // J: #
    //   ###
    Figure {
        data: 0b100_111,
        wh: 3 << 4 | 2,
    },
    // L:   #
    //     ###
    Figure {
        data: 0b001_111,
        wh: 3 << 4 | 2,
    },
]);

#[cfg(test)]
mod tests {
    use super::*;
    const FOUR: &str = r#"
# #
# #
###
  #
  #
"#;

    const ROTATED_FOUR: &str = r#"
  ###
  #  
#####
"#;

    #[test]
    fn parse() {
        let text = FOUR.trim_start_matches('\n');
        let f = Figure::from_str(text.trim());
        assert_eq!(f.str(), text);
    }

    #[test]
    fn rotated() {
        let text = ROTATED_FOUR.trim_start_matches('\n');
        let f = Figure::from_str(FOUR.trim());
        let rotated = f.rotate();
        assert_eq!(
            rotated.str(),
            text,
            "'\n{}'\n!='\n{}'\n",
            rotated.str(),
            text
        );
    }
}
#[cfg(test)]
fn main() {}
