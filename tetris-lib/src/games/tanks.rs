use core::default::Default;
use core::ops::Fn;
use smart_leds::RGB8;

use crate::common::{
    Dot, FrameBuffer, Game, GameController, LedDisplay, Prng, Timer, COLORS, GREEN_IDX, PINK_IDX,
    RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH,
};

use crate::digits::DIGITS;
use crate::figure::{Figure, TANK};

#[derive(Clone, Copy)]
struct Missile {
    x: i8,
    y: i8,
    dx: i8,
    dy: i8,
}

impl Missile {
    fn new(x: i8, y: i8, dx: i8, dy: i8) -> Self {
        Self { x, y, dx, dy }
    }

    fn move_(&mut self) {
        self.x += self.dx;
        self.y += self.dy;
    }

    fn visible(&self) -> bool {
        self.x >= 0 && self.x < 8 && self.y >= 0 && self.y < 32
    }

    fn hide(&mut self) {
        self.x = -1;
        self.y = -1;
    }
}

#[derive(Clone, Copy)]
struct Tank {
    missiles: [Missile; 2],
    pos: Dot,
    origin: i8,
    rotation: u8,
    rotations: [Dot; 4],
    figure: Figure,
    lives: i8,
}

impl Tank {
    pub fn new(pos: Dot, origin: i8) -> Self {
        Self {
            missiles: [Missile::new(-1, -1, 0, 0); 2],
            pos,
            origin,
            rotation: 2,
            rotations: [
                Dot::new(-1, 0),
                Dot::new(0, -1),
                Dot::new(1, 0),
                Dot::new(0, 1),
            ],
            figure: TANK,
            lives: 1,
        }
    }

    fn player(&self) -> bool {
        self.origin == -1
    }

    fn direction(&self) -> Dot {
        self.rotations[self.rotation as usize % self.rotations.len()]
    }

    fn rotate(&mut self, direction: &Dot) -> bool {
        if direction.is_zero() {
            self.figure = self.figure.rotate();
            self.rotation = (self.rotation + 1) % self.rotations.len() as u8;
            return true;
        }

        let mut rotated = 0;
        while self.direction() != *direction {
            self.figure = self.figure.rotate();
            self.rotation = (self.rotation + 1) % self.rotations.len() as u8;
            rotated += 1;
            if rotated == self.rotations.len() {
                return false;
            }
        }
        rotated > 0
    }

    fn move_(
        &mut self,
        direction: &Dot,
        collides: impl Fn(i8, i8, &Figure) -> bool,
        allow_backward: bool,
    ) {
        if !direction.is_zero()
            && ((allow_backward && self.direction().is_opposite(direction))
                || !self.rotate(direction))
        {
            let pos = self.pos.move_by(*direction);
            if !collides(pos.x, pos.y, &self.figure) {
                self.pos = pos;
            }
        }
    }

    fn move_forward(&mut self) {
        let direction = self.direction();
        self.pos = self.pos.move_by(direction);
    }

    fn fire(&mut self) {
        let direction = self.direction();
        for m in &mut self.missiles {
            if !m.visible() {
                m.x = self.pos.x + 1 + direction.x;
                m.y = self.pos.y + 1 + direction.y;

                if direction.x < 0 {
                    m.x -= 1;
                }
                if direction.y < 0 {
                    m.y -= 1;
                }

                m.dx = direction.x;
                m.dy = direction.y;
                break;
            }
        }
    }

    fn move_missiles(&mut self) {
        for m in &mut self.missiles {
            if m.visible() {
                m.move_();
                if !m.visible() {
                    m.hide();
                }
            }
        }
    }

    fn collides(&self, pos: Dot) -> bool {
        pos.x >= self.pos.x
            && pos.x < self.pos.x + 2
            && pos.y >= self.pos.y
            && pos.y < self.pos.y + 2
    }

    fn hit(&mut self) {
        self.lives -= 1;
    }

    fn is_dead(&self) -> bool {
        self.lives <= 0
    }

    fn is_dying(&self) -> bool {
        self.lives < 0
    }
}

// Tanks game implementation
pub struct TanksGame<'a, D, C, T> {
    screen: FrameBuffer,
    display: &'a mut D,
    controller: &'a mut C,
    timer: &'a T,

    tank: Tank,
    enemies: [Tank; 4],
    enemy_count: usize,
    score: u8,
    lives: i8,
    prng: Prng,
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> TanksGame<'a, D, C, T> {
    pub fn new(prng: Prng, display: &'a mut D, controller: &'a mut C, timer: &'a T) -> Self {
        Self {
            screen: FrameBuffer::new(),
            display,
            controller,
            timer,

            tank: Tank::new(Dot::new(3, 16), -1),
            enemies: [Tank::new(Dot::new(0, 0), 0); 4],
            enemy_count: 0,
            score: 0,
            lives: 3,
            prng,
        }
    }

    fn ai(&mut self) {
        // First priority: spawn tanks if we have less than 2
        if self.enemy_count < 2 && self.enemy_count < self.enemies.len() {
            let spawns = [
                Dot::new(0, 6),
                Dot::new(5, 6),
                Dot::new(0, 29),
                Dot::new(5, 29),
            ];

            // Find an available spawn point
            for (idx, &spawn_pos) in spawns.iter().enumerate() {
                if !self.enemies[..self.enemy_count]
                    .iter()
                    .any(|e| e.origin == idx as i8)
                {
                    self.enemies[self.enemy_count] = Tank::new(spawn_pos, idx as i8);
                    self.enemy_count += 1;
                    break;
                }
            }
        }

        // Process each tank AI behavior
        for i in 0..self.enemy_count {
            if self.enemies[i].is_dying() {
                continue;
            }

            let mut enemy = self.enemies[i];

            // Random chance to fire (about 10% chance each frame)
            if self.prng.next_range(10) == 0 {
                enemy.fire();
            }

            // Movement behavior: sometimes move 1-5 points
            if self.prng.next_range(5) == 0 {
                // 20% chance to move each frame
                let steps = 1 + self.prng.next_range(5) as i8; // 1-5 steps

                for _ in 0..steps {
                    let current_direction = enemy.direction();
                    let new_pos = enemy.pos.move_by(current_direction);

                    // Check if tank can move forward
                    if !self.screen.collides(new_pos.x, new_pos.y, &enemy.figure) {
                        enemy.move_forward();
                    } else {
                        // Can't move forward, try to rotate to a direction where we can move
                        let directions = [
                            Dot::new(1, 0),  // right
                            Dot::new(-1, 0), // left
                            Dot::new(0, 1),  // down
                            Dot::new(0, -1), // up
                        ];

                        // Try each direction to find one where we can move
                        let mut found_direction = false;
                        for &direction in &directions {
                            let test_pos = enemy.pos.move_by(direction);
                            if !self.screen.collides(test_pos.x, test_pos.y, &enemy.figure) {
                                // Rotate to this direction
                                enemy.rotate(&direction);
                                found_direction = true;
                                break;
                            }
                        }

                        // If we found a good direction, move in it
                        if found_direction {
                            let new_direction = enemy.direction();
                            let final_pos = enemy.pos.move_by(new_direction);
                            if !self
                                .screen
                                .collides(final_pos.x, final_pos.y, &enemy.figure)
                            {
                                enemy.move_forward();
                            }
                        }
                        break; // Stop moving if we hit an obstacle
                    }
                }
            }

            self.enemies[i] = enemy;
        }

        // Clean up dead tanks
        let mut i = 0;
        while i < self.enemy_count {
            if self.enemies[i].is_dead() {
                self.enemies[i] = self.enemies[self.enemy_count - 1];
                self.enemy_count -= 1;
            } else {
                i += 1;
            }
        }
    }

    fn draw_enemy_tank(&mut self, idx: usize) {
        let color = if self.tank.player() {
            GREEN_IDX
        } else {
            RED_IDX
        };
        self.screen.draw_figure(
            self.enemies[idx].pos.x,
            self.enemies[idx].pos.y,
            &self.enemies[idx].figure,
            color,
        );
    }

    fn draw_enemy_missiles(&mut self, idx: usize) {
        for m in &self.enemies[idx].missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }
    fn draw_tank(&mut self) {
        let color = if self.tank.player() {
            GREEN_IDX
        } else {
            RED_IDX
        };
        self.screen
            .draw_figure(self.tank.pos.x, self.tank.pos.y, &self.tank.figure, color);
    }

    fn draw_missiles(&mut self) {
        for m in &self.tank.missiles {
            if m.visible() {
                self.screen.set(m.x as usize, m.y as usize, RED_IDX);
            }
        }
    }

    fn draw_score(&mut self) {
        let score_display = (self.score % 100) as usize;
        let tens = score_display / 10;
        let ones = score_display % 10;

        self.screen.draw_figure(0, 0, &DIGITS[tens], GREEN_IDX);

        self.screen.draw_figure(5, 0, &DIGITS[ones], GREEN_IDX);
    }

    fn draw_lives(&mut self) {
        for i in 0..self.lives {
            self.screen.set(3, i as usize, PINK_IDX);
        }
    }

    fn check_collisions(&mut self) {
        for i in 0..self.enemy_count {
            let enemy = &mut self.enemies[i];
            for m in &mut enemy.missiles {
                if m.visible() && self.tank.collides(Dot::new(m.x, m.y)) {
                    self.tank.hit();
                    m.hide();
                }
            }
        }

        for m in &mut self.tank.missiles {
            if m.visible() {
                for j in 0..self.enemy_count {
                    let enemy = &mut self.enemies[j];
                    if enemy.collides(Dot::new(m.x, m.y)) {
                        enemy.hit();
                        m.hide();
                        if enemy.is_dead() {
                            self.score += 1;
                        }
                    }
                }
            }
        }
    }

    async fn game_over(&mut self, mut leds: [RGB8; 256]) {
        while !self.controller.was_pressed() {
            let x = self.prng.next_range(SCREEN_WIDTH as u8);
            let y = self.prng.next_range(SCREEN_HEIGHT as u8);
            let color = self.prng.next_range(COLORS.len() as u8);
            self.screen.set(x as usize, y as usize, color);
            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(200).await;
        }
    }
}

impl<'a, D: LedDisplay, C: GameController, T: Timer> Game for TanksGame<'a, D, C, T> {
    async fn run(&mut self)
    where
        D: LedDisplay,
        C: GameController,
        T: Timer,
    {
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        let mut step = 10;
        let round = 10;

        loop {
            self.screen.clear();
            if self.tank.is_dead() {
                self.lives -= 1;
                if self.lives < 0 {
                    self.game_over(leds).await;
                    return;
                }
            }

            if self.controller.was_pressed() {
                self.tank.fire();
            }

            let x_input = self.controller.read_x().await;
            let y_input = self.controller.read_y().await;
            let direction = Dot::new(x_input, y_input).to_direction();

            let fig = self.tank.figure;
            self.tank.move_(
                &direction,
                |x, y, _| self.screen.collides(x, y, &fig),
                false,
            );

            self.tank.move_missiles();
            for i in 0..self.enemy_count {
                self.enemies[i].move_missiles();
            }
            self.check_collisions();

            self.draw_tank();
            for i in 0..self.enemy_count {
                self.draw_enemy_tank(i);
                self.draw_enemy_missiles(i);
            }
            self.draw_missiles();
            self.draw_score();
            self.draw_lives();

            let speedup = self.score / 10;
            if step >= round {
                self.ai();
                step = 0;
            }
            step += 1 + speedup;

            self.screen.render(&mut leds);
            self.display.write(&leds).await;
            self.timer.sleep_millis(100).await;
        }
    }
}
