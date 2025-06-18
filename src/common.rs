use crate::figure::Figure;
use defmt::Format;
use embassy_time::Instant;
use smart_leds::RGB8;

pub const SCREEN_WIDTH: usize = 8;
pub const SCREEN_HEIGHT: usize = 32;
pub const SCREEN_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

// Colors matching the Python version
pub const BLACK: RGB8 = RGB8::new(0, 0, 0);
pub const BRICK: RGB8 = RGB8::new(12, 2, 0);
pub const RED: RGB8 = RGB8::new(6, 0, 0);
pub const GREEN: RGB8 = RGB8::new(0, 6, 0);
pub const BLUE: RGB8 = RGB8::new(0, 0, 6);
pub const LIGHT_BLUE: RGB8 = RGB8::new(0, 6, 6);
pub const PINK: RGB8 = RGB8::new(3, 0, 3);
pub const YELLOW: RGB8 = RGB8::new(6, 6, 0);
pub const DARK_GREEN: RGB8 = RGB8::new(0, 3, 0);
pub const LIGHT_GREEN: RGB8 = RGB8::new(0, 9, 0);

// Color indices
pub const BLACK_IDX: u8 = 0;
pub const BRICK_IDX: u8 = 1;
pub const RED_IDX: u8 = 2;
pub const GREEN_IDX: u8 = 3;
pub const BLUE_IDX: u8 = 4;
pub const LIGHT_BLUE_IDX: u8 = 5;
pub const PINK_IDX: u8 = 6;
pub const YELLOW_IDX: u8 = 7;
pub const DARK_GREEN_IDX: u8 = 8;
pub const _LIGHT_GREEN_IDX: u8 = 9;

pub type ColorsType = [RGB8; 10];
pub const COLORS: ColorsType = [
    BLACK,
    BRICK,
    RED,
    GREEN,
    BLUE,
    LIGHT_BLUE,
    PINK,
    YELLOW,
    DARK_GREEN,
    LIGHT_GREEN,
];

trait ColorsIndexer {
    fn at(&self, idx: u8) -> RGB8;
}

impl ColorsIndexer for ColorsType {
    fn at(&self, idx: u8) -> RGB8 {
        let mut idx = idx as usize;
        while idx >= self.len() {
            idx -= self.len();
        }
        self[idx]
    }
}

// Simple PRNG implementation
pub(crate) struct Prng {
    state: u32,
}

impl Prng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u8 {
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        (self.state >> 16) as u8
    }

    pub fn next_range(&mut self, max: u8) -> u8 {
        if max == 0 {
            return 0;
        }
        self.next() % max
    }
}

// Point/Dot structure for coordinates
#[derive(Copy, Clone, Format, PartialEq)]
pub(crate) struct Dot {
    pub x: i8,
    pub y: i8,
}

impl Dot {
    pub fn new(x: i8, y: i8) -> Self {
        Self { x, y }
    }

    pub fn move_by(&self, direction: Dot) -> Dot {
        Dot::new(self.x + direction.x, self.y + direction.y)
    }

    pub fn move_wrap(&self, direction: Dot) -> Dot {
        let mut new_dot = self.move_by(direction);

        if new_dot.x == -1 {
            new_dot.x = SCREEN_WIDTH as i8 - 1;
        } else if new_dot.x == SCREEN_WIDTH as i8 {
            new_dot.x = 0;
        }

        if new_dot.y == -1 {
            new_dot.y = SCREEN_HEIGHT as i8 - 1;
        } else if new_dot.y == SCREEN_HEIGHT as i8 {
            new_dot.y = 0;
        }

        new_dot
    }

    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }

    pub fn is_opposite(&self, other: &Dot) -> bool {
        (self.x + other.x) == 0 && (self.y + other.y) == 0
    }

    pub fn _opposite(&self) -> Dot {
        Dot::new(-self.x, -self.y)
    }

    pub fn _outside(&self) -> bool {
        self.x < 0 || self.x >= SCREEN_WIDTH as i8 || self.y < 0 || self.y >= SCREEN_HEIGHT as i8
    }

    pub fn to_direction(mut self) -> Dot {
        if self.x != 0 && self.y != 0 {
            self.x = 0;
        }
        Dot::new(self.x.signum(), self.y.signum())
    }
}

fn set_pixel(leds: &mut [RGB8], x: usize, y: usize, color_idx: u8) {
    let mut x = x;
    if y % 2 == 0 {
        x = 7 - x;
    }
    let idx = SCREEN_WIDTH * y + x;
    if idx < leds.len() {
        leds[idx] = COLORS.at(color_idx);
    }
}

pub(crate) struct FrameBuffer {
    content: [u8; SCREEN_SIZE],
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self {
            content: [0; SCREEN_SIZE],
        }
    }

    pub fn clear(&mut self) {
        self.content.fill(0);
    }

    pub fn clear_range(&mut self, from: usize, to: usize) {
        for idx in from..to.min(SCREEN_SIZE) {
            self.content[idx] = 0;
        }
    }

    pub fn set(&mut self, x: usize, y: usize, color: u8) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            let idx = y * SCREEN_WIDTH + x;
            self.content[idx] = color;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> u8 {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.content[y * SCREEN_WIDTH + x]
        } else {
            0
        }
    }

    fn available(&self, x: i8, y: i8, color: u8) -> bool {
        if x >= 0 && x < SCREEN_WIDTH as i8 && y >= 0 && y < SCREEN_HEIGHT as i8 {
            self.content[y as usize * SCREEN_WIDTH + x as usize] == color
        } else {
            false
        }
    }

    pub fn collides(&self, x: i8, y: i8, figure: &Figure) -> bool {
        for row in 0..figure.height() {
            for col in 0..figure.width() {
                if figure.get_bit(col, row)
                    && !self.available(x + col as i8, y + row as i8, BLACK_IDX)
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn draw_figure(&mut self, x: i8, y: i8, figure: &Figure, color: u8) {
        for row in 0..figure.height() {
            for col in 0..figure.width() {
                if figure.get_bit(col, row) {
                    let px = x + col as i8;
                    let py = y + row as i8;
                    if px >= 0 && px < SCREEN_WIDTH as i8 && py >= 0 && py < SCREEN_HEIGHT as i8 {
                        self.set(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    pub fn copy_from(&mut self, other: &FrameBuffer) {
        self.content.copy_from_slice(&other.content);
    }

    pub fn render(&self, leds: &mut [RGB8]) {
        for x in 0..SCREEN_WIDTH {
            for y in 0..SCREEN_HEIGHT {
                let color_idx = self.get(x, y);
                set_pixel(leds, x, y, color_idx);
            }
        }
    }

    pub fn row_is_full(&self, row: usize) -> bool {
        if row >= SCREEN_HEIGHT {
            return false;
        }
        for x in 0..SCREEN_WIDTH {
            if self.content[row * SCREEN_WIDTH + x] == 0 {
                return false;
            }
        }
        true
    }

    pub fn row_is_empty(&self, row: usize) -> bool {
        if row >= SCREEN_HEIGHT {
            return true;
        }
        for x in 0..SCREEN_WIDTH {
            if self.content[row * SCREEN_WIDTH + x] != 0 {
                return false;
            }
        }
        true
    }
}

// Abstract interfaces for game hardware - using generics instead of dyn traits

/// Trait for LED display functionality
pub trait LedDisplay {
    async fn write(&mut self, leds: &[smart_leds::RGB8]);
}

/// Trait for game controller functionality (joystick + button)
pub trait GameController {
    async fn read_x(&mut self) -> i8;
    async fn read_y(&mut self) -> i8;
    fn was_pressed(&self) -> bool;
}

/// Game trait for different game implementations - using generics to avoid dyn issues
pub trait Game {
    async fn run<D, C>(&mut self, display: &mut D, controller: &mut C)
    where
        D: LedDisplay,
        C: GameController;
}
