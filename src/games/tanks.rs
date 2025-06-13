use defmt::info;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;

use crate::{
    common::{COLORS, GREEN_IDX, RED_IDX, SCREEN_HEIGHT, SCREEN_WIDTH},
    digits::DIGITS,
    figure::{Figure, TANK},
    Dot, FrameBuffer, Game, Joystick, Prng,
};

#[derive(Clone, Copy)]
struct Missile {
    x: i8,
    y: i8,
    dx: i8,
    dy: i8,
    _group: u8,
}

impl Missile {
    fn new(x: i8, y: i8, dx: i8, dy: i8, group: u8) -> Self {
        Self {
            x,
            y,
            dx,
            dy,
            _group: group,
        }
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
    missiles: [Missile; 8],
    missile_count: usize,
    pos: Dot,
    origin: u8,
    player: bool,
    rotation: u8,
    figure: Figure,
    lives: i8,
}

impl Tank {
    pub fn new(pos: Dot, origin: u8, player: bool) -> Self {
        Self {
            missiles: [Missile::new(-1, -1, 0, 0, 0); 8],
            missile_count: 0,
            pos,
            origin,
            player,
            rotation: 0,
            figure: TANK,
            lives: 1,
        }
    }

    fn direction(&self) -> Dot {
        match self.rotation {
            0 => Dot::new(1, 0),
            1 => Dot::new(0, 1),
            2 => Dot::new(-1, 0),
            _ => Dot::new(0, -1),
        }
    }

    fn rotate(&mut self, clockwise: bool) {
        if clockwise {
            self.rotation = (self.rotation + 1) % 4;
        } else {
            self.rotation = (self.rotation + 3) % 4;
        }
    }

    fn fire(&mut self) {
        if self.missile_count < self.missiles.len() {
            let direction = self.direction();
            let pos = self.pos.move_by(Dot::new(1, 1));
            self.missiles[self.missile_count] = Missile::new(
                pos.x,
                pos.y,
                direction.x,
                direction.y,
                if self.player { 0 } else { 1 },
            );
            self.missile_count += 1;
        }
    }

    fn move_forward(&mut self) {
        let direction = self.direction();
        self.pos = self.pos.move_by(direction);
    }

    fn move_missiles(&mut self) {
        for i in 0..self.missile_count {
            self.missiles[i].move_();
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
pub struct TanksGame {
    screen: FrameBuffer,
    tank: Tank,
    enemies: [Tank; 4],
    enemy_count: usize,
    score: u32,
    lives: i8,
    prng: Prng,
}

impl TanksGame {
    const STAGE_SPAWN: u8 = 0;
    const STAGE_MOVE: u8 = 1;
    const STAGE_ROTATE: u8 = 2;
    const STAGE_FIRE: u8 = 3;
    const STAGE_NONE: u8 = 4;

    pub fn new() -> Self {
        Self {
            screen: FrameBuffer::new(),
            tank: Tank::new(Dot::new(3, 16), 0, true),
            enemies: [Tank::new(Dot::new(0, 0), 0, false); 4],
            enemy_count: 0,
            score: 0,
            lives: 3,
            prng: Prng::new(),
        }
    }

    fn can_hit_player(&self, tank: &Tank) -> bool {
        let direction = tank.direction();
        let pos = tank.pos.move_by(Dot::new(1, 1)); // Tank center
        let player_center = self.tank.pos.move_by(Dot::new(1, 1));

        if direction.x != 0 {
            pos.y == player_center.y
                && ((direction.x > 0 && pos.x < player_center.x)
                    || (direction.x < 0 && pos.x > player_center.x))
        } else if direction.y != 0 {
            pos.x == player_center.x
                && ((direction.y > 0 && pos.y < player_center.y)
                    || (direction.y < 0 && pos.y > player_center.y))
        } else {
            false
        }
    }

    fn _has_line_of_sight(&self, tank: &Tank, target_pos: Dot) -> bool {
        let tank_center = tank.pos.move_by(Dot::new(1, 1));
        let target_center = target_pos.move_by(Dot::new(1, 1));

        let dx = target_center.x - tank_center.x;
        let dy = target_center.y - tank_center.y;

        if dx != 0 && dy != 0 {
            return false;
        }

        let steps = dx.abs().max(dy.abs());
        if steps == 0 {
            return true;
        }

        let step_x = if dx == 0 {
            0
        } else if dx > 0 {
            1
        } else {
            -1
        };
        let step_y = if dy == 0 {
            0
        } else if dy > 0 {
            1
        } else {
            -1
        };

        for i in 1..steps.min(5) {
            let check_pos = Dot::new(tank_center.x + i * step_x, tank_center.y + i * step_y);
            if self.screen.collides(check_pos.x, check_pos.y, &tank.figure) {
                return false;
            }
        }

        true
    }

    fn _should_fire(&mut self, tank: &Tank) -> bool {
        if self.can_hit_player(tank) {
            return true;
        }
        self.prng.next_range(10) == 0
    }

    fn _smart_move(&mut self, tank: &mut Tank) {
        let player_pos = self.tank.pos;
        let tank_pos = tank.pos;

        let dx = player_pos.x - tank_pos.x;
        let dy = player_pos.y - tank_pos.y;

        let mut preferred_directions = [Dot::new(0, 0); 4];
        let mut dir_count = 0;

        if dx.abs() > dy.abs() {
            if dx > 0 {
                preferred_directions[dir_count] = Dot::new(1, 0);
                dir_count += 1;
            } else {
                preferred_directions[dir_count] = Dot::new(-1, 0);
                dir_count += 1;
            }
            if dy > 0 {
                preferred_directions[dir_count] = Dot::new(0, 1);
                dir_count += 1;
            } else if dy < 0 {
                preferred_directions[dir_count] = Dot::new(0, -1);
                dir_count += 1;
            }
        } else {
            if dy > 0 {
                preferred_directions[dir_count] = Dot::new(0, 1);
                dir_count += 1;
            } else {
                preferred_directions[dir_count] = Dot::new(0, -1);
                dir_count += 1;
            }
            if dx > 0 {
                preferred_directions[dir_count] = Dot::new(1, 0);
                dir_count += 1;
            } else if dx < 0 {
                preferred_directions[dir_count] = Dot::new(-1, 0);
                dir_count += 1;
            }
        }

        for i in 0..dir_count {
            let pos = tank.pos.move_by(preferred_directions[i]);
            if !self.screen.collides(pos.x, pos.y, &tank.figure) {
                tank.rotate(preferred_directions[i].x > 0);
                tank.move_forward();
                return;
            }
        }

        let direction = tank.direction();
        let pos = tank.pos.move_by(direction);
        if self.screen.collides(pos.x, pos.y, &tank.figure) {
            tank.rotate(!direction.is_zero());
        }
        tank.move_forward();
    }

    fn update_enemies(&mut self) {
        let stage = if self.enemy_count > 1 {
            let stages = if self.tank.lives <= 1 {
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            } else {
                [
                    Self::STAGE_FIRE,
                    Self::STAGE_FIRE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_MOVE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_ROTATE,
                    Self::STAGE_NONE,
                ]
            };
            stages[self.prng.next_range(stages.len() as u8) as usize]
        } else {
            Self::STAGE_SPAWN
        };

        match stage {
            Self::STAGE_SPAWN => {
                if self.enemy_count < self.enemies.len() {
                    let spawns = [
                        Dot::new(0, 6),
                        Dot::new(5, 6),
                        Dot::new(0, 29),
                        Dot::new(5, 29),
                    ];

                    for (idx, &spawn_pos) in spawns.iter().enumerate() {
                        if !self.enemies.iter().any(|e| e.origin == idx as u8) {
                            self.enemies[self.enemy_count] = Tank::new(spawn_pos, idx as u8, false);
                            self.enemy_count += 1;
                            break;
                        }
                    }
                }
            }
            Self::STAGE_NONE => {}
            _ => {
                for i in 0..self.enemy_count {
                    let mut enemy = self.enemies[i];
                    if enemy.is_dying() {
                        continue;
                    }

                    match stage {
                        Self::STAGE_MOVE => {
                            let direction = enemy.direction();
                            let pos = enemy.pos.move_by(direction);
                            if !self.screen.collides(pos.x, pos.y, &enemy.figure) {
                                enemy.move_forward();
                            } else {
                                enemy.rotate(self.prng.next_range(2) == 0);
                            }
                        }
                        Self::STAGE_FIRE => {
                            let can_hit = self.can_hit_player(&enemy);
                            let random_fire = self.prng.next_range(10) == 0;
                            if can_hit || random_fire {
                                enemy.fire();
                            }
                        }
                        Self::STAGE_ROTATE => {
                            enemy.rotate(self.prng.next_range(2) == 0);
                        }
                        _ => {}
                    }
                    enemy.move_missiles();
                    self.enemies[i] = enemy
                }
            }
        }
    }

    fn draw_enemy_tank(&mut self, idx: usize) {
        let color = if self.tank.player { GREEN_IDX } else { RED_IDX };
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
        let color = if self.tank.player { GREEN_IDX } else { RED_IDX };
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
        let mut score = self.score;
        let mut digits = [0u8; 4];
        let mut count = 0;

        while score > 0 && count < digits.len() {
            digits[count] = (score % 10) as u8;
            score /= 10;
            count += 1;
        }

        for i in (0..count).rev() {
            self.screen.draw_figure(
                ((count - 1 - i) * 4).try_into().unwrap(),
                0,
                &DIGITS[digits[i] as usize],
                GREEN_IDX,
            );
        }
    }

    fn draw_lives(&mut self) {
        for i in 0..self.lives {
            self.screen.set(i as usize, 7, GREEN_IDX);
        }
    }

    fn check_collisions(&mut self) {
        for i in 0..self.enemy_count {
            let enemy = &mut self.enemies[i];
            for j in 0..enemy.missile_count {
                let missile = &mut enemy.missiles[j];
                if missile.visible() && self.tank.collides(Dot::new(missile.x, missile.y)) {
                    self.tank.hit();
                    missile.hide();
                }
            }
        }

        for i in 0..self.tank.missile_count {
            let missile = &mut self.tank.missiles[i];
            if missile.visible() {
                for j in 0..self.enemy_count {
                    let enemy = &mut self.enemies[j];
                    if enemy.collides(Dot::new(missile.x, missile.y)) {
                        enemy.hit();
                        missile.hide();
                        if enemy.is_dead() {
                            self.score += 100;
                        }
                    }
                }
            }
        }
    }

    async fn game_over(
        &mut self,
        mut leds: [RGB8; 256],
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        while !joystick.was_pressed() {
            let x = self.prng.next_range(SCREEN_WIDTH as u8);
            let y = self.prng.next_range(SCREEN_HEIGHT as u8);
            let color = self.prng.next_range(COLORS.len() as u8);
            self.screen.set(x as usize, y as usize, color);
            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
        }
        info!("game over return");
    }
}

impl Game for TanksGame {
    async fn run(
        &mut self,
        ws2812: &mut PioWs2812<'_, embassy_rp::peripherals::PIO0, 0, 256>,
        joystick: &mut Joystick<'_>,
    ) {
        let mut ticker = Ticker::every(Duration::from_millis(100));
        let mut leds: [RGB8; 256] = [RGB8::default(); 256];

        let mut step = 10;
        let mut round = 10;

        loop {
            if self.tank.is_dead() {
                self.game_over(leds, ws2812, joystick).await;
                return;
            }

            if joystick.was_pressed() {
                self.tank.fire();
            }

            let x_input = joystick.read_x().await;
            let y_input = joystick.read_y().await;
            let direction = Dot::new(x_input, y_input);

            if !direction.is_zero() {
                self.tank.rotate(direction.x > 0);
                self.tank.move_forward();
            }

            self.tank.move_missiles();
            self.update_enemies();
            self.check_collisions();

            self.screen.clear();
            self.draw_tank();
            for i in 0..self.enemy_count {
                self.draw_enemy_tank(i);
                self.draw_enemy_missiles(i);
            }
            self.draw_missiles();
            self.draw_score();
            self.draw_lives();

            let speedup = self.score / 10;
            if step >= round - speedup {
                self.ai();
                step = 0;
            }
            step += 1;

            self.screen.render(&mut leds);
            ws2812.write(&leds).await;
            ticker.next().await;
        }
    }
}
