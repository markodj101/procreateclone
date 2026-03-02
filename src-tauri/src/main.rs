mod canvas;
mod renderer;
use canvas::Canvas;
use rdev::{listen, Button, Event, EventType};
use renderer::Renderer;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

// Komande samo postavljaju TARGET — animacioni thread radi ostalo
#[tauri::command]
fn zoom_in(target_zoom: tauri::State<'_, Arc<Mutex<f32>>>) {
    let mut t = target_zoom.lock().unwrap();
    *t *= 1.2;
    println!("Target zoom in: {}", *t);
}

#[tauri::command]
fn zoom_out(target_zoom: tauri::State<'_, Arc<Mutex<f32>>>) {
    let mut t = target_zoom.lock().unwrap();
    *t /= 1.2;
    println!("Target zoom out: {}", *t);
}

#[tauri::command]
fn set_zoom(value: f32, target_zoom: tauri::State<'_, Arc<Mutex<f32>>>) {
    let mut t = target_zoom.lock().unwrap();
    *t = value;
    println!("Target zoom set: {}", *t);
}

fn main() {
    let canvas = Arc::new(Mutex::new(Canvas::new()));

    // Trenutni zoom koji renderer koristi
    let zoom = Arc::new(Mutex::new(1.0f32));

    // Ciljani zoom — komande menjaju samo ovo
    let target_zoom = Arc::new(Mutex::new(1.0f32));

    tauri::Builder::default()
        .manage(canvas.clone())
        .manage(target_zoom.clone()) // ← upravljamo target_zoom-om
        .invoke_handler(tauri::generate_handler![
            clear_canvas,
            set_color,
            set_pen_size,
            frontend_ready,
            zoom_in,
            zoom_out,
            set_zoom,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            let scale_factor = window.scale_factor().unwrap_or(1.0) as f32;
            println!("Scale factor: {}", scale_factor);

            let renderer = pollster::block_on(Renderer::new(&window, zoom.clone()));
            let c = canvas.lock().unwrap();
            renderer.render(&c).unwrap_or_default();
            drop(c);

            let renderer = Arc::new(Mutex::new(renderer));
            app.manage(renderer.clone());

            let window_pos = window.inner_position().unwrap_or_default();
            let window_pos = Arc::new(Mutex::new((window_pos.x as f64, window_pos.y as f64)));

            let wp_move = window_pos.clone();
            let app_handle = app.handle().clone();

            window.on_window_event(move |event| match event {
                tauri::WindowEvent::Resized(size) => {
                    if let Some(r) = app_handle.try_state::<Arc<Mutex<Renderer>>>() {
                        r.lock().unwrap().resize(size.width, size.height);
                    }
                }
                tauri::WindowEvent::Moved(pos) => {
                    let mut p = wp_move.lock().unwrap();
                    *p = (pos.x as f64, pos.y as f64);
                }
                _ => {}
            });

            // ── Animacioni thread ──────────────────────────────────────────
            // Radi na ~60fps, interpolira zoom → target_zoom sa ease-out
            {
                let zoom_anim = zoom.clone();
                let target_anim = target_zoom.clone();
                let renderer_anim = renderer.clone();
                let canvas_anim = canvas.clone();

                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_millis(16)); // ~60fps

                        let target = *target_anim.lock().unwrap();
                        let current = *zoom_anim.lock().unwrap();

                        let diff = target - current;

                        // Ako smo dovoljno blizu — zaustavljamo animaciju
                        if diff.abs() < 0.0005 {
                            if (current - target).abs() > 0.00001 {
                                *zoom_anim.lock().unwrap() = target;
                                let c = canvas_anim.lock().unwrap();
                                renderer_anim.lock().unwrap().render(&c).unwrap_or_default();
                            }
                            continue;
                        }

                        // Ease-out: pomeri 18% razlike svaki frame
                        // Što bliže cilju — sporije se kreće → prirodan ease-out
                        let new_zoom = current + diff * 0.18;
                        *zoom_anim.lock().unwrap() = new_zoom;

                        let c = canvas_anim.lock().unwrap();
                        renderer_anim.lock().unwrap().render(&c).unwrap_or_default();
                    }
                });
            }
            // ──────────────────────────────────────────────────────────────

            let mouse_held = Arc::new(Mutex::new(false));
            let mouse_held_clone = mouse_held.clone();

            let canvas_mouse = canvas.clone();
            let renderer_mouse = renderer.clone();
            let window_pos_clone = window_pos.clone();

            std::thread::spawn(move || {
                listen(move |event: Event| match event.event_type {
                    EventType::MouseMove { x, y } => {
                        let (wx, wy) = *window_pos_clone.lock().unwrap();

                        let local_x = (x * scale_factor as f64 - wx) as f32;
                        let local_y = (y * scale_factor as f64 - wy) as f32;

                        let held = *mouse_held_clone.lock().unwrap();

                        if local_x < 0.0 || local_y < 0.0 {
                            canvas_mouse.lock().unwrap().end_stroke();
                            return;
                        }

                        // Inverzna shader transformacija (zoom od centra)
                        let (zoom, w, h) = {
                            let r = renderer_mouse.lock().unwrap();
                            let vals = (
                                *r.zoom.lock().unwrap(),
                                r.config.width as f32,
                                r.config.height as f32,
                            );
                            vals
                        };
                        let cx = w / 2.0;
                        let cy = h / 2.0;
                        let canvas_x = (local_x - cx) / zoom + cx;
                        let canvas_y = (local_y - cy) / zoom + cy;

                        {
                            let mut c = canvas_mouse.lock().unwrap();
                            if held && !c.is_drawing {
                                c.start_stroke(canvas_x, canvas_y);
                            } else {
                                c.add_point(canvas_x, canvas_y);
                            }
                        }

                        let c = canvas_mouse.lock().unwrap();
                        renderer_mouse
                            .lock()
                            .unwrap()
                            .render(&c)
                            .unwrap_or_default();
                    }

                    EventType::ButtonPress(Button::Left) => {
                        *mouse_held_clone.lock().unwrap() = true;
                        let mut c = canvas_mouse.lock().unwrap();
                        let canvas_x = c.cursor_pos[0];
                        let canvas_y = c.cursor_pos[1];
                        c.start_stroke(canvas_x, canvas_y);
                    }

                    EventType::ButtonRelease(Button::Left) => {
                        *mouse_held_clone.lock().unwrap() = false;
                        canvas_mouse.lock().unwrap().end_stroke();
                    }

                    _ => {}
                })
                .unwrap();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap();
}
