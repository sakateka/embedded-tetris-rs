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

        // Define button layout: [Left] [Down] [Up] [Right]  [A] [B]
        let button_width = window_width / 6;
        let button_height = controls_height / 2;
        let button_y = controls_y_start + (controls_height - button_height) / 2;

        let buttons = [
            (0 * button_width, "←"), // Left
            (1 * button_width, "↓"), // Down
            (2 * button_width, "↑"), // Up
            (3 * button_width, "→"), // Right
            (4 * button_width, "A"), // A button
            (5 * button_width, "B"), // B button
        ];

        // Draw each button
        for (x, _label) in buttons {
            self.draw_button(pixels, x, button_y, button_width, button_height, stride);
        }
    }

    fn draw_button(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        stride: usize,
    ) {
        // Draw button border in white (R5G6B5: all bits set)
        let border_color = 0xFFFFu16.to_le_bytes();
        let fill_color = 0x8410u16.to_le_bytes(); // Dark gray

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
                    let format = buffer.format();

                    info!(
                        "📱 Window: {}x{}, stride: {}, format: {:?}",
                        window_width, window_height, stride, format
                    );

                    // Calculate scaling to fit the 8x32 display in the window
                    let scale_x = window_width / SCREEN_WIDTH;
                    let scale_y = window_height / SCREEN_HEIGHT;
                    let scale = scale_x.min(scale_y).max(1).min(20); // Limit scale to prevent huge pixels

                    // Center the display in the upper portion, leaving space for controls
                    let display_width = SCREEN_WIDTH * scale;
                    let display_height = SCREEN_HEIGHT * scale;
                    let controls_height = 200; // Reserve space for touch controls
                    let game_area_height = window_height.saturating_sub(controls_height);

                    let offset_x = (window_width - display_width) / 2;
                    let offset_y = (game_area_height - display_height) / 2;

                    // Get buffer as slice of pixels
                    let Some(pixels) = buffer.bytes() else {
                        log::warn!("Failed to get buffer bytes");
                        return;
                    };

                    // Clear the screen to black
                    for pixel in pixels.iter_mut() {
                        pixel.write(0);
                    }

                    // Draw each LED pixel as a scaled block
                    for led_y in 0..SCREEN_HEIGHT {
                        for led_x in 0..SCREEN_WIDTH {
                            let led_idx = if led_y % 2 == 0 {
                                led_y * SCREEN_WIDTH + (SCREEN_WIDTH - 1 - led_x)
                            } else {
                                led_y * SCREEN_WIDTH + led_x
                            };

                            let led = leds[led_idx];

                            // For R5G6B5 format: 5 bits red, 6 bits green, 5 bits blue
                            // Scale LED color from 0-31 to appropriate bit ranges
                            let r5 = (led.r as u16).min(31); // 5 bits: 0-31
                            let g6 = (led.g as u16 * 63 / 31).min(63); // 6 bits: 0-63
                            let b5 = (led.b as u16).min(31); // 5 bits: 0-31

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

                    if active_pixels > 0 {
                        info!(
                            "📺 Rendered {} pixels to {}x{} window (scale: {}x)",
                            active_pixels, window_width, window_height, scale
                        );
                    }
                }
                Err(e) => {
                    if active_pixels > 0 {
                        log::warn!("Failed to lock native window buffer: {:?}", e);
                    }
                }
            }
        } else if active_pixels > 0 {
            // Only warn when we actually have something to display
            log::warn!(
                "⚠️  Native window not available yet (active pixels: {})",
                active_pixels
            );
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
            let controls_height = 200;
            let controls_y_start = window_height - controls_height;

            // Check if touch is in controls area
            if y >= controls_y_start {
                let button_width = window_width / 6;
                let button_height = controls_height / 2;
                let button_y = controls_y_start + (controls_height - button_height) / 2;

                // Check if touch is within button height range
                if y >= button_y && y < button_y + button_height {
                    let button_index = x / button_width;

                    // Clear previous inputs first
                    INPUT_STATE.x_input.store(0, Ordering::Relaxed);
                    INPUT_STATE.y_input.store(0, Ordering::Relaxed);
                    INPUT_STATE.joystick_pressed.store(false, Ordering::Relaxed);
                    INPUT_STATE.a_pressed.store(false, Ordering::Relaxed);
                    INPUT_STATE.b_pressed.store(false, Ordering::Relaxed);

                    // Set the appropriate input based on button
                    match button_index {
                        0 => INPUT_STATE.x_input.store(-1, Ordering::Relaxed), // Left
                        1 => INPUT_STATE.y_input.store(1, Ordering::Relaxed),  // Down
                        2 => INPUT_STATE.y_input.store(-1, Ordering::Relaxed), // Up
                        3 => INPUT_STATE.x_input.store(1, Ordering::Relaxed),  // Right
                        4 => INPUT_STATE.a_pressed.store(true, Ordering::Relaxed), // A
                        5 => INPUT_STATE.b_pressed.store(true, Ordering::Relaxed), // B
                        _ => {}
                    }
                }
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
