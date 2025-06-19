pub mod races;
pub mod snake;
pub mod tanks;
pub mod tetris;

use crate::common::{Game, GameController, LedDisplay, Prng, Timer};
use races::RacesGame;
use snake::SnakeGame;
use tanks::TanksGame;
use tetris::TetrisGame;

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

/// Run the selected game based on game index
pub async fn run_game<D, C, T>(
    game_idx: u8,
    prng: Prng,
    display: &mut D,
    controller: &mut C,
    timer: &T,
) where
    D: LedDisplay,
    C: GameController,
    T: Timer,
{
    match game_idx {
        0 => {
            let mut tetris = TetrisGame::new(prng);
            tetris.run(display, controller, timer).await;
        }
        1 => {
            let mut snake = SnakeGame::new(prng);
            snake.run(display, controller, timer).await;
        }
        2 => {
            let mut tanks = TanksGame::new(prng);
            tanks.run(display, controller, timer).await;
        }
        3 => {
            let mut races = RacesGame::new(prng);
            races.run(display, controller, timer).await;
        }
        _ => {}
    }
}
