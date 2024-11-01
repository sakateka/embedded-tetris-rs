#![no_main]
#![no_std]

use no_std_strings::str32;
use nrf52833_hal::Rng;
use smart_leds::RGB8;
use smart_leds_trait::SmartLedsWrite;
use ws2812_nrf52833_pwm::Ws2812;

use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use microbit::{board::Board, hal::Timer};
use rtt_target::{rprintln, rtt_init_print};

use panic_halt as _;

struct Figure {
    data: u16,
    wh: u8,
}

impl Figure {
    fn from_str(figure: &'static str) -> Self {
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

    fn width(&self) -> u8 {
        self.wh >> 4
    }

    fn height(&self) -> u8 {
        self.wh & 0x0f
    }

    fn len(&self) -> u8 {
        self.height() * self.width()
    }

    fn str(&self) -> str32 {
        let mut repr = str32::new();
        let mut cursor: u16 = 1 << self.len();
        let mut shift: u8 = 0;
        while cursor != 0 {
            let ch = if self.data & cursor != 0 { '#' } else { ' ' };
            repr.set(shift as usize + cursor.trailing_zeros() as usize, ch);
            if self.len() - cursor.trailing_zeros() as u8 % self.width() == 0 {
                shift += 1;
                repr.set(shift as usize + cursor.trailing_zeros() as usize, '\n');
            }
            cursor >>= 1;
        }
        repr
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let pin = board.edge.e16.degrade();
    let mut ws2812: Ws2812<{ 256 * 24 }, _> = Ws2812::new(board.PWM0, pin);

    let leds = [
        RGB8::new(12, 0, 0),
        RGB8::new(0, 12, 0),
        RGB8::new(0, 0, 12),
        RGB8::new(12, 12, 0),
        RGB8::new(0, 12, 12),
        RGB8::new(12, 0, 12),
        RGB8::new(10, 10, 10),
        RGB8::new(12, 6, 0),
    ];

    rprintln!("starting loop");

    let nleds = leds.len();
    let mut cur_leds: [RGB8; 256] = [RGB8::default(); 256];
    let mut r = Rng::new(board.RNG);
    loop {
        let idx = r.random_u8();
        let color = r.random_u8();
        cur_leds[idx as usize] = leds[color as usize % nleds];
        ws2812.write(cur_leds).unwrap();
        timer.delay_ms(1);
    }
}
