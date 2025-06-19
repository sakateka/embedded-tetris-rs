use core::default::Default;

use crate::figure::Figure;

// Each digit is represented by a 5x3 bit pattern stored in a u16
// The bits are arranged in rows, with each row taking 3 bits
// For example, digit 1:
//  0 1 0  -> 010
//  1 1 0  -> 110
//  0 1 0  -> 010
//  0 1 0  -> 010
//  0 1 0  -> 010
const DIGITS_DATA: [Figure; 10] = [
    // 0: ###
    //    # #
    //    # #
    //    # #
    //    ###
    Figure {
        data: 0b111_101_101_101_111,
        wh: 3 << 4 | 5,
    },
    // 1:  #
    //    ##
    //     #
    //     #
    //     #
    Figure {
        data: 0b010_110_010_010_010,
        wh: 3 << 4 | 5,
    },
    // 2: ###
    //      #
    //    ###
    //    #
    //    ###
    Figure {
        data: 0b111_001_111_100_111,
        wh: 3 << 4 | 5,
    },
    // 3: ###
    //      #
    //    ###
    //      #
    //    ###
    Figure {
        data: 0b111_001_111_001_111,
        wh: 3 << 4 | 5,
    },
    // 4: # #
    //    # #
    //    ###
    //      #
    //      #
    Figure {
        data: 0b101_101_111_001_001,
        wh: 3 << 4 | 5,
    },
    // 5: ###
    //    #
    //    ###
    //      #
    //    ###
    Figure {
        data: 0b111_100_111_001_111,
        wh: 3 << 4 | 5,
    },
    // 6: ###
    //    #
    //    ###
    //    # #
    //    ###
    Figure {
        data: 0b111_100_111_101_111,
        wh: 3 << 4 | 5,
    },
    // 7: ###
    //      #
    //      #
    //      #
    //      #
    Figure {
        data: 0b111_001_001_001_001,
        wh: 3 << 4 | 5,
    },
    // 8: ###
    //    # #
    //    ###
    //    # #
    //    ###
    Figure {
        data: 0b111_101_111_101_111,
        wh: 3 << 4 | 5,
    },
    // 9: ###
    //    # #
    //    ###
    //      #
    //    ###
    Figure {
        data: 0b111_101_111_001_111,
        wh: 3 << 4 | 5,
    },
];

#[derive(Default)]
pub struct Digits([Figure; 10]);

impl Digits {
    pub const fn new(data: [Figure; 10]) -> Self {
        Self(data)
    }

    pub fn wrapping_at(&self, idx: u8) -> &Figure {
        &self.0[idx as usize % 10]
    }
}

impl core::ops::Index<usize> for Digits {
    type Output = Figure;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

pub const DIGITS: Digits = Digits::new(DIGITS_DATA);

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn test_digit_one_pattern() {
        let one = &DIGITS.0[1];
        // Verify the pattern matches the ASCII art
        // First row: single pixel on center
        // data: 0b010_110_010_010_010,
        assert_eq!(one.get_bit(0, 0), false);
        assert_eq!(one.get_bit(1, 0), true);
        assert_eq!(one.get_bit(2, 0), false);

        // Second row: two pixels on left and center
        assert_eq!(one.get_bit(0, 1), true);
        assert_eq!(one.get_bit(1, 1), true);
        assert_eq!(one.get_bit(2, 1), false);

        // Third row: single pixel on center
        assert_eq!(one.get_bit(0, 2), false);
        assert_eq!(one.get_bit(1, 2), true);
        assert_eq!(one.get_bit(2, 2), false);

        // Fourth row: single pixel on center
        assert_eq!(one.get_bit(2, 3), false);
        assert_eq!(one.get_bit(1, 3), true);
        assert_eq!(one.get_bit(0, 3), false);

        // Fifth row: single pixel on center
        assert_eq!(one.get_bit(2, 4), false);
        assert_eq!(one.get_bit(1, 4), true);
        assert_eq!(one.get_bit(0, 4), false);
    }

    #[test]
    fn test_all_digits_dimensions() {
        for (i, digit) in DIGITS.0.iter().enumerate() {
            // All digits should be 3 pixels wide and 5 pixels tall
            assert_eq!(digit.width(), 3, "Digit {} has wrong width", i);
            assert_eq!(digit.height(), 5, "Digit {} has wrong height", i);
        }
    }

    #[test]
    fn test_digit_wrapping() {
        // Test that wrapping works correctly
        assert_eq!(DIGITS.wrapping_at(10), &DIGITS.0[0]);
        assert_eq!(DIGITS.wrapping_at(11), &DIGITS.0[1]);
        assert_eq!(DIGITS.wrapping_at(9), &DIGITS.0[9]);
    }

    #[test]
    fn test_digit_patterns() {
        // Test that each digit has at least some pixels set
        for (i, digit) in DIGITS.0.iter().enumerate() {
            let mut has_pixels = false;
            for y in 0..digit.height() {
                for x in 0..digit.width() {
                    if digit.get_bit(x, y) {
                        has_pixels = true;
                        break;
                    }
                }
            }
            assert!(has_pixels, "Digit {} has no pixels set", i);
        }
    }
}
