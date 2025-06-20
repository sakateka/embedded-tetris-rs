use std::time::Duration;
use tetris_lib::{common::Timer, games::run_game_menu};

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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable raw terminal mode like machine.py
    enable_raw_mode();

    // Set up Ctrl+C handler to restore terminal
    ctrlc::set_handler(move || {
        restore_terminal();
        println!("\nTerminal restored. Goodbye!");
        std::process::exit(0);
    })?;

    let mut display = SimpleConsoleDisplay;
    let mut controller = SimpleConsoleController::new();
    let timer = ConsoleTimer;

    // Use the extracted game menu loop
    run_game_menu(&mut display, &mut controller, &timer, || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32
    })
    .await;

    Ok(())
}
