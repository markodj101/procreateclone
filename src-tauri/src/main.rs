mod canvas;
mod renderer;
use canvas::Canvas;
use rdev::{listen, Button, Event, EventType};
use renderer::Renderer;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[tauri::command]
fn frontend_ready(window: tauri::WebviewWindow) {
    window.show().unwrap();
}

#[tauri::command]
fn clear_canvas(
    state: tauri::State<'_, Arc<Mutex<Canvas>>>,
    renderer: tauri::State<'_, Arc<Mutex<Renderer>>>,
) {
    let mut c = state.lock().unwrap();
    c.clear();
    renderer.lock().unwrap().render(&c).unwrap_or_default();
}

#[tauri::command]
fn set_color(r: f32, g: f32, b: f32, state: tauri::State<'_, Arc<Mutex<Canvas>>>) {
    state.lock().unwrap().pen_color = [r, g, b, 1.0];
}

#[tauri::command]
fn set_pen_size(size: f32, state: tauri::State<'_, Arc<Mutex<Canvas>>>) {
    state.lock().unwrap().pen_size = size;
}

fn main() {
    let canvas = Arc::new(Mutex::new(Canvas::new()));

    tauri::Builder::default()
        .manage(canvas.clone())
        .invoke_handler(tauri::generate_handler![
            clear_canvas,
            set_color,
            set_pen_size,
            frontend_ready,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            // Init wgpu
            let renderer = pollster::block_on(Renderer::new(&window));
            let c = canvas.lock().unwrap();
            renderer.render(&c).unwrap_or_default();
            drop(c);

            let renderer = Arc::new(Mutex::new(renderer));
            app.manage(renderer.clone());

            // ✅ inner_position — bez title bara
            let window_pos = window.inner_position().unwrap_or_default();
            let window_pos = Arc::new(Mutex::new((window_pos.x as f64, window_pos.y as f64)));

            let wp_move = window_pos.clone();
            let app_handle = app.handle().clone();

            window.on_window_event(move |event| {
                match event {
                    tauri::WindowEvent::Resized(size) => {
                        if let Some(r) = app_handle.try_state::<Arc<Mutex<Renderer>>>() {
                            r.lock().unwrap().resize(size.width, size.height);
                        }
                    }
                    // ✅ Prati pomak prozora
                    tauri::WindowEvent::Moved(pos) => {
                        let mut p = wp_move.lock().unwrap();
                        *p = (pos.x as f64, pos.y as f64);
                    }
                    _ => {}
                }
            });

            // ✅ Prati je li tipka fizički držana odvojeno od is_drawing
            let mouse_held = Arc::new(Mutex::new(false));
            let mouse_held_clone = mouse_held.clone();

            let canvas_mouse = canvas.clone();
            let renderer_mouse = renderer.clone();
            let window_pos_clone = window_pos.clone();

            std::thread::spawn(move || {
                listen(move |event: Event| {
                    match event.event_type {
                        EventType::MouseMove { x, y } => {
                            let (wx, wy) = *window_pos_clone.lock().unwrap();
                            let local_x = (x - wx) as f32;
                            let local_y = (y - wy) as f32;

                            let held = *mouse_held_clone.lock().unwrap();

                            // ✅ Van prozora — SAČUVAJ trenutni potez pa zaustavi
                            if local_x < 0.0 || local_y < 0.0 {
                                canvas_mouse.lock().unwrap().end_stroke(); // ← ovo je bila greška
                                return;
                            }

                            {
                                let mut c = canvas_mouse.lock().unwrap();

                                // Vratio se u prozor sa stisnutim mišem — počni novi potez
                                if held && !c.is_drawing {
                                    c.start_stroke(local_x, local_y);
                                } else {
                                    c.add_point(local_x, local_y);
                                }
                            }

                            let c = canvas_mouse.lock().unwrap();
                            renderer_mouse
                                .lock()
                                .unwrap()
                                .render(&c)
                                .unwrap_or_default();
                        }

                        // ✅ Lijevi klik pritisnut
                        EventType::ButtonPress(Button::Left) => {
                            *mouse_held_clone.lock().unwrap() = true;
                            let mut c = canvas_mouse.lock().unwrap();
                            let pos = c.cursor_pos;
                            c.start_stroke(pos[0], pos[1]);
                        }

                        // ✅ Lijevi klik pušten
                        EventType::ButtonRelease(Button::Left) => {
                            *mouse_held_clone.lock().unwrap() = false;
                            canvas_mouse.lock().unwrap().end_stroke();
                        }

                        _ => {}
                    }
                })
                .unwrap();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap();
}
