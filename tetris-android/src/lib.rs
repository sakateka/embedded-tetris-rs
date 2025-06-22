use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use android_logger::Config;
use log::info;
use smart_leds::RGB8;

use std::sync::atomic::{AtomicBool, AtomicI8, Ordering};
use std::time::Duration;
use tetris_lib::{
    common::{GameController, LedDisplay, Timer, SCREEN_HEIGHT, SCREEN_WIDTH},
    games::run_game_menu,
};

// Global state for the game display and input
static mut LEDS: [RGB8; 256] = [RGB8::new(0, 0, 0); 256];
static SHOULD_UPDATE_DISPLAY: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct InputState {
    x_input: AtomicI8,
    y_input: AtomicI8,
    joystick_pressed: AtomicBool,
    a_pressed: AtomicBool,
    b_pressed: AtomicBool,
    prev_joystick_pressed: AtomicBool,
    prev_a_pressed: AtomicBool,
    prev_b_pressed: AtomicBool,
}

static INPUT_STATE: InputState = InputState {
    x_input: AtomicI8::new(0),
    y_input: AtomicI8::new(0),
    joystick_pressed: AtomicBool::new(false),
    a_pressed: AtomicBool::new(false),
    b_pressed: AtomicBool::new(false),
    prev_joystick_pressed: AtomicBool::new(false),
    prev_a_pressed: AtomicBool::new(false),
    prev_b_pressed: AtomicBool::new(false),
};

// PNG button icons (placeholder data - replace with actual PNG bytes)
// These are minimal 32x32 PNG images for each button
const LEFT_ARROW_PNG: &[u8] = include_bytes!("../assets/left_arrow.png");
const RIGHT_ARROW_PNG: &[u8] = include_bytes!("../assets/right_arrow.png");
const UP_ARROW_PNG: &[u8] = include_bytes!("../assets/up_arrow.png");
const DOWN_ARROW_PNG: &[u8] = include_bytes!("../assets/down_arrow.png");
const A_BUTTON_PNG: &[u8] = include_bytes!("../assets/a_button.png");
const B_BUTTON_PNG: &[u8] = include_bytes!("../assets/b_button.png");
const ENTER_BUTTON_PNG: &[u8] = include_bytes!("../assets/enter_button.png");

// Structure to hold decoded PNG data
struct ButtonIcon {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 4]>, // RGBA pixels
}

impl ButtonIcon {
    fn from_png_bytes(png_data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let img = image::load_from_memory(png_data)?;
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();

        let pixels: Vec<[u8; 4]> = rgba_img
            .pixels()
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();

        Ok(ButtonIcon {
            width,
            height,
            pixels,
        })
    }
}

// Timer implementation for Android
pub struct AndroidTimer;

impl Timer for AndroidTimer {
    async fn sleep_millis(&self, millis: u64) {
        std::thread::sleep(Duration::from_millis(millis));
    }
}

// Display implementation for Android
pub struct AndroidDisplay {
    app: AndroidApp,
}

impl AndroidDisplay {
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    fn draw_touch_controls(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        window_width: usize,
        window_height: usize,
        stride: usize,
        controls_height: usize,
    ) {
        let controls_y_start = window_height - controls_height;

        // SIMPLIFIED: Side buttons in fixed, safe positions for testing
        let side_button_width = 160; // Twice bigger
        let side_button_height = 160; // Twice bigger

        // Put them at the bottom but still on the sides
        let left_button_x = 10;
        let right_button_x = window_width - side_button_width - 10;
        let button_start_y = controls_y_start - (3 * side_button_height + 2 * 20); // Position so all 3 buttons fit above controls area
        let button_gap = 20; // Bigger gap too

        // Left side buttons: Left, Up, A (3 buttons vertically)
        self.draw_button_with_text(
            pixels,
            left_button_x,
            button_start_y,
            side_button_width,
            side_button_height,
            stride,
            "←",
        );
        self.draw_button_with_text(
            pixels,
            left_button_x,
            button_start_y + side_button_height + button_gap,
            side_button_width,
            side_button_height,
            stride,
            "↑",
        );
        self.draw_button_with_text(
            pixels,
            left_button_x,
            button_start_y + 2 * (side_button_height + button_gap),
            side_button_width,
            side_button_height,
            stride,
            "A",
        );

        // Right side buttons: Right, Down, B (3 buttons vertically)
        self.draw_button_with_text(
            pixels,
            right_button_x,
            button_start_y,
            side_button_width,
            side_button_height,
            stride,
            "→",
        );
        self.draw_button_with_text(
            pixels,
            right_button_x,
            button_start_y + side_button_height + button_gap,
            side_button_width,
            side_button_height,
            stride,
            "↓",
        );
        self.draw_button_with_text(
            pixels,
            right_button_x,
            button_start_y + 2 * (side_button_height + button_gap),
            side_button_width,
            side_button_height,
            stride,
            "B",
        );

        // No more center buttons - all moved to sides

        // Big Enter button at the bottom (now can be bigger since no center buttons)
        let big_button_width = window_width * 2 / 3; // Bigger since no center buttons
        let big_button_height = controls_height; // Twice as high - full controls area height
        let big_button_x = (window_width - big_button_width) / 2; // Center horizontally
        let big_button_y = controls_y_start; // Start at bottom of screen

        self.draw_button_with_text(
            pixels,
            big_button_x,
            big_button_y,
            big_button_width,
            big_button_height,
            stride,
            "⏎",
        );
    }

    fn draw_button_with_text(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        stride: usize,
        label: &str,
    ) {
        // Colors in R5G6B5 format
        let border_color = 0xFFFFu16.to_le_bytes(); // White
        let fill_color = 0x2104u16.to_le_bytes(); // Dark gray

        // Draw button background and border
        for py in 0..height {
            for px in 0..width {
                let screen_x = x + px;
                let screen_y = y + py;
                let pixel_offset = (screen_y * stride + screen_x) * 2;

                if pixel_offset + 1 < pixels.len() {
                    // Draw border (2-pixel wide)
                    let is_border = px < 2 || px >= width - 2 || py < 2 || py >= height - 2;
                    let color = if is_border { border_color } else { fill_color };

                    pixels[pixel_offset].write(color[0]);
                    pixels[pixel_offset + 1].write(color[1]);
                }
            }
        }

        // Draw text/icon in the center of the button
        let icon_size = 80; // Bigger icons for better visibility (was 20)
        let icon_x = x + (width - icon_size) / 2;
        let icon_y = y + (height - icon_size) / 2;

        // Draw PNG icons only
        match label {
            "←" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, LEFT_ARROW_PNG);
            }
            "→" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, RIGHT_ARROW_PNG);
            }
            "↑" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, UP_ARROW_PNG);
            }
            "↓" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, DOWN_ARROW_PNG);
            }
            "A" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, A_BUTTON_PNG);
            }
            "B" => {
                self.draw_png_icon(pixels, icon_x, icon_y, icon_size, stride, B_BUTTON_PNG);
            }
            "⏎" => {
                // Even smaller enter icon (was width/3, now width/4)
                let enter_icon_size = width / 4;
                let enter_x = x + (width - enter_icon_size) / 2;
                let enter_y = y + (height - enter_icon_size) / 2;
                self.draw_png_icon(
                    pixels,
                    enter_x,
                    enter_y,
                    enter_icon_size,
                    stride,
                    ENTER_BUTTON_PNG,
                );
            }
            _ => {}
        }
    }

    fn draw_png_icon(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        x: usize,
        y: usize,
        size: usize,
        stride: usize,
        png_data: &[u8],
    ) {
        // Try to load and draw PNG icon
        match ButtonIcon::from_png_bytes(png_data) {
            Ok(icon) => {
                let scale_x = size as f32 / icon.width as f32;
                let scale_y = size as f32 / icon.height as f32;
                let scale = scale_x.min(scale_y); // Maintain aspect ratio

                let scaled_width = (icon.width as f32 * scale) as usize;
                let scaled_height = (icon.height as f32 * scale) as usize;

                // Center the scaled icon
                let offset_x = (size - scaled_width) / 2;
                let offset_y = (size - scaled_height) / 2;

                for py in 0..scaled_height {
                    for px in 0..scaled_width {
                        // Calculate source pixel (with scaling)
                        let src_x = (px as f32 / scale) as usize;
                        let src_y = (py as f32 / scale) as usize;

                        if src_x < icon.width as usize && src_y < icon.height as usize {
                            let src_idx = src_y * icon.width as usize + src_x;
                            if src_idx < icon.pixels.len() {
                                let [r, g, b, a] = icon.pixels[src_idx];

                                // Skip transparent pixels
                                if a < 128 {
                                    continue;
                                }

                                // Convert RGBA to R5G6B5
                                let r5 = (r as u16 >> 3) & 0x1F; // 5 bits
                                let g6 = (g as u16 >> 2) & 0x3F; // 6 bits
                                let b5 = (b as u16 >> 3) & 0x1F; // 5 bits
                                let rgb565 = (r5 << 11) | (g6 << 5) | b5;
                                let color_bytes = rgb565.to_le_bytes();

                                // Draw pixel to screen
                                let screen_x = x + offset_x + px;
                                let screen_y = y + offset_y + py;
                                let pixel_offset = (screen_y * stride + screen_x) * 2;

                                if pixel_offset + 1 < pixels.len() {
                                    pixels[pixel_offset].write(color_bytes[0]);
                                    pixels[pixel_offset + 1].write(color_bytes[1]);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load PNG icon: {:?}", e);
            }
        }
    }

    fn render_to_native_window(&self, leds: &[RGB8; 256]) {
        // Count non-black pixels to verify the game is running
        let active_pixels = leds
            .iter()
            .filter(|led| led.r > 0 || led.g > 0 || led.b > 0)
            .count();

        if let Some(native_window) = self.app.native_window() {
            // Try to lock the window buffer for drawing
            match native_window.lock(None) {
                Ok(mut buffer) => {
                    let window_width = buffer.width() as usize;
                    let window_height = buffer.height() as usize;
                    let stride = buffer.stride() as usize;

                    // let format = buffer.format();
                    // info!(
                    //     "📱 Window: {}x{}, stride: {}, format: {:?}",
                    //     window_width, window_height, stride, format
                    // );

                    // Calculate scaling to fit the 8x32 display in the window
                    let scale_x = window_width / SCREEN_WIDTH;
                    let scale_y = window_height / SCREEN_HEIGHT;
                    let base_scale = scale_x.min(scale_y).max(1);
                    let scale = (((base_scale * 3) as f64 * 0.97) as usize).min(60); // 3x larger, max 60x

                    // Center the display in the upper portion, leaving space for controls
                    let display_width = SCREEN_WIDTH * scale;
                    let display_height = SCREEN_HEIGHT * scale;
                    let controls_height = 150; // Smaller controls area to give more space to game
                    let game_area_height = window_height.saturating_sub(controls_height);

                    let offset_x = (window_width - display_width) / 2;
                    let offset_y = (game_area_height - display_height) / 2;

                    // Get buffer as slice of pixels
                    let Some(pixels) = buffer.bytes() else {
                        log::warn!("Failed to get buffer bytes");
                        return;
                    };

                    // Clear the entire screen to medium gray first
                    // Medium gray in R5G6B5: R=16, G=32, B=16 (roughly 50% gray)
                    let gray_r5g6b5: u16 = (16 << 11) | (32 << 5) | 16;
                    let gray_bytes = gray_r5g6b5.to_le_bytes();

                    for i in (0..pixels.len()).step_by(2) {
                        if i + 1 < pixels.len() {
                            pixels[i].write(gray_bytes[0]); // Low byte
                            pixels[i + 1].write(gray_bytes[1]); // High byte
                        }
                    }

                    // No need to fill game area separately - black pixels will be converted to dark gray automatically

                    // Draw each LED pixel as a scaled block
                    for led_y in 0..SCREEN_HEIGHT {
                        for led_x in 0..SCREEN_WIDTH {
                            let led_idx = if led_y % 2 == 0 {
                                led_y * SCREEN_WIDTH + (SCREEN_WIDTH - 1 - led_x)
                            } else {
                                led_y * SCREEN_WIDTH + led_x
                            };

                            let led = leds[led_idx];

                            // Check if this is a black pixel (background) and convert to dark gray
                            let (r, g, b) = if led.r == 0 && led.g == 0 && led.b == 0 {
                                // Convert black pixels to dark gray (equal RGB values for true gray)
                                (12u8, 12u8, 12u8) // Dark gray RGB values - all equal for neutral gray
                            } else {
                                // Keep original color for non-black pixels
                                (led.r, led.g, led.b)
                            };

                            // For R5G6B5 format: 5 bits red, 6 bits green, 5 bits blue
                            // Scale LED color from 0-31 to appropriate bit ranges
                            let r5 = (r as u16).min(31); // 5 bits: 0-31
                            let g6 = (g as u16 * 63 / 31).min(63); // 6 bits: 0-63
                            let b5 = (b as u16).min(31); // 5 bits: 0-31

                            // Pack into 16-bit R5G6B5 format: RRRRRGGGGGGBBBBB
                            let rgb565 = (r5 << 11) | (g6 << 5) | b5;
                            let color_bytes = rgb565.to_le_bytes(); // Little endian

                            // Draw scaled pixel block
                            for py in 0..scale {
                                for px in 0..scale {
                                    let screen_x = offset_x + led_x * scale + px;
                                    let screen_y = offset_y + led_y * scale + py;

                                    if screen_x < window_width && screen_y < window_height {
                                        let pixel_offset = (screen_y * stride + screen_x) * 2; // 2 bytes per pixel for R5G6B5
                                        if pixel_offset + 1 < pixels.len() {
                                            pixels[pixel_offset].write(color_bytes[0]); // Low byte
                                            pixels[pixel_offset + 1].write(color_bytes[1]);
                                            // High byte
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Draw touch controls at the bottom
                    self.draw_touch_controls(
                        pixels,
                        window_width,
                        window_height,
                        stride,
                        controls_height,
                    );

                    // Unlock buffer to present to screen
                    drop(buffer);

                    // if active_pixels > 0 {
                    //     info!(
                    //         "📺 Rendered {} pixels to {}x{} window (scale: {}x)",
                    //         active_pixels, window_width, window_height, scale
                    //     );
                    // }
                }
                Err(e) => {
                    if active_pixels > 0 {
                        log::warn!("Failed to lock native window buffer: {:?}", e);
                    }
                }
            }
            // } else if active_pixels > 0 {
            //     // Only warn when we actually have something to display
            //     log::warn!(
            //         "⚠️  Native window not available yet (active pixels: {})",
            //         active_pixels
            //     );
        }
    }
}

impl LedDisplay for AndroidDisplay {
    async fn write(&mut self, leds: &[RGB8; 256]) {
        unsafe {
            LEDS.copy_from_slice(leds);
        }
        SHOULD_UPDATE_DISPLAY.store(true, Ordering::Relaxed);

        // Actually render to the screen
        self.render_to_native_window(leds);
    }
}

// Controller implementation for Android
pub struct AndroidController {
    app: AndroidApp,
}

impl AndroidController {
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    fn handle_touch_input(&self, x: usize, y: usize) {
        // Get window dimensions to calculate button positions
        if let Some(native_window) = self.app.native_window() {
            let window_width = native_window.width() as usize;
            let window_height = native_window.height() as usize;
            let controls_height = 150;
            let controls_y_start = window_height - controls_height;

            // Clear previous inputs first
            INPUT_STATE.x_input.store(0, Ordering::Relaxed);
            INPUT_STATE.y_input.store(0, Ordering::Relaxed);
            INPUT_STATE.joystick_pressed.store(false, Ordering::Relaxed);
            INPUT_STATE.a_pressed.store(false, Ordering::Relaxed);
            INPUT_STATE.b_pressed.store(false, Ordering::Relaxed);

            // SIMPLIFIED: Check side buttons using same simple positions as drawing
            let side_button_width = 160; // Twice bigger
            let side_button_height = 160; // Twice bigger
            let left_button_x = 10;
            let right_button_x = window_width - side_button_width - 10;
            let button_start_y = controls_y_start - (3 * side_button_height + 2 * 20); // Position so all 3 buttons fit above controls area

            let button_gap = 20; // Bigger gap to match drawing

            // Left side buttons: Left, Up, A (3 buttons vertically)
            // Left button
            if x >= left_button_x
                && x < left_button_x + side_button_width
                && y >= button_start_y
                && y < button_start_y + side_button_height
            {
                INPUT_STATE.x_input.store(-1, Ordering::Relaxed); // Left
            }
            // Up button
            if x >= left_button_x
                && x < left_button_x + side_button_width
                && y >= button_start_y + side_button_height + button_gap
                && y < button_start_y + side_button_height + button_gap + side_button_height
            {
                INPUT_STATE.y_input.store(-1, Ordering::Relaxed); // Up
            }
            // A button
            if x >= left_button_x
                && x < left_button_x + side_button_width
                && y >= button_start_y + 2 * (side_button_height + button_gap)
                && y < button_start_y + 2 * (side_button_height + button_gap) + side_button_height
            {
                INPUT_STATE.a_pressed.store(true, Ordering::Relaxed); // A
            }

            // Right side buttons: Right, Down, B (3 buttons vertically)
            // Right button
            if x >= right_button_x
                && x < right_button_x + side_button_width
                && y >= button_start_y
                && y < button_start_y + side_button_height
            {
                INPUT_STATE.x_input.store(1, Ordering::Relaxed); // Right
            }
            // Down button
            if x >= right_button_x
                && x < right_button_x + side_button_width
                && y >= button_start_y + side_button_height + button_gap
                && y < button_start_y + side_button_height + button_gap + side_button_height
            {
                INPUT_STATE.y_input.store(1, Ordering::Relaxed); // Down
            }
            // B button
            if x >= right_button_x
                && x < right_button_x + side_button_width
                && y >= button_start_y + 2 * (side_button_height + button_gap)
                && y < button_start_y + 2 * (side_button_height + button_gap) + side_button_height
            {
                INPUT_STATE.b_pressed.store(true, Ordering::Relaxed); // B
            }

            // Check big ENTER button (updated to match new drawing size)
            let big_button_width = window_width * 2 / 3; // Bigger since no center buttons
            let big_button_height = controls_height; // Twice as high - full controls area height
            let big_button_x = (window_width - big_button_width) / 2; // Center horizontally
            let big_button_y = controls_y_start; // Start at bottom of screen

            if x >= big_button_x
                && x < big_button_x + big_button_width
                && y >= big_button_y
                && y < big_button_y + big_button_height
            {
                INPUT_STATE.joystick_pressed.store(true, Ordering::Relaxed);
            }
        }
    }

    fn process_input_events(&self) {
        match self.app.input_events_iter() {
            Ok(mut iter) => {
                loop {
                    let read_input = iter.next(|event| {
                        use android_activity::input::{InputEvent, KeyAction, Keycode};

                        let handled = match event {
                            InputEvent::KeyEvent(key_event) => {
                                let pressed = key_event.action() == KeyAction::Down;

                                match key_event.key_code() {
                                    Keycode::DpadLeft => INPUT_STATE
                                        .x_input
                                        .store(if pressed { -1 } else { 0 }, Ordering::Relaxed),
                                    Keycode::DpadRight => INPUT_STATE
                                        .x_input
                                        .store(if pressed { 1 } else { 0 }, Ordering::Relaxed),
                                    Keycode::DpadUp => INPUT_STATE
                                        .y_input
                                        .store(if pressed { -1 } else { 0 }, Ordering::Relaxed),
                                    Keycode::DpadDown => INPUT_STATE
                                        .y_input
                                        .store(if pressed { 1 } else { 0 }, Ordering::Relaxed),
                                    Keycode::DpadCenter | Keycode::Enter | Keycode::Space => {
                                        INPUT_STATE
                                            .joystick_pressed
                                            .store(pressed, Ordering::Relaxed);
                                    }
                                    Keycode::A => {
                                        INPUT_STATE.a_pressed.store(pressed, Ordering::Relaxed)
                                    }
                                    Keycode::B => {
                                        INPUT_STATE.b_pressed.store(pressed, Ordering::Relaxed)
                                    }
                                    _ => {}
                                }
                                true
                            }
                            InputEvent::MotionEvent(motion_event) => {
                                use android_activity::input::{MotionAction, Source};

                                // Only handle touch screen events
                                if motion_event.source() == Source::Touchscreen {
                                    let pointer = motion_event.pointer_at_index(0);
                                    let x = pointer.x() as usize;
                                    let y = pointer.y() as usize;

                                    match motion_event.action() {
                                        MotionAction::Down | MotionAction::Move => {
                                            self.handle_touch_input(x, y);
                                            true
                                        }
                                        MotionAction::Up => {
                                            // Clear all touch inputs when finger lifts
                                            INPUT_STATE.x_input.store(0, Ordering::Relaxed);
                                            INPUT_STATE.y_input.store(0, Ordering::Relaxed);
                                            INPUT_STATE
                                                .joystick_pressed
                                                .store(false, Ordering::Relaxed);
                                            INPUT_STATE.a_pressed.store(false, Ordering::Relaxed);
                                            INPUT_STATE.b_pressed.store(false, Ordering::Relaxed);
                                            true
                                        }
                                        _ => false,
                                    }
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };

                        if handled {
                            InputStatus::Handled
                        } else {
                            InputStatus::Unhandled
                        }
                    });

                    if !read_input {
                        break;
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to get input events iterator: {err:?}");
            }
        }
    }
}

impl GameController for AndroidController {
    async fn read_x(&mut self) -> i8 {
        self.process_input_events();
        INPUT_STATE.x_input.load(Ordering::Relaxed)
    }

    async fn read_y(&mut self) -> i8 {
        self.process_input_events();
        INPUT_STATE.y_input.load(Ordering::Relaxed)
    }

    fn joystick_was_pressed(&self) -> bool {
        let current = INPUT_STATE.joystick_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE
            .prev_joystick_pressed
            .swap(current, Ordering::Relaxed);
        current && !prev
    }

    fn a_was_pressed(&self) -> bool {
        let current = INPUT_STATE.a_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE.prev_a_pressed.swap(current, Ordering::Relaxed);
        current && !prev
    }

    fn b_was_pressed(&self) -> bool {
        let current = INPUT_STATE.b_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE.prev_b_pressed.swap(current, Ordering::Relaxed);
        current && !prev
    }
}

// Main entry point using android-activity
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(Config::default().with_max_level(log::LevelFilter::Info));
    info!("Tetris Android app starting with android-activity");

    // Start the game in a separate thread
    let game_app = app.clone();
    let _game_handle = std::thread::spawn(move || {
        // Create a simple async runtime using futures-executor
        let mut display = AndroidDisplay::new(game_app.clone());
        let mut controller = AndroidController::new(game_app);
        let timer = AndroidTimer;

        let seed_fn = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32
        };

        info!("Starting game menu with android-activity backend");

        // Use a simple blocking async runtime
        pollster::block_on(async {
            run_game_menu(&mut display, &mut controller, &timer, seed_fn).await;
        });
    });

    // Main event loop
    let mut quit = false;
    loop {
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            match event {
                PollEvent::Wake => {
                    info!("App woke up");
                }
                PollEvent::Timeout => {
                    // Regular frame update - 60 FPS target
                }
                PollEvent::Main(main_event) => match main_event {
                    MainEvent::Destroy => {
                        info!("🚪 App destroy - shutting down");
                        quit = true;
                    }
                    MainEvent::Start => {
                        info!("🚀 App started");
                    }
                    MainEvent::Resume { .. } => {
                        info!("▶️  App resumed");
                    }
                    MainEvent::Pause => {
                        info!("⏸️  App paused");
                    }
                    MainEvent::Stop => {
                        info!("⏹️  App stopped");
                    }
                    MainEvent::InitWindow { .. } => {
                        info!("🪟 Native window initialized - ready for rendering!");
                    }
                    MainEvent::TerminateWindow { .. } => {
                        info!("❌ Native window terminated");
                    }
                    MainEvent::RedrawNeeded { .. } => {
                        info!("🎨 Redraw requested");
                    }
                    _ => {
                        info!("📱 Other main event: {:?}", main_event);
                    }
                },
                _ => {}
            }
        });

        if quit {
            break;
        }
    }

    info!("Android app shutting down");
}
