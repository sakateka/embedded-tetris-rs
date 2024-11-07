#![allow(dead_code)]

use core::ops::Index;

use no_std_strings::str32;

#[derive(Default)]
pub struct Figure {
    pub data: u16,
    pub wh: u8,
}

impl Figure {
    pub fn from_str(figure: &str) -> Self {
        let mut data = 0;
        let mut width = 0;
        let mut height = 0;
        for (idx, line) in figure.lines().enumerate() {
            if idx == 0 {
                width = line.len() as u8;
            }
            height += 1;
            for ch in line.chars() {
                data |= if ch == '#' { 1 } else { 0 };
                data <<= 1;
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

    pub fn str(&self) -> str32 {
        let mut repr = str32::new();
        let mut cursor: u16 = 1 << self.len();
        while cursor != 0 {
            let ch = if self.data & cursor != 0 { "#" } else { " " };
            repr.push(ch);
            let row_size = ((self.len() - cursor.trailing_zeros() as u8) % self.width()) + 1;
            if row_size == self.width() {
                repr.push("\n");
            }
            cursor >>= 1;
        }
        repr
    }
}

#[derive(Default)]
pub struct Digits([Figure; 10]);

impl Digits{
    pub const fn new(data: [Figure; 10]) -> Self {
        Digits(data)
    }

    pub fn wrapping_at(&self, idx: u8) -> &Figure {
        let idx: usize = idx as usize % self.0.len();
        &self.0[idx]
    }
}

impl Index<usize> for Digits {
    type Output = Figure;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Default)]
pub struct Tetramino([Figure; 7]);

impl Index<usize> for Tetramino {
    type Output = Figure;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Tetramino {
    pub const fn new(data: [Figure; 7]) -> Self {
        Tetramino(data)
    }

    pub fn wrapping_at(&self, idx: u8) -> &Figure {
        let idx: usize = idx as usize % self.0.len();
        &self.0[idx]
    }
}
