pub mod races;
pub mod snake;
pub mod tanks;
pub mod tetris;

use crate::common::{Game, GameController, LedDisplay, Prng, Timer, FrameBuffer, GREEN_IDX};
use races::RacesGame;
use snake::SnakeGame;
use tanks::TanksGame;
use tetris::TetrisGame;
use smart_leds::RGB8;

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

// Game title graphics (converted from Python GAMES array)
pub const TETRIS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_01110011100111001110010010011100,
    0b_00100010000010001010010010010000,
    0b_00100011100010001110010110010000,
    0b_00100010000010001000011010010000,
    0b_00100011100010001000010010011100,
    0b_00000000000000000000000000000000,
];

pub const RACES_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110011100101001010010010000,
    0b_00001000010100101001100010010000,
    0b_00001000010100111001100010110000,
    0b_00001000010100101001010011010000,
    0b_00001000011100101001010010010000,
    0b_00000000000000000000000000000000,
];

pub const TANKS_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000000000000000,
    0b_00001110001000101001010010010000,
    0b_00000100010100101001100010010000,
    0b_00000100011100111001100010110000,
    0b_00000100010100101001010011010000,
    0b_00000100010100101001010010010000,
    0b_00000000000000000000000000000000,
];

pub const SNAKE_TITLE: [u32; 8] = [
    0b_00000000000000000000000000000000,
    0b_00000000000000000000010000000000,
    0b_01100110110111001000101001001100,
    0b_00010101010100001001101010010010,
    0b_01100100010111001010101100011110,
    0b_00010100010100001100101010010010,
    0b_01100100010111001000101001010010,
    0b_00000000000000000000000000000000,
];

// Game titles array
pub const GAME_TITLES: [&[u32; 8]; 4] = [&TETRIS_TITLE, &SNAKE_TITLE, &TANKS_TITLE, &RACES_TITLE];

/// Run a game menu loop that allows selecting and starting games
pub async fn run_game_menu<D, C, T, F>(
    display: &mut D,
    controller: &mut C,
    timer: &T,
    seed_fn: F,
) where
    D: LedDisplay,
    C: GameController,
    T: Timer,
    F: Fn() -> u32,
{
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];
    let mut game_idx: u8 = 0;

    loop {
        // Read input for menu navigation
        let x_input = controller.read_x().await;

        if x_input != 0 {
            game_idx = game_idx.wrapping_add(x_input as u8) % GAME_TITLES.len() as u8;
        }

        if controller.was_pressed() {
            let seed = seed_fn();
            let prng = Prng::new(seed);
            match game_idx {
                0 => {
                    let mut tetris = TetrisGame::new(prng, display, controller, timer);
                    tetris.run().await;
                }
                1 => {
                    let mut snake = SnakeGame::new(prng, display, controller, timer);
                    snake.run().await;
                }
                2 => {
                    let mut tanks = TanksGame::new(prng, display, controller, timer);
                    tanks.run().await;
                }
                3 => {
                    let mut races = RacesGame::new(prng, display, controller, timer);
                    races.run().await;
                }
                _ => {}
            }
        }

        // Display menu - show game index
        let title = GAME_TITLES[game_idx as usize];
        let screen = FrameBuffer::from_rows(title, GREEN_IDX);
        screen.render(&mut leds);
        display.write(&leds).await;

        timer.sleep_millis(100).await;
    }
}
