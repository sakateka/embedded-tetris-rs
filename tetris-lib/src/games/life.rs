use smart_leds::RGB8;

use crate::{
    common::{
        FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, BLACK_IDX, BRICK_IDX,
        GREEN_IDX, PINK_IDX, SCREEN_HEIGHT, SCREEN_WIDTH, YELLOW_IDX,
    },
    digits::DIGITS,
    log::{debug, info},
};

pub struct LifeGame<'a, D, C, T> {
    screen: FrameBuffer,
    next_screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,
    prng: Prng,
    generation: u32,
    paused: bool,
    pattern_index: usize,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> LifeGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        let mut game = Self {
            screen: FrameBuffer::new(),
            next_screen: FrameBuffer::new(),
            display,
            controller,
            timer,
            prng,
            generation: 0,
            paused: false,
            pattern_index: 0,
        };

        game.set_pattern();
        game
    }

    fn set_pattern(&mut self) {
        self.screen.clear();
        self.generation = 0;

        let current_pattern = PATTERNS[self.pattern_index];
        if let Some(pattern) = current_pattern {
            // Predefined pattern
            info!("Setting predefined pattern {}", self.pattern_index);
            for &(x, y) in pattern {
                if x >= 0 && x < SCREEN_WIDTH as i8 && y >= 6 && y < SCREEN_HEIGHT as i8 {
                    self.screen.set(x as usize, y as usize, GREEN_IDX);
                }
            }
        } else {
            // Random pattern
            info!("Setting random pattern");
            for x in 0..SCREEN_WIDTH {
                for y in 6..SCREEN_HEIGHT {
                    // Skip top area for UI
                    if self.prng.next_range(4) == 0 {
                        // 25% chance of being alive
                        self.screen.set(x, y, GREEN_IDX);
                    }
                }
            }
        }
    }

    fn next_pattern(&mut self) {
        self.pattern_index = (self.pattern_index + 1) % PATTERNS.len();
        debug!("Switching to pattern {}", self.pattern_index);
        self.set_pattern();
    }

    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue; // Skip the cell itself
                }

                let nx = x as i8 + dx;
                let ny = y as i8 + dy;

                // Handle wrapping at screen boundaries
                let nx = if nx < 0 {
                    SCREEN_WIDTH as i8 - 1
                } else if nx >= SCREEN_WIDTH as i8 {
                    0
                } else {
                    nx
                };

                let ny = if ny < 6 {
                    // Don't wrap vertically into UI area
                    continue;
                } else if ny >= SCREEN_HEIGHT as i8 {
                    6
                } else {
                    ny
                };

                if self.screen.get(nx as usize, ny as usize) != BLACK_IDX {
                    count += 1;
                }
            }
        }
        count
    }

    fn next_generation(&mut self) {
        self.next_screen.clear();

        // Copy UI area
        for x in 0..SCREEN_WIDTH {
            for y in 0..6 {
                let color = self.screen.get(x, y);
                self.next_screen.set(x, y, color);
            }
        }

        // Apply Conway's rules to game area
        let mut alive_count = 0;
        for x in 0..SCREEN_WIDTH {
            for y in 6..SCREEN_HEIGHT {
                let neighbors = self.count_neighbors(x, y);
                let is_alive = self.screen.get(x, y) != BLACK_IDX;

                // Conway's Game of Life rules:
                // 1. Live cell with 2-3 neighbors survives
                let stays_alive = is_alive && (neighbors == 2 || neighbors == 3);
                // 2. Dead cell with exactly 3 neighbors becomes alive
                let reborns = !is_alive && neighbors == 3;
                // 3. All other cells die or stay dead
                if stays_alive || reborns {
                    self.next_screen.set(x, y, GREEN_IDX);
                    alive_count += 1;
                } // else: cell dies or stays dead (already cleared)
            }
        }

        // Swap buffers
        core::mem::swap(&mut self.screen, &mut self.next_screen);
        self.generation += 1;

        if self.generation % 50 == 0 {
            debug!(
                "Generation {}, alive cells: {}",
                self.generation, alive_count
            );
        }
    }

    fn draw_ui(&mut self, speed: u8) {
        // Clear score area
        for x in 0..SCREEN_WIDTH {
            for y in 0..5 {
                self.screen.set(x, y, BLACK_IDX);
            }
        }

        // Show pause indicator
        if self.paused {
            // Draw pause symbol (two vertical lines)
            for y in 1..=3 {
                self.screen.set(2, y, YELLOW_IDX);
                self.screen.set(4, y, YELLOW_IDX);
            }
        } else {
            // Display pattern number (0-5)
            let pattern_digit = self.pattern_index % 10;
            self.screen
                .draw_figure(0, 0, &DIGITS[pattern_digit], GREEN_IDX);

            // Show generation indicator (simplified)
            let gen_indicator = ((self.generation / 10) % 10) as usize; // Show progress as single digit
            self.screen
                .draw_figure(4, 0, &DIGITS[gen_indicator], GREEN_IDX);
        }

        // Draw horizontal line
        for x in 0..SCREEN_WIDTH {
            self.screen.set(x, 5, PINK_IDX);
        }
        for x in 0..(speed * 2) {
            if x & 1 == 1 {
                continue;
            }
            self.screen.set(x.into(), 5, BRICK_IDX);
        }
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for LifeGame<'a, D, C, T> {
    async fn run(&mut self) {
        let mut step = 0;
        let round: u8 = 20;
        let mut speed: u8 = 1;
        let delay = 50;

        let mut leds = [RGB8::new(0, 0, 0); 256];

        loop {
            // Handle input
            if self.controller.joystick_was_pressed() {
                self.paused = !self.paused;
            }

            // Cycle through patterns with X axis
            if !self.paused && self.controller.a_was_pressed() {
                self.next_pattern();
                self.paused = false;
            }

            // Speed control with joystick Y
            if self.paused {
                if self.controller.a_was_pressed() {
                    speed = speed.saturating_sub(1).clamp(1, 4);
                    info!("Speed has increased to {}", speed);
                } else if self.controller.b_was_pressed() {
                    speed = speed.saturating_add(1).clamp(1, 4);
                    info!("Speed has dropped to {}", speed);
                }
            }

            // Update generation
            if !self.paused && step >= round / speed {
                self.next_generation();
                step = 0;
            }

            // Draw everything
            self.draw_ui(speed);
            self.screen.render(&mut leds);
            self.display.write(&leds).await;

            step += 1;
            self.timer.sleep_millis(delay as u64).await;
        }
    }
}

// Define some classic Conway patterns
static PATTERNS: &[Option<&[(i8, i8)]>] = &[
    // Random pattern
    None,
    // Glider
    Some(&[(1, 8), (2, 9), (0, 10), (1, 10), (2, 10)]),
    // Blinker
    Some(&[(3, 10), (3, 11), (3, 12)]),
    // Block
    Some(&[(3, 10), (4, 10), (3, 11), (4, 11)]),
    // Toad
    Some(&[(2, 10), (3, 10), (4, 10), (1, 11), (2, 11), (3, 11)]),
    // Beacon
    Some(&[(1, 8), (2, 8), (1, 9), (4, 10), (3, 11), (4, 11)]),
];
