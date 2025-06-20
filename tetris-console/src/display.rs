use smart_leds::RGB8;
use std::io::{self, Write};
use tetris_lib::common::{LedDisplay, SCREEN_HEIGHT, SCREEN_WIDTH};

// Simple console display implementation
pub struct SimpleConsoleDisplay;

impl LedDisplay for SimpleConsoleDisplay {
    async fn write(&mut self, leds: &[RGB8; 256]) {
        // Move cursor up to overwrite previous frame
        let _ = io::stdout().write_all(format!("\x1b[{}A", SCREEN_HEIGHT * 2 + 1).as_bytes());

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
                        let _ = io::stdout().write_all(
                            format!("\x1b[38;2;{};{};{}m####\x1b[0m", red, green, blue).as_bytes(),
                        );
                    } else {
                        let _ = io::stdout().write_all(b"    ");
                    }
                }
                let _ = io::stdout().write_all(b"\n");
            }
        }
        let _ = io::stdout().write_all(b"Controls: A/D = change game, Space = start, Q = quit\n");
        let _ = io::stdout().flush();
    }
}
