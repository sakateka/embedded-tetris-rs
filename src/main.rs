#![no_std]
#![no_main]

use crate::control::ButtonController;
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler as AdcInterruptHandler;
use embassy_rp::adc::{Adc, Channel, Config};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_time::{Instant, Timer};
use games::snake::SnakeGame;
use games::tetris::TetrisGame;
use smart_leds::RGB8;
use {defmt_rtt as _, panic_probe as _};

mod common;
mod control;
mod digits;
mod figure;
mod games;

use common::*;
use control::{button_task, Joystick};
use games::{races::RacesGame, tanks::TanksGame};

//  Coordinates
//        x
//     0 --->  7
//    0+-------+
//     |       |
//     |   S   |
//   | |   C   |
// y | |   R   |---+
//   | |   E   | +----+
//   v |   E   | |::::| <- microbit
//     |   N   | +----+
//     |       | @ |<---- joystick
//   31+-------+---+

// Implement LedDisplay directly for PioWs2812
impl LedDisplay for PioWs2812<'_, PIO0, 0, 256> {
    async fn write(&mut self, leds: &[RGB8]) {
        // Convert slice to fixed array for ws2812
        if leds.len() >= 256 {
            let array: &[RGB8; 256] = &leds[..256].try_into().unwrap();
            self.write(array).await;
        }
    }
}

// Game title graphics (converted from Python GAMES array)
const TETRIS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_01110011100111001110010010011100,
    0b_00100010000010001010010010010000,
    0b_00100011100010001110010110010000,
    0b_00100010000010001000011010010000,
    0b_00100011100010001000010010011100,
    0b_00000000000000000000000000000000,
];

const RACES_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110011100101001010010010000,
    0b_00001000010100101001100010010000,
    0b_00001000010100111001100010110000,
    0b_00001000010100101001010011010000,
    0b_00001000011100101001010010010000,
    0b_00000000000000000000000000000000,
];

const TANKS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110001000101001010010010000,
    0b_00000100010100101001100010010000,
    0b_00000100011100111001100010110000,
    0b_00000100010100101001010011010000,
    0b_00000100010100101001010010010000,
    0b_00000000000000000000000000000000,
];

const SNAKE_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000010000000000000,
    0b_01100110110111001000101001001100,
    0b_00010101010100001001101010010010,
    0b_01100100010111001010101100011110,
    0b_00010100010100001100101010010010,
    0b_01100100010111001000101001010010,
    0b_00000000000000000000000000000000,
];

// Game titles array
const GAME_TITLES: [&[u32; 8]; 4] = [&TETRIS_TITLE, &SNAKE_TITLE, &TANKS_TITLE, &RACES_TITLE];

// Function to create FrameBuffer from title rows (like Python's from_rows)
fn framebuffer_from_rows(rows: &[u32; 8], color: u8) -> FrameBuffer {
    let mut buffer = FrameBuffer::new();

    for y in 0..SCREEN_HEIGHT {
        for (x, row) in rows.iter().enumerate() {
            if x < SCREEN_WIDTH {
                let bit = row >> (SCREEN_HEIGHT - y - 1) & 1;
                if bit == 1 {
                    buffer.set(SCREEN_WIDTH - x - 1, y, color);
                }
            }
        }
    }

    buffer
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting game collection!");

    bind_interrupts!(struct Irqs {
        PIO0_IRQ_0 => InterruptHandler<PIO0>;
        ADC_IRQ_FIFO => AdcInterruptHandler;
    });

    let p = embassy_rp::init(Default::default());
    // Initialize PIO for WS2812
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let program = PioWs2812Program::new(&mut common);
    let mut ws2812 = PioWs2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_13, &program);

    // Initialize ADC for joystick input (Pins 27 and 28)
    let adc_reader = Adc::new(p.ADC, Irqs, Config::default());
    let adc_pin_x = Channel::new_pin(p.PIN_27, Pull::None);
    let adc_pin_y = Channel::new_pin(p.PIN_28, Pull::None);
    // Initialize button (Pin 16)
    let button_pin = Input::new(p.PIN_16, Pull::Up);
    let button_controller = ButtonController::new(button_pin);

    // Spawn button task
    spawner.spawn(button_task(button_controller)).unwrap();

    let mut joystick = Joystick::new(adc_reader, adc_pin_x, adc_pin_y);
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];

    // Game menu
    let mut game_idx: u8 = 0;

    info!("Starting main menu loop");
    loop {
        // Read joystick for menu navigation
        let x_input = joystick.read_x().await;

        if x_input != 0 {
            game_idx = game_idx.wrapping_add(x_input as u8) % GAME_TITLES.len() as u8;
            Timer::after_millis(200).await; // Debounce
        }

        if joystick.was_pressed() {
            let seed = Instant::now().as_ticks() as u32;
            let prng = common::Prng::new(seed);
            match game_idx {
                0 => {
                    info!("Starting Tetris");
                    let mut tetris = TetrisGame::new(prng);
                    tetris.run(&mut ws2812, &mut joystick).await;
                }
                1 => {
                    info!("Starting Snake");
                    let mut snake = SnakeGame::new(prng);
                    snake.run(&mut ws2812, &mut joystick).await;
                }
                2 => {
                    info!("Starting Tanks");
                    let mut tanks = TanksGame::new(prng);
                    tanks.run(&mut ws2812, &mut joystick).await;
                }
                3 => {
                    info!("Starting Races");
                    let mut races = RacesGame::new(prng);
                    races.run(&mut ws2812, &mut joystick).await;
                }
                _ => {}
            }
        }

        // Display menu - show game index
        leds.fill(BLACK);
        let title = GAME_TITLES[game_idx as usize % GAME_TITLES.len()];
        let screen = framebuffer_from_rows(title, GREEN_IDX);
        screen.render(&mut leds);
        ws2812.write(&leds).await;

        // Phantom press detected due to ws2812 write ???
        _ = joystick.was_pressed();

        Timer::after_millis(100).await;
    }
}
