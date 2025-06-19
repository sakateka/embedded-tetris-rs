use smart_leds::RGB8;
use std::io::{self, Write};
use tetris_lib::common::{LedDisplay, SCREEN_HEIGHT, SCREEN_WIDTH};

// Simple console display implementation
pub struct SimpleConsoleDisplay {
    first_frame: bool,
}

impl SimpleConsoleDisplay {
    pub fn new() -> Self {
        Self { first_frame: true }
    }

    pub fn reset_frame(&mut self) {
        self.first_frame = true;
    }
}

impl LedDisplay for SimpleConsoleDisplay {
    async fn write(&mut self, leds: &[RGB8]) {
        if !self.first_frame {
            // Move cursor up to overwrite previous frame
            print!("\x1b[{}A", SCREEN_HEIGHT * 2 + 4);
            println!(" {:>1$}\n{:>1$}\n", " ", SCREEN_WIDTH);
        } else {
            self.first_frame = false;
        }

        for y in 0..SCREEN_HEIGHT {
            for _ in 0..2 {
                // Double height for better visibility
                for x in 0..SCREEN_WIDTH {
                    let actual_x = if y % 2 == 0 { SCREEN_WIDTH - 1 - x } else { x };
                    let idx = y * SCREEN_WIDTH + actual_x;

                    if idx < leds.len() {
                        let color = &leds[idx];
                        let red = color.r as u16 * 20;
                        let green = color.g as u16 * 20;
                        let blue = color.b as u16 * 20;
                        print!("\x1b[38;2;{};{};{}m####\x1b[0m", red, green, blue);
                    } else {
                        print!("    ");
                    }
                }
                println!();
            }
        }
        println!("Controls: A/D = change game, Space = start, Q = quit");
        io::stdout().flush().unwrap();
    }
}
