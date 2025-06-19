use smart_leds::RGB8;
use std::time::Duration;
use tetris_lib::{
    common::{FrameBuffer, GameController, LedDisplay, Prng, Timer},
    games::{run_game, GAME_TITLES},
};

mod control;
mod display;

use control::{enable_raw_mode, restore_terminal, SimpleConsoleController};
use display::SimpleConsoleDisplay;

// Console timer implementation
pub struct ConsoleTimer;

impl Timer for ConsoleTimer {
    async fn sleep_millis(&self, millis: u64) {
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable raw terminal mode like machine.py
    enable_raw_mode();

    // Set up Ctrl+C handler to restore terminal
    ctrlc::set_handler(move || {
        restore_terminal();
        println!("\nTerminal restored. Goodbye!");
        std::process::exit(0);
    })?;

    let mut display = SimpleConsoleDisplay::new();
    let mut controller = SimpleConsoleController::new();
    let mut leds: [RGB8; 256] = [RGB8::default(); 256];
    let timer = ConsoleTimer;

    // Game menu
    let mut game_idx: u8 = 0;

    loop {
        // Reset button state (handled by was_pressed() method)

        // Read input for menu navigation
        let x_input = controller.read_x().await;

        if x_input != 0 {
            game_idx = game_idx.wrapping_add(x_input as u8) % GAME_TITLES.len() as u8;
            println!("Selected game: {}", game_idx);
            tokio::time::sleep(Duration::from_millis(100)).await; // Minimal debounce
        }

        if controller.was_pressed() {
            println!("Starting game {}...", game_idx);
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u32;
            let prng = Prng::new(seed);

            run_game(game_idx, prng, &mut display, &mut controller, &timer).await;

            // Reset for menu
            display.reset_frame();
            println!("\nBack to menu - Controls: A/D = change game, Space = start, Q = quit");
        }

        // Display menu - show game index
        leds.fill(tetris_lib::common::BLACK);
        let title = GAME_TITLES[game_idx as usize % GAME_TITLES.len()];
        let screen = FrameBuffer::from_rows(title, tetris_lib::common::GREEN_IDX);
        screen.render(&mut leds);
        display.write(&leds).await;

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
