use smart_leds::RGB8;
use tetris_lib::{
    common::{GameController, LedDisplay, Timer, SCREEN_HEIGHT, SCREEN_WIDTH},
    games::run_game_menu,
};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent};

// Console logging macro (currently unused but may be useful for debugging)
#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

// Timer implementation for WASM
pub struct WasmTimer;

impl Timer for WasmTimer {
    async fn sleep_millis(&self, millis: u64) {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let window = web_sys::window().unwrap();
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis as i32)
                .unwrap();
        });
        wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    }
}

// Display implementation for WASM
pub struct WasmDisplay {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

impl WasmDisplay {
    pub fn new(canvas: HtmlCanvasElement, pixel_size: f64) -> Result<Self, JsValue> {
        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Set canvas size
        canvas.set_width((SCREEN_WIDTH as f64 * pixel_size) as u32);
        canvas.set_height((SCREEN_HEIGHT as f64 * pixel_size) as u32);

        Ok(Self { canvas, context })
    }
}

impl LedDisplay for WasmDisplay {
    async fn write(&mut self, leds: &[RGB8; 256]) {
        let width = SCREEN_WIDTH as u32;
        let height = SCREEN_HEIGHT as u32;

        // Create image data
        let mut data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let led_idx = if y % 2 == 0 {
                    y * width + (width - 1 - x)
                } else {
                    y * width + x
                } as usize;

                let led = leds[led_idx];
                data.push(led.r * 8); // Scale up from 0-31 to 0-248
                data.push(led.g * 8);
                data.push(led.b * 8);
                data.push(255); // Alpha
            }
        }

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&data),
            width,
            height,
        )
        .unwrap();

        // Clear the canvas
        self.context.clear_rect(
            0.0,
            0.0,
            self.canvas.width() as f64,
            self.canvas.height() as f64,
        );

        // Disable image smoothing for pixel-perfect scaling
        self.context.set_image_smoothing_enabled(false);

        // Scale the 8x32 pixel image to fill the entire canvas
        self.context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        // Create a temporary canvas for scaling
        let temp_canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        temp_canvas.set_width(width);
        temp_canvas.set_height(height);

        let temp_context = temp_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        temp_context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        // Scale from temporary canvas to main canvas
        self.context
            .draw_image_with_html_canvas_element_and_dw_and_dh(
                &temp_canvas,
                0.0,
                0.0,
                self.canvas.width() as f64,
                self.canvas.height() as f64,
            )
            .unwrap();
    }
}

// Global input state to avoid aliasing issues
static mut INPUT_STATE: InputState = InputState {
    x_input: 0,
    y_input: 0,
    joystick_pressed: false,
    a_pressed: false,
    b_pressed: false,
    prev_joystick_pressed: false,
    prev_a_pressed: false,
    prev_b_pressed: false,
};

#[derive(Default)]
struct InputState {
    x_input: i8,
    y_input: i8,
    joystick_pressed: bool,
    a_pressed: bool,
    b_pressed: bool,
    prev_joystick_pressed: bool,
    prev_a_pressed: bool,
    prev_b_pressed: bool,
}

// Controller implementation for WASM
pub struct WasmController;

impl WasmController {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_key_down(event: &KeyboardEvent) {
        unsafe {
            match event.key().as_str() {
                "ArrowLeft" | "a" | "A" => INPUT_STATE.x_input = -1,
                "ArrowRight" | "d" | "D" => INPUT_STATE.x_input = 1,
                "ArrowUp" | "w" | "W" => INPUT_STATE.y_input = -1,
                "ArrowDown" | "s" | "S" => INPUT_STATE.y_input = 1,
                "Enter" | " " => INPUT_STATE.joystick_pressed = true,
                "q" | "Q" => INPUT_STATE.a_pressed = true,
                "e" | "E" => INPUT_STATE.b_pressed = true,
                _ => {}
            }
        }
    }

    pub fn handle_key_up(event: &KeyboardEvent) {
        unsafe {
            match event.key().as_str() {
                "ArrowLeft" | "ArrowRight" | "a" | "A" | "d" | "D" => INPUT_STATE.x_input = 0,
                "ArrowUp" | "ArrowDown" | "w" | "W" | "s" | "S" => INPUT_STATE.y_input = 0,
                "Enter" | " " => INPUT_STATE.joystick_pressed = false,
                "q" | "Q" => INPUT_STATE.a_pressed = false,
                "e" | "E" => INPUT_STATE.b_pressed = false,
                _ => {}
            }
        }
    }
}

impl GameController for WasmController {
    async fn read_x(&mut self) -> i8 {
        unsafe { INPUT_STATE.x_input }
    }

    async fn read_y(&mut self) -> i8 {
        unsafe { INPUT_STATE.y_input }
    }

    fn joystick_was_pressed(&self) -> bool {
        unsafe {
            let pressed = INPUT_STATE.joystick_pressed && !INPUT_STATE.prev_joystick_pressed;
            // Update previous state for next check
            INPUT_STATE.prev_joystick_pressed = INPUT_STATE.joystick_pressed;
            pressed
        }
    }

    fn a_was_pressed(&self) -> bool {
        unsafe {
            let pressed = INPUT_STATE.a_pressed && !INPUT_STATE.prev_a_pressed;
            // Update previous state for next check
            INPUT_STATE.prev_a_pressed = INPUT_STATE.a_pressed;
            pressed
        }
    }

    fn b_was_pressed(&self) -> bool {
        unsafe {
            let pressed = INPUT_STATE.b_pressed && !INPUT_STATE.prev_b_pressed;
            // Update previous state for next check
            INPUT_STATE.prev_b_pressed = INPUT_STATE.b_pressed;
            pressed
        }
    }
}

// This was already replaced above with static functions, so this section should be removed

// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

// Export the start_game function for JavaScript
#[wasm_bindgen]
pub async fn start_game(canvas: HtmlCanvasElement, pixel_size: f64) -> Result<(), JsValue> {
    // Create display
    let mut display = WasmDisplay::new(canvas, pixel_size)?;

    // Create controller and timer
    let mut controller = WasmController::new();
    let timer = WasmTimer;

    // Seed function using current timestamp
    let seed_fn = || js_sys::Date::now() as u32;

    // Run the game menu
    run_game_menu(&mut display, &mut controller, &timer, seed_fn).await;

    Ok(())
}

// Export key handling functions for JavaScript
#[wasm_bindgen]
pub fn handle_key_down(event: KeyboardEvent) {
    WasmController::handle_key_down(&event);
}

#[wasm_bindgen]
pub fn handle_key_up(event: KeyboardEvent) {
    WasmController::handle_key_up(&event);
}
