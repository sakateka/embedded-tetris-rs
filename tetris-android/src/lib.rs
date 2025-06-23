use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use android_logger::Config;
use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
    Font, FontSettings,
};
use log::info;
use smart_leds::RGB8;

use std::sync::{
    atomic::{AtomicBool, AtomicI8, Ordering},
    Mutex,
};
use std::time::Duration;
use tetris_lib::{
    common::{GameController, LedDisplay, Timer, SCREEN_HEIGHT, SCREEN_WIDTH},
    games::run_game_menu,
};

// Global state for the game display and input
static LEDS: Mutex<[RGB8; 256]> = Mutex::new([RGB8::new(0, 0, 0); 256]);
static SHOULD_UPDATE_DISPLAY: AtomicBool = AtomicBool::new(false);

// Gesture detection state
#[derive(Debug, Clone, Copy)]
struct TouchPoint {
    x: f32,
    y: f32,
    timestamp: std::time::Instant,
}

impl Default for TouchPoint {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            timestamp: std::time::Instant::now(),
        }
    }
}

struct GestureState {
    touch_start: std::sync::RwLock<Option<TouchPoint>>,
    last_touch: std::sync::RwLock<Option<TouchPoint>>,
    last_tap: std::sync::RwLock<Option<TouchPoint>>, // Track last tap for double tap detection
    started_in_game_area: std::sync::RwLock<bool>, // Track if current gesture started in LED display
}

impl Default for GestureState {
    fn default() -> Self {
        Self {
            touch_start: std::sync::RwLock::new(None),
            last_touch: std::sync::RwLock::new(None),
            last_tap: std::sync::RwLock::new(None), // Initialize last tap tracking
            started_in_game_area: std::sync::RwLock::new(false),
        }
    }
}

static GESTURE_STATE: GestureState = GestureState {
    touch_start: std::sync::RwLock::new(None),
    last_touch: std::sync::RwLock::new(None),
    last_tap: std::sync::RwLock::new(None), // Initialize last tap tracking in static
    started_in_game_area: std::sync::RwLock::new(false),
};

// Gesture detection constants
const MIN_SWIPE_DISTANCE: f32 = 100.0; // Minimum distance for a swipe
const MAX_SWIPE_TIME_MS: u64 = 500; // Maximum time for a swipe gesture
const TAP_MAX_DISTANCE: f32 = 50.0; // Maximum movement for a tap
const LONG_PRESS_TIME_MS: u64 = 500; // Time for long press detection
const DOUBLE_TAP_MAX_TIME_MS: u64 = 300; // Maximum time between taps for double tap
const DOUBLE_TAP_MAX_DISTANCE: f32 = 100.0; // Maximum distance between taps for double tap

#[derive(Debug, Clone, Copy)]
enum GestureType {
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
    Tap,
    DoubleTap,
    LongPress,
}

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
    // Flags to track if gesture inputs should be cleared after next read
    gesture_x_pending: AtomicBool,
    gesture_y_pending: AtomicBool,
    gesture_joystick_pending: AtomicBool,
    gesture_a_pending: AtomicBool,
    gesture_b_pending: AtomicBool,
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
    gesture_x_pending: AtomicBool::new(false),
    gesture_y_pending: AtomicBool::new(false),
    gesture_joystick_pending: AtomicBool::new(false),
    gesture_a_pending: AtomicBool::new(false),
    gesture_b_pending: AtomicBool::new(false),
};

// Track when we last processed input events to avoid processing too frequently
static LAST_INPUT_PROCESS_TIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// No embedded fonts - using system fonts only

// Structure to hold text renderer for ASCII characters
struct TextRenderer {
    font: Font,
    layout: Layout,
}

impl TextRenderer {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Try Android system fonts for ASCII characters (no emoji needed)
        let font = if let Ok(system_font_data) = std::fs::read("/system/fonts/Roboto-Regular.ttf") {
            Font::from_bytes(system_font_data, FontSettings::default())?
        } else if let Ok(system_font_data) = std::fs::read("/system/fonts/DroidSans.ttf") {
            Font::from_bytes(system_font_data, FontSettings::default())?
        } else {
            return Err("No suitable system font found for ASCII characters".into());
        };

        let layout = Layout::new(CoordinateSystem::PositiveYDown);
        Ok(Self { font, layout })
    }

    fn render_text_to_pixels(
        &mut self,
        text: &str,
        size: f32,
    ) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error>> {
        self.layout.reset(&LayoutSettings {
            max_width: Some(size),
            max_height: Some(size),
            ..LayoutSettings::default()
        });

        self.layout
            .append(&[&self.font], &TextStyle::new(text, size, 0));

        let glyphs = self.layout.glyphs();

        if glyphs.is_empty() {
            return Err("No glyphs found for text".into());
        }

        // Calculate bounds
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for glyph in glyphs {
            let (metrics, _) = self.font.rasterize(glyph.parent, glyph.key.px);
            min_x = min_x.min(glyph.x);
            min_y = min_y.min(glyph.y);
            max_x = max_x.max(glyph.x + metrics.width as f32);
            max_y = max_y.max(glyph.y + metrics.height as f32);
        }

        let width = (max_x - min_x).ceil() as u32;
        let height = (max_y - min_y).ceil() as u32;

        // Create bitmap
        let mut pixels = vec![0u8; (width * height) as usize];

        for glyph in glyphs {
            let (metrics, bitmap) = self.font.rasterize(glyph.parent, glyph.key.px);
            let glyph_x = (glyph.x - min_x) as i32;
            let glyph_y = (glyph.y - min_y) as i32;

            for y in 0..metrics.height {
                for x in 0..metrics.width {
                    let src_idx = y * metrics.width + x;
                    let dst_x = glyph_x + x as i32;
                    let dst_y = glyph_y + y as i32;

                    if dst_x >= 0 && dst_y >= 0 && dst_x < width as i32 && dst_y < height as i32 {
                        let dst_idx = (dst_y as u32 * width + dst_x as u32) as usize;
                        if src_idx < bitmap.len() && dst_idx < pixels.len() {
                            pixels[dst_idx] = bitmap[src_idx];
                        }
                    }
                }
            }
        }

        Ok((pixels, width, height))
    }
}

// Global text renderer instance
lazy_static::lazy_static! {
    static ref TEXT_RENDERER: std::sync::Mutex<TextRenderer> = std::sync::Mutex::new(TextRenderer::new().unwrap());
}

// Helper structs to reduce function parameter counts
#[derive(Debug, Clone, Copy)]
struct ButtonRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl ButtonRect {
    fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RenderContext {
    x: usize,
    y: usize,
    max_size: usize,
    stride: usize,
}

impl RenderContext {
    fn new(x: usize, y: usize, max_size: usize, stride: usize) -> Self {
        Self {
            x,
            y,
            max_size,
            stride,
        }
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

        // Side buttons in fixed positions
        let side_button_width = 160;
        let side_button_height = 160;

        // Position buttons higher to make room for enter and gesture buttons
        let left_button_x = 10;
        let right_button_x = window_width - side_button_width - 10;
        let button_start_y = controls_y_start - (4 * side_button_height + 3 * 20); // Position so all 4 buttons fit above controls area
        let button_gap = 20;

        // Left side buttons: Left, Up, A, Enter (4 buttons vertically)
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                left_button_x,
                button_start_y,
                side_button_width,
                side_button_height,
            ),
            stride,
            "←",
        );
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                left_button_x,
                button_start_y + side_button_height + button_gap,
                side_button_width,
                side_button_height,
            ),
            stride,
            "↑",
        );
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                left_button_x,
                button_start_y + 2 * (side_button_height + button_gap),
                side_button_width,
                side_button_height,
            ),
            stride,
            "A",
        );
        // Enter button on left side
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                left_button_x,
                button_start_y + 3 * (side_button_height + button_gap),
                side_button_width,
                side_button_height,
            ),
            stride,
            "⏎",
        );

        // Right side buttons: Right, Down, B, Gesture Toggle (4 buttons vertically)
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                right_button_x,
                button_start_y,
                side_button_width,
                side_button_height,
            ),
            stride,
            "→",
        );
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                right_button_x,
                button_start_y + side_button_height + button_gap,
                side_button_width,
                side_button_height,
            ),
            stride,
            "↓",
        );
        self.draw_button_with_text(
            pixels,
            ButtonRect::new(
                right_button_x,
                button_start_y + 2 * (side_button_height + button_gap),
                side_button_width,
                side_button_height,
            ),
            stride,
            "B",
        );
    }

    fn draw_button_with_text(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        rect: ButtonRect,
        stride: usize,
        label: &str,
    ) {
        // Colors in R5G6B5 format
        let border_color = 0xFFFFu16.to_le_bytes(); // White
        let fill_color = 0x2104u16.to_le_bytes(); // Dark gray

        // Draw button background and border
        for py in 0..rect.height {
            for px in 0..rect.width {
                let screen_x = rect.x + px;
                let screen_y = rect.y + py;
                let pixel_offset = (screen_y * stride + screen_x) * 2;

                if pixel_offset + 1 < pixels.len() {
                    // Draw border (2-pixel wide)
                    let is_border =
                        px < 2 || px >= rect.width - 2 || py < 2 || py >= rect.height - 2;
                    let color = if is_border { border_color } else { fill_color };

                    pixels[pixel_offset].write(color[0]);
                    pixels[pixel_offset + 1].write(color[1]);
                }
            }
        }

        // Try to render emoji first, fallback to PNG if it fails
        let icon_size = 80;
        let icon_x = rect.x + (rect.width - icon_size) / 2;
        let icon_y = rect.y + (rect.height - icon_size) / 2;

        // Use simple ASCII characters that work reliably
        let text = match label {
            "←" => "<",
            "→" => ">",
            "↑" => "^",
            "↓" => "v",
            "A" => "A",
            "B" => "B",
            "⏎" => "E", // E for Enter
            _ => unreachable!(),
        };

        // Render ASCII text
        let mut renderer = TEXT_RENDERER.lock().unwrap();
        let (text_pixels, text_width, text_height) = renderer
            .render_text_to_pixels(text, icon_size as f32)
            .unwrap();

        // Draw text pixels
        let render_ctx = RenderContext::new(icon_x, icon_y, icon_size, stride);
        self.draw_text_pixels(pixels, render_ctx, &text_pixels, text_width, text_height);
    }

    fn draw_text_pixels(
        &self,
        pixels: &mut [std::mem::MaybeUninit<u8>],
        ctx: RenderContext,
        text_pixels: &[u8],
        text_width: u32,
        text_height: u32,
    ) {
        let scale_x = ctx.max_size as f32 / text_width as f32;
        let scale_y = ctx.max_size as f32 / text_height as f32;
        let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

        let scaled_width = (text_width as f32 * scale) as usize;
        let scaled_height = (text_height as f32 * scale) as usize;

        let offset_x = (ctx.max_size - scaled_width) / 2;
        let offset_y = (ctx.max_size - scaled_height) / 2;

        for py in 0..scaled_height {
            for px in 0..scaled_width {
                let src_x = (px as f32 / scale) as usize;
                let src_y = (py as f32 / scale) as usize;

                if src_x < text_width as usize && src_y < text_height as usize {
                    let src_idx = src_y * text_width as usize + src_x;
                    if src_idx < text_pixels.len() {
                        let alpha = text_pixels[src_idx];

                        if alpha > 128 {
                            // Only draw if not transparent
                            // Convert grayscale to white in R5G6B5
                            let intensity = alpha;
                            let r5 = (intensity as u16 * 31 / 255) & 0x1F;
                            let g6 = (intensity as u16 * 63 / 255) & 0x3F;
                            let b5 = (intensity as u16 * 31 / 255) & 0x1F;
                            let rgb565 = (r5 << 11) | (g6 << 5) | b5;
                            let color_bytes = rgb565.to_le_bytes();

                            let screen_x = ctx.x + offset_x + px;
                            let screen_y = ctx.y + offset_y + py;
                            let pixel_offset = (screen_y * ctx.stride + screen_x) * 2;

                            if pixel_offset + 1 < pixels.len() {
                                pixels[pixel_offset].write(color_bytes[0]);
                                pixels[pixel_offset + 1].write(color_bytes[1]);
                            }
                        }
                    }
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
                    let window_width = buffer.width();
                    let window_height = buffer.height();
                    let stride = buffer.stride();

                    // Calculate scaling to fit the 8x32 display in the window
                    let scale_x = window_width / SCREEN_WIDTH;
                    let scale_y = window_height / SCREEN_HEIGHT;
                    let base_scale = scale_x.min(scale_y).max(1);
                    // Use base_scale but limit it to prevent extreme sizes
                    let scale = base_scale.min(100); // Cap at 100x scale to prevent issues
                                                     // debug!(
                                                     //     "📱 Window: {}x{}, stride: {}, scale: {}, base_scale: {}",
                                                     //     window_width, window_height, stride, scale, base_scale
                                                     // );

                    // Center the display in the upper portion, leaving space for controls
                    let display_width = SCREEN_WIDTH * scale;
                    let display_height = SCREEN_HEIGHT * scale;
                    let controls_height = 150; // Smaller controls area to give more space to game
                    let game_area_height = window_height.saturating_sub(controls_height);

                    // Ensure display fits on screen - if too large, it will be clipped but positioned correctly
                    let offset_x = if display_width <= window_width {
                        (window_width - display_width) / 2
                    } else {
                        0 // If display is larger than window, start at left edge
                    };

                    let offset_y = if display_height <= game_area_height {
                        (game_area_height - display_height) / 2
                    } else {
                        0 // If display is larger than available height, start at top
                    };

                    // debug!(
                    //     "🎮 Display: {}x{} at ({}, {}), window: {}x{}, game_area_height: {}",
                    //     display_width, display_height, offset_x, offset_y, window_width, window_height, game_area_height
                    // );

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
        if let Ok(mut led_array) = LEDS.lock() {
            led_array.copy_from_slice(leds);
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

    fn detect_gesture(&self, start: TouchPoint, end: TouchPoint) -> Option<GestureType> {
        // Clear stale last_tap entries that are too old to be part of a double tap
        if let Ok(mut last_tap_guard) = GESTURE_STATE.last_tap.write() {
            if let Some(last_tap) = *last_tap_guard {
                let time_since_last_tap =
                    end.timestamp.duration_since(last_tap.timestamp).as_millis() as u64;
                if time_since_last_tap > DOUBLE_TAP_MAX_TIME_MS * 2 {
                    *last_tap_guard = None;
                    info!("🧹 Cleared stale last_tap ({}ms old)", time_since_last_tap);
                }
            }
        }

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let distance = (dx * dx + dy * dy).sqrt();
        let time_diff = end.timestamp.duration_since(start.timestamp).as_millis() as u64;

        // Check for tap (short press with minimal movement)
        if distance < TAP_MAX_DISTANCE && time_diff < MAX_SWIPE_TIME_MS {
            // Check for double tap by comparing with last tap
            if let Ok(last_tap_guard) = GESTURE_STATE.last_tap.read() {
                if let Some(last_tap) = *last_tap_guard {
                    let time_since_last_tap =
                        end.timestamp.duration_since(last_tap.timestamp).as_millis() as u64;
                    let distance_from_last_tap =
                        ((end.x - last_tap.x).powi(2) + (end.y - last_tap.y).powi(2)).sqrt();

                    info!("🔍 Checking double tap: time_diff={}ms (max={}ms), distance={:.1}px (max={:.1}px)", 
                          time_since_last_tap, DOUBLE_TAP_MAX_TIME_MS, distance_from_last_tap, DOUBLE_TAP_MAX_DISTANCE);

                    // If this tap is close enough in time and space to the last tap, it's a double tap
                    if time_since_last_tap <= DOUBLE_TAP_MAX_TIME_MS
                        && distance_from_last_tap <= DOUBLE_TAP_MAX_DISTANCE
                    {
                        info!("✅ Double tap detected! Clearing last_tap state.");
                        // Clear the last tap to prevent triple taps from being detected as double taps
                        drop(last_tap_guard);
                        if let Ok(mut last_tap_write) = GESTURE_STATE.last_tap.write() {
                            *last_tap_write = None;
                        }
                        return Some(GestureType::DoubleTap);
                    } else {
                        info!("❌ Not a double tap - conditions not met");
                    }
                } else {
                    info!("🔍 No previous tap found for double tap check");
                }
            }

            // Store this tap as the last tap for potential double tap detection
            if let Ok(mut last_tap_guard) = GESTURE_STATE.last_tap.write() {
                *last_tap_guard = Some(end);
                info!(
                    "💾 Stored tap at ({:.1}, {:.1}) for double tap detection",
                    end.x, end.y
                );
            }

            return Some(GestureType::Tap);
        }

        // Check for long press
        if distance < TAP_MAX_DISTANCE && time_diff >= LONG_PRESS_TIME_MS {
            return Some(GestureType::LongPress);
        }

        // Check for swipe gestures
        if distance >= MIN_SWIPE_DISTANCE && time_diff <= MAX_SWIPE_TIME_MS {
            let abs_dx = dx.abs();
            let abs_dy = dy.abs();

            // Determine primary direction
            if abs_dx > abs_dy {
                // Horizontal swipe
                if dx > 0.0 {
                    return Some(GestureType::SwipeRight);
                } else {
                    return Some(GestureType::SwipeLeft);
                }
            } else {
                // Vertical swipe
                if dy > 0.0 {
                    return Some(GestureType::SwipeDown);
                } else {
                    return Some(GestureType::SwipeUp);
                }
            }
        }

        None
    }

    fn handle_gesture(&self, gesture: GestureType) {
        info!("🎯 Setting gesture input: {:?}", gesture);

        match gesture {
            GestureType::SwipeLeft => {
                INPUT_STATE.x_input.store(-1, Ordering::Relaxed);
                INPUT_STATE.gesture_x_pending.store(true, Ordering::Relaxed);
                info!("✅ Set x_input=-1, gesture_x_pending=true");
            }
            GestureType::SwipeRight => {
                INPUT_STATE.x_input.store(1, Ordering::Relaxed);
                INPUT_STATE.gesture_x_pending.store(true, Ordering::Relaxed);
                info!("✅ Set x_input=1, gesture_x_pending=true");
            }
            GestureType::SwipeUp => {
                INPUT_STATE.y_input.store(-1, Ordering::Relaxed);
                INPUT_STATE.gesture_y_pending.store(true, Ordering::Relaxed);
                info!("✅ Set y_input=-1, gesture_y_pending=true");
            }
            GestureType::SwipeDown => {
                INPUT_STATE.y_input.store(1, Ordering::Relaxed);
                INPUT_STATE.gesture_y_pending.store(true, Ordering::Relaxed);
                info!("✅ Set y_input=1, gesture_y_pending=true");
            }
            GestureType::Tap => {
                INPUT_STATE.joystick_pressed.store(true, Ordering::Relaxed);
                INPUT_STATE
                    .gesture_joystick_pending
                    .store(true, Ordering::Relaxed);
                info!("✅ Set joystick_pressed=true, gesture_joystick_pending=true");
            }
            GestureType::LongPress => {
                INPUT_STATE.a_pressed.store(true, Ordering::Relaxed);
                INPUT_STATE.gesture_a_pending.store(true, Ordering::Relaxed);
                info!("✅ Set a_pressed=true, gesture_a_pending=true");
            }
            GestureType::DoubleTap => {
                INPUT_STATE.b_pressed.store(true, Ordering::Relaxed);
                INPUT_STATE.gesture_b_pending.store(true, Ordering::Relaxed);
                info!("✅ Set b_pressed=true, gesture_b_pending=true");
            }
        }
    }

    fn is_in_game_area(&self, x: f32, y: f32) -> bool {
        // Check if touch is actually within the rendered LED display bounds
        if let Some(native_window) = self.app.native_window() {
            let window_width = native_window.width() as usize;
            let window_height = native_window.height() as usize;

            // Calculate the same scaling and positioning as in render_to_native_window
            let scale_x = window_width / SCREEN_WIDTH;
            let scale_y = window_height / SCREEN_HEIGHT;
            let base_scale = scale_x.min(scale_y).max(1);
            let scale = base_scale.min(100);

            let display_width = SCREEN_WIDTH * scale;
            let display_height = SCREEN_HEIGHT * scale;
            let controls_height = 150;
            let game_area_height = window_height.saturating_sub(controls_height);

            let offset_x = if display_width <= window_width {
                (window_width - display_width) / 2
            } else {
                0
            };

            let offset_y = if display_height <= game_area_height {
                (game_area_height - display_height) / 2
            } else {
                0
            };

            // Check if touch is within the actual LED display bounds
            let display_left = offset_x as f32;
            let display_right = (offset_x + display_width) as f32;
            let display_top = offset_y as f32;
            let display_bottom = (offset_y + display_height) as f32;

            x >= display_left && x <= display_right && y >= display_top && y <= display_bottom
        } else {
            false
        }
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

            // Check side buttons using same positions as drawing
            let side_button_width = 160;
            let side_button_height = 160;
            let left_button_x = 10;
            let right_button_x = window_width - side_button_width - 10;
            let button_start_y = controls_y_start - (4 * side_button_height + 3 * 20); // Position so all 4 buttons fit above controls area
            let button_gap = 20;

            // Left side buttons: Left, Up, A, Enter (4 buttons vertically)
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
            // Enter button (4th button on left side)
            if x >= left_button_x
                && x < left_button_x + side_button_width
                && y >= button_start_y + 3 * (side_button_height + button_gap)
                && y < button_start_y + 3 * (side_button_height + button_gap) + side_button_height
            {
                INPUT_STATE.joystick_pressed.store(true, Ordering::Relaxed); // Enter
            }

            // Right side buttons: Right, Down, B, Gesture Toggle (4 buttons vertically)
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
                                    let x = pointer.x();
                                    let y = pointer.y();
                                    let now = std::time::Instant::now();

                                    match motion_event.action() {
                                        MotionAction::Down => {
                                            let touch_point = TouchPoint {
                                                x,
                                                y,
                                                timestamp: now,
                                            };

                                            // Store touch start for gesture detection
                                            if let Ok(mut start) = GESTURE_STATE.touch_start.write()
                                            {
                                                *start = Some(touch_point);
                                            }
                                            if let Ok(mut last) = GESTURE_STATE.last_touch.write() {
                                                *last = Some(touch_point);
                                            }

                                            // Track whether this gesture started in the LED display area
                                            let started_in_led = self.is_in_game_area(x, y);
                                            if let Ok(mut started) =
                                                GESTURE_STATE.started_in_game_area.write()
                                            {
                                                *started = started_in_led;
                                            }

                                            // Handle button presses only if touch started outside LED display area
                                            if !started_in_led {
                                                self.handle_touch_input(x as usize, y as usize);
                                            }
                                            true
                                        }
                                        MotionAction::Move => {
                                            let touch_point = TouchPoint {
                                                x,
                                                y,
                                                timestamp: now,
                                            };

                                            // Update last touch for gesture tracking
                                            if let Ok(mut last) = GESTURE_STATE.last_touch.write() {
                                                *last = Some(touch_point);
                                            }

                                            // Handle continuous button presses only if gesture didn't start in LED area
                                            if let Ok(started) =
                                                GESTURE_STATE.started_in_game_area.read()
                                            {
                                                if !*started {
                                                    self.handle_touch_input(x as usize, y as usize);
                                                }
                                            }
                                            true
                                        }
                                        MotionAction::Up => {
                                            let touch_point = TouchPoint {
                                                x,
                                                y,
                                                timestamp: now,
                                            };

                                            // Check if gesture started in game area BEFORE we clear state
                                            let started_in_led = if let Ok(started) =
                                                GESTURE_STATE.started_in_game_area.read()
                                            {
                                                *started
                                            } else {
                                                false
                                            };

                                            // Always try to detect gesture everywhere
                                            if let Ok(start_guard) =
                                                GESTURE_STATE.touch_start.read()
                                            {
                                                if let Some(start) = *start_guard {
                                                    if let Some(gesture) =
                                                        self.detect_gesture(start, touch_point)
                                                    {
                                                        // Only apply gesture if it started in LED display area
                                                        if started_in_led {
                                                            self.handle_gesture(gesture);
                                                        }
                                                    }
                                                }
                                            }

                                            // Clear gesture state
                                            if let Ok(mut start) = GESTURE_STATE.touch_start.write()
                                            {
                                                *start = None;
                                            }
                                            if let Ok(mut last) = GESTURE_STATE.last_touch.write() {
                                                *last = None;
                                            }
                                            if let Ok(mut started) =
                                                GESTURE_STATE.started_in_game_area.write()
                                            {
                                                *started = false;
                                            }

                                            // Only clear button input states if gesture didn't start in LED area
                                            if !started_in_led {
                                                INPUT_STATE.x_input.store(0, Ordering::Relaxed);
                                                INPUT_STATE.y_input.store(0, Ordering::Relaxed);
                                                INPUT_STATE
                                                    .joystick_pressed
                                                    .store(false, Ordering::Relaxed);
                                                INPUT_STATE
                                                    .a_pressed
                                                    .store(false, Ordering::Relaxed);
                                                INPUT_STATE
                                                    .b_pressed
                                                    .store(false, Ordering::Relaxed);
                                            }
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
        // Only process events once per 10ms to avoid race conditions
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last_time = LAST_INPUT_PROCESS_TIME.load(Ordering::Relaxed);

        if now - last_time > 10 {
            self.process_input_events();
            LAST_INPUT_PROCESS_TIME.store(now, Ordering::Relaxed);
        }

        let value = INPUT_STATE.x_input.load(Ordering::Relaxed);

        // Clear gesture input after reading if it was set by a gesture
        if INPUT_STATE.gesture_x_pending.swap(false, Ordering::Relaxed) {
            info!("📖 Reading x_input={}, clearing gesture", value);
            INPUT_STATE.x_input.store(0, Ordering::Relaxed);
        } else if value != 0 {
            info!("📖 Reading x_input={} (button input)", value);
        }

        value
    }

    async fn read_y(&mut self) -> i8 {
        let value = INPUT_STATE.y_input.load(Ordering::Relaxed);

        // Clear gesture input after reading if it was set by a gesture
        if INPUT_STATE.gesture_y_pending.swap(false, Ordering::Relaxed) {
            info!("📖 Reading y_input={}, clearing gesture", value);
            INPUT_STATE.y_input.store(0, Ordering::Relaxed);
        } else if value != 0 {
            info!("📖 Reading y_input={} (button input)", value);
        }

        value
    }

    fn joystick_was_pressed(&self) -> bool {
        let current = INPUT_STATE.joystick_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE
            .prev_joystick_pressed
            .swap(current, Ordering::Relaxed);

        // Clear gesture input after reading if it was set by a gesture
        if INPUT_STATE
            .gesture_joystick_pending
            .swap(false, Ordering::Relaxed)
        {
            INPUT_STATE.joystick_pressed.store(false, Ordering::Relaxed);
        }

        current && !prev
    }

    fn a_was_pressed(&self) -> bool {
        let current = INPUT_STATE.a_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE.prev_a_pressed.swap(current, Ordering::Relaxed);

        // Clear gesture input after reading if it was set by a gesture
        if INPUT_STATE.gesture_a_pending.swap(false, Ordering::Relaxed) {
            INPUT_STATE.a_pressed.store(false, Ordering::Relaxed);
        }

        current && !prev
    }

    fn b_was_pressed(&self) -> bool {
        let current = INPUT_STATE.b_pressed.load(Ordering::Relaxed);
        let prev = INPUT_STATE.prev_b_pressed.swap(current, Ordering::Relaxed);

        // Clear gesture input after reading if it was set by a gesture
        if INPUT_STATE.gesture_b_pending.swap(false, Ordering::Relaxed) {
            INPUT_STATE.b_pressed.store(false, Ordering::Relaxed);
        }

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
