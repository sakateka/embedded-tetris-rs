use smart_leds::RGB8;

use crate::{
    common::{
        get_pixel, set_pixel, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer,
        BLACK_IDX, BRICK_IDX, GREEN_IDX, PINK_IDX, SCREEN_HEIGHT, SCREEN_WIDTH, YELLOW_IDX,
    },
    log::{debug, info},
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum GameState {
    Running,
    Paused,
    DrawMode,
}

pub struct LifeGame<'a, D, C, T> {
    screen: FrameBuffer,
    next_screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,
    prng: Prng,
    generation: u32,
    state: GameState,
    pattern_index: usize,
    cursor_x: usize,
    cursor_y: usize,
    blink_counter: u8,
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
            state: GameState::Running,
            pattern_index: 0,
            cursor_x: SCREEN_WIDTH / 2,
            cursor_y: (SCREEN_HEIGHT + 6) / 2, // Start cursor in middle of game area
            blink_counter: 0,
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

        // Show pause indicator or draw mode indicator
        if self.state == GameState::Paused {
            // Draw pause symbol (two vertical lines)
            for y in 1..=3 {
                self.screen.set(2, y, YELLOW_IDX);
                self.screen.set(4, y, YELLOW_IDX);
            }
        } else if self.state == GameState::DrawMode {
            // Draw pencil icon (simple representation)
            self.screen.set(1, 1, PINK_IDX);
            self.screen.set(2, 2, PINK_IDX);
            self.screen.set(3, 3, PINK_IDX);
            self.screen.set(4, 4, PINK_IDX);
        } else {
            // Display pattern index as individual pixels (one pixel per pattern)
            for i in 0..self.pattern_index {
                self.screen
                    .set(i % SCREEN_WIDTH, i / SCREEN_WIDTH, GREEN_IDX);
            }

            let mut available_row = self.pattern_index / SCREEN_WIDTH;
            if self.pattern_index % SCREEN_WIDTH > 0 {
                available_row += 1;
            }
            if available_row < 5 {
                // Show generation progress as pixels on available space
                let gen_progress = ((self.generation / 10) % SCREEN_WIDTH as u32) as usize;
                for i in 0..gen_progress.min(SCREEN_WIDTH) {
                    self.screen.set(i, available_row, YELLOW_IDX);
                }
            }
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

    fn draw_cursor(&mut self, leds: &mut [RGB8; 256]) {
        let color = if self.blink_counter >> 1 > 5 {
            PINK_IDX
        } else {
            get_pixel(leds, self.cursor_x, self.cursor_y)
        };
        set_pixel(leds, self.cursor_x, self.cursor_y, color);
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for LifeGame<'a, D, C, T> {
    async fn run(&mut self) {
        let mut step = 0;
        let round: u8 = 20;
        let mut speed: u8 = 1;
        let delay = 50;

        let mut leds = [RGB8::new(0, 0, 0); 256];
        let mut last_x_input = 0;
        let mut last_y_input = 0;
        let mut cursor_move_counter: u8 = 0;

        loop {
            // Handle input based on current state
            match self.state {
                GameState::Running => {
                    if self.controller.joystick_was_pressed() {
                        self.state = GameState::Paused;
                    }

                    // Enter draw mode with B button in running state
                    if self.controller.b_was_pressed() {
                        self.state = GameState::DrawMode;
                        info!("Entered draw mode");
                    }

                    // Cycle through patterns with A button
                    if self.controller.a_was_pressed() {
                        self.next_pattern();
                    }
                }
                GameState::Paused => {
                    if self.controller.joystick_was_pressed() {
                        self.state = GameState::Running;
                    }

                    // Speed control with A and B buttons
                    if self.controller.a_was_pressed() {
                        speed = speed.saturating_sub(1).clamp(1, 4);
                        info!("Speed has increased to {}", speed);
                    } else if self.controller.b_was_pressed() {
                        speed = speed.saturating_add(1).clamp(1, 4);
                        info!("Speed has dropped to {}", speed);
                    }
                }
                GameState::DrawMode => {
                    // Handle cursor movement with joystick
                    let x_delta = self.controller.read_x().await;
                    let y_delta = self.controller.read_y().await;

                    // Update cursor movement counter
                    cursor_move_counter = cursor_move_counter.wrapping_add(1);

                    // Check if input direction changed (immediate response)
                    let input_changed = x_delta != last_x_input || y_delta != last_y_input;

                    // Allow movement on input change OR every 8 frames for held input
                    let should_move = input_changed || (cursor_move_counter % 8 == 0);

                    if should_move {
                        if x_delta != 0 {
                            let new_x =
                                (self.cursor_x as i8 + x_delta).clamp(0, SCREEN_WIDTH as i8 - 1);
                            self.cursor_x = new_x as usize;
                        }

                        if y_delta != 0 {
                            let new_y =
                                (self.cursor_y as i8 + y_delta).clamp(6, SCREEN_HEIGHT as i8 - 1);
                            self.cursor_y = new_y as usize;
                        }
                    }

                    // Update last input state
                    last_x_input = x_delta;
                    last_y_input = y_delta;

                    // Toggle cell with joystick press
                    if self.controller.joystick_was_pressed() {
                        let current_color = self.screen.get(self.cursor_x, self.cursor_y);
                        if current_color == BLACK_IDX {
                            self.screen.set(self.cursor_x, self.cursor_y, GREEN_IDX);
                        } else {
                            self.screen.set(self.cursor_x, self.cursor_y, BLACK_IDX);
                        }
                    }

                    // Exit draw mode with A or B button
                    if self.controller.a_was_pressed() || self.controller.b_was_pressed() {
                        self.state = GameState::Running;
                        info!("Exited draw mode");
                    }
                }
            }

            // Update generation only when running
            if self.state == GameState::Running && step >= round / speed {
                self.next_generation();
                step = 0;
            }

            // Update blink counter for cursor
            self.blink_counter = (self.blink_counter + 1) % 20; // Blink every 20 frames

            // Draw everything
            self.draw_ui(speed);

            self.screen.render(&mut leds);
            // Draw cursor in draw mode
            if self.state == GameState::DrawMode {
                self.draw_cursor(&mut leds);
            }

            self.display.write(&leds).await;

            step += 1;
            self.timer.sleep_millis(delay as u64).await;
        }
    }
}

// Define Conway patterns showcasing different behaviors
static PATTERNS: &[Option<&[(i8, i8)]>] = &[
    // Random pattern
    None,
    // Glider - moves diagonally
    Some(&[(1, 8), (2, 9), (0, 10), (1, 10), (2, 10)]),
    // Blinker - simple period-2 oscillator
    Some(&[(3, 10), (3, 11), (3, 12)]),
    // Toad - period-2 oscillator
    Some(&[(2, 10), (3, 10), (4, 10), (1, 11), (2, 11), (3, 11)]),
    // Beacon - period-2 oscillator
    Some(&[(1, 8), (2, 8), (1, 9), (4, 10), (3, 11), (4, 11)]),
    // Lightweight Spaceship (LWSS) - travels horizontally
    Some(&[
        (1, 10),
        (4, 10),
        (0, 11),
        (0, 12),
        (0, 13),
        (4, 13),
        (3, 14),
        (2, 14),
        (1, 14),
        (0, 14),
    ]),
    // Pulsar (small version) - period-3 oscillator
    Some(&[(2, 8), (3, 8), (4, 8), (2, 13), (3, 13), (4, 13)]),
    // R-pentomino - famous methuselah (creates chaos then stabilizes)
    Some(&[(3, 10), (4, 10), (2, 11), (3, 11), (3, 12)]),
    // Acorn - methuselah that takes 5206 generations to stabilize
    Some(&[
        (1, 10),
        (3, 11),
        (0, 12),
        (1, 12),
        (4, 12),
        (5, 12),
        (6, 12),
    ]),
    // Diehard - dies after exactly 130 generations
    Some(&[
        (6, 10),
        (0, 11),
        (1, 11),
        (1, 12),
        (5, 12),
        (6, 12),
        (7, 12),
    ]),
    // Clock - period-2 oscillator
    Some(&[(2, 10), (3, 10), (1, 11), (4, 11), (2, 12), (3, 12)]),
    // Penta-decathlon (mini version) - long period oscillator
    Some(&[
        (3, 8),
        (3, 9),
        (2, 10),
        (4, 10),
        (3, 11),
        (3, 12),
        (3, 13),
        (3, 14),
        (2, 15),
        (4, 15),
        (3, 16),
        (3, 17),
    ]),
    // Gosper Glider Gun (tiny version) - creates gliders
    Some(&[
        (0, 10),
        (1, 10),
        (0, 11),
        (1, 11),
        (2, 12),
        (3, 12),
        (4, 12),
        (5, 13),
        (6, 14),
        (7, 14),
    ]),
    // Twin bees shuttle (small version) - period-46 oscillator
    Some(&[
        (1, 10),
        (3, 10),
        (0, 11),
        (4, 11),
        (0, 12),
        (4, 12),
        (1, 13),
        (3, 13),
    ]),
    // Figure eight - period-8 oscillator
    Some(&[
        (1, 10),
        (2, 10),
        (3, 10),
        (0, 11),
        (3, 11),
        (0, 12),
        (3, 12),
        (1, 13),
        (2, 13),
        (3, 13),
    ]),
    // Beehive - still life (more interesting than block)
    Some(&[(2, 10), (3, 10), (1, 11), (4, 11), (2, 12), (3, 12)]),
    // Traffic lights - period-2 oscillator
    Some(&[(1, 9), (2, 9), (1, 10), (4, 11), (5, 11), (4, 12)]),
    // Pentaplex - complex period-2 oscillator
    Some(&[
        (2, 8),
        (3, 8),
        (1, 9),
        (4, 9),
        (0, 10),
        (5, 10),
        (1, 11),
        (4, 11),
        (2, 12),
        (3, 12),
    ]),
    // B-heptomino - interesting methuselah
    Some(&[
        (2, 10),
        (3, 10),
        (1, 11),
        (2, 11),
        (2, 12),
        (3, 12),
        (4, 12),
    ]),
    // Pi-heptomino - creates two gliders
    Some(&[
        (1, 10),
        (2, 10),
        (3, 10),
        (1, 11),
        (3, 11),
        (1, 12),
        (3, 12),
    ]),
    // Galaxy - amazing 4-fold rotational pattern
    Some(&[
        (1, 8),
        (2, 8),
        (1, 9),
        (2, 9),
        (1, 10),
        (2, 10),
        (5, 11),
        (6, 11),
        (5, 12),
        (6, 12),
        (5, 13),
        (6, 13),
    ]),
    // Boat - small still life
    Some(&[(1, 10), (2, 10), (1, 11), (3, 11), (2, 12)]),
    // Loaf - classic still life
    Some(&[(2, 9), (3, 9), (1, 10), (4, 10), (2, 11), (4, 11), (3, 12)]),
    // Hammer - period-14 oscillator
    Some(&[(1, 10), (3, 10), (2, 11), (1, 12), (3, 12)]),
    // Cross - period-3 oscillator
    Some(&[(3, 9), (2, 10), (3, 10), (4, 10), (3, 11)]),
    // Pinwheel - period-4 oscillator
    Some(&[
        (2, 9),
        (3, 9),
        (1, 10),
        (4, 10),
        (1, 11),
        (4, 11),
        (2, 12),
        (3, 12),
    ]),
    // Rabbits - methuselah that creates infinite growth
    Some(&[
        (0, 10),
        (2, 10),
        (3, 11),
        (0, 12),
        (1, 12),
        (3, 12),
        (4, 12),
    ]),
    // Switch engine (small version) - creates infinite growth
    Some(&[
        (1, 10),
        (3, 10),
        (4, 11),
        (1, 12),
        (5, 12),
        (1, 13),
        (2, 13),
        (3, 13),
    ]),
    // Max - methuselah that settles after 312 generations
    Some(&[
        (1, 10),
        (3, 10),
        (0, 11),
        (4, 11),
        (0, 12),
        (1, 12),
        (2, 12),
        (3, 12),
    ]),
    // Octagon II - period-5 oscillator
    Some(&[
        (2, 8),
        (3, 8),
        (1, 9),
        (4, 9),
        (0, 10),
        (5, 10),
        (0, 11),
        (5, 11),
        (1, 12),
        (4, 12),
        (2, 13),
        (3, 13),
    ]),
    // Thunderbird - methuselah
    Some(&[(1, 10), (2, 10), (3, 10), (2, 11), (2, 12), (2, 13)]),
];
