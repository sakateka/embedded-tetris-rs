#![no_main]
#![no_std]

mod figure;

use core::ops::Index;

use nrf52833_hal::Rng;
use smart_leds::{colors, RGB8};
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use microbit::{board::Board, hal::Timer};
use rtt_target::{rprintln, rtt_init_print};

use panic_halt as _;

use figure::{Figure, Digits, Tetramino};

const SCREEN_WIDTH: usize = 8;
const SCREEN_HEIGHT: usize = 32;

#[derive(Copy, Clone, Debug)]
enum ColorIdx {
    Brick,
    Red,
    Green,
    Blue,
    Pink,
    Yellow,
    ColorArraySize,
}
const C_IDX_BRICK: usize = ColorIdx::Brick as usize;
const C_IDX_RED: usize = ColorIdx::Red as usize;
const C_IDX_GREEN: usize = ColorIdx::Green as usize;
const C_IDX_BLUE: usize = ColorIdx::Blue as usize;
const C_IDX_PINK: usize = ColorIdx::Pink as usize;
const C_IDX_YELLOW: usize = ColorIdx::Yellow as usize;

impl From<usize> for ColorIdx {
    fn from(value: usize) -> Self {
        match value {
            C_IDX_BRICK => ColorIdx::Brick,
            C_IDX_RED => ColorIdx::Red,
            C_IDX_GREEN => ColorIdx::Green,
            C_IDX_BLUE => ColorIdx::Blue,
            C_IDX_PINK => ColorIdx::Pink,
            C_IDX_YELLOW => ColorIdx::Yellow,
            _ => panic!("out of range {}", value),
        }
    }
}

type ColorsType = [RGB8; ColorIdx::ColorArraySize as usize];

const COLORS: ColorsType = [
    RGB8::new(12, 2, 0),
    RGB8::new(6, 0, 0),
    RGB8::new(0, 6, 0),
    RGB8::new(0, 0, 6),
    RGB8::new(3, 0, 3),
    RGB8::new(6, 6, 0),
];

impl Index<ColorIdx> for ColorsType {
    type Output = RGB8;
    fn index(&self, index: ColorIdx) -> &Self::Output {
        &self[index as usize]
    }
}

trait ColorsIndexer<'a> {
    fn at(&self, idx: usize) -> RGB8;

}

impl<'a> ColorsIndexer<'a> for ColorsType {
    fn at(&self, idx: usize) -> RGB8 {
        if idx >= self.len() {
            return colors::RED; // bright RED indicates an error
        }
        self[idx]
    }
}

type Painter = fn(&mut [RGB8], u8, u8, RGB8) -> bool;

fn dot(m: &mut [RGB8], x: u8, y: u8, color: RGB8) -> bool {
    let mut x = x;
    if y & 1 != 1 {
        x = 7 - x;
    }
    m[SCREEN_WIDTH * y as usize + x as usize] = color;

    true
}

fn draw_figure(
    m: &mut [RGB8],
    f: &Figure,
    x: u8,
    y: u8,
    color: RGB8,
    paniter: Painter,
) -> bool {
    let mut row: u8 = 0;
    let mut col: u8 = 0;
    let mut cursor: u16 = 1 << f.len();
    while cursor != 0 {
        if f.data & cursor != 0 && !paniter(m, x + col, y + row, color) {
            return false;
        }
        col += 1;
        if col == f.width() {
            row += 1;
            col = 0;
        }
        cursor >>= 1;
    }
    true
}

fn clear(m: &mut [RGB8]) {
    (0..m.len()).for_each(|idx| {
        m[idx] = colors::BLACK;
    });
}

include!(concat!(env!("OUT_DIR"), "/figures.rs"));

#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("message from build.rs: {}", DIGITS[8].str());
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let pin = board.edge.e16.degrade();
    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    let mut r = Rng::new(board.RNG);
    rprintln!("created figure(1):\n{}", DIGITS[1].str());

    let x = 3;
    let mut y = 4;

    rprintln!("starting loop");
    let mut digit_idx: u8 = 0;
    loop {
        let color = COLORS.at(r.random_u8() as usize % COLORS.len());
        clear(&mut leds);
        rprintln!("draw at x={} y={} color={:?}", x, y, color);
        digit_idx += 1;
        let digit = DIGITS.wrapping_at(digit_idx);

        _ = draw_figure(&mut leds, digit, x, y, color, dot);
        ws2812.write(leds).unwrap();
        rprintln!("sleep");
        timer.delay_ms(1000);
        y += 1;
        if (y + digit.height()) as usize  > SCREEN_HEIGHT {
            y = 0;
        }
    }
}
