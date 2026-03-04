mod canvas;
mod renderer;
use canvas::Canvas;
use rdev::{listen, Button, Event, EventType};
use renderer::Renderer;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

struct TargetZoom(Arc<Mutex<f32>>);
struct TargetRotation(Arc<Mutex<f32>>);
struct CurrentRotation(Arc<Mutex<f32>>);

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

#[tauri::command]
fn zoom_in(state: tauri::State<'_, TargetZoom>) {
    *state.0.lock().unwrap() *= 1.2;
}

#[tauri::command]
fn zoom_out(state: tauri::State<'_, TargetZoom>) {
    *state.0.lock().unwrap() /= 1.2;
}

#[tauri::command]
fn set_zoom(value: f32, state: tauri::State<'_, TargetZoom>) {
    *state.0.lock().unwrap() = value;
}

#[tauri::command]
fn rotate_cw(state: tauri::State<'_, TargetRotation>) {
    *state.0.lock().unwrap() += std::f32::consts::PI / 12.0;
}

#[tauri::command]
fn rotate_ccw(state: tauri::State<'_, TargetRotation>) {
    *state.0.lock().unwrap() -= std::f32::consts::PI / 12.0;
}

#[tauri::command]
fn set_rotation(
    _value: f32,
    target: tauri::State<'_, TargetRotation>,
    current: tauri::State<'_, CurrentRotation>,
) {
    let tau = std::f32::consts::TAU;
    let cur = *current.0.lock().unwrap();

    // normalizuj na [-π, π] da animacija ide najkraćim putem
    let normalized = cur - tau * (cur / tau).round();

    *current.0.lock().unwrap() = normalized;
    *target.0.lock().unwrap() = 0.0;
}

fn main() {
    let canvas = Arc::new(Mutex::new(Canvas::new()));

    let zoom = Arc::new(Mutex::new(1.0f32));
    let target_zoom = Arc::new(Mutex::new(1.0f32));
    let rotation = Arc::new(Mutex::new(0.0f32));
    let target_rotation = Arc::new(Mutex::new(0.0f32));

    tauri::Builder::default()
        .manage(canvas.clone())
        .manage(TargetZoom(target_zoom.clone()))
        .manage(TargetRotation(target_rotation.clone()))
        .manage(CurrentRotation(rotation.clone()))
        .invoke_handler(tauri::generate_handler![
            clear_canvas,
            set_color,
            set_pen_size,
            frontend_ready,
            zoom_in,
            zoom_out,
            set_zoom,
            rotate_cw,
            rotate_ccw,
            set_rotation,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            let scale_factor = window.scale_factor().unwrap_or(1.0) as f32;

            let renderer =
                pollster::block_on(Renderer::new(&window, zoom.clone(), rotation.clone()));
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
            {
                let zoom_anim = zoom.clone();
                let target_anim = target_zoom.clone();
                let rotation_anim = rotation.clone();
                let target_anim_r = target_rotation.clone();
                let renderer_anim = renderer.clone();
                let canvas_anim = canvas.clone();

                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_millis(16));

                    let target_z = *target_anim.lock().unwrap();
                    let current_z = *zoom_anim.lock().unwrap();
                    let diff_z = target_z - current_z;

                    let target_r = *target_anim_r.lock().unwrap();
                    let current_r = *rotation_anim.lock().unwrap();
                    let diff_r = target_r - current_r;

                    if diff_z.abs() < 0.0005 && diff_r.abs() < 0.0001 {
                        continue;
                    }

                    if diff_z.abs() >= 0.0005 {
                        *zoom_anim.lock().unwrap() = current_z + diff_z * 0.18;
                    }

                    if diff_r.abs() >= 0.0001 {
                        *rotation_anim.lock().unwrap() = current_r + diff_r * 0.18;
                    }

                    let c = canvas_anim.lock().unwrap();
                    renderer_anim.lock().unwrap().render(&c).unwrap_or_default();
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

                        let (zoom, rot, w, h) = {
                            let r = renderer_mouse.lock().unwrap();
                            let zoom_guard = r.zoom.lock().unwrap();
                            let rot_guard = r.rotation.lock().unwrap();
                            let zoom = *zoom_guard;
                            let rot = *rot_guard;
                            let w = r.config.width as f32;
                            let h = r.config.height as f32;
                            (zoom, rot, w, h)
                        };

                        let aspect = w / h;

                        let nx = (local_x / w - 0.5) * aspect;
                        let ny = local_y / h - 0.5;

                        let (s, c) = (rot.sin(), rot.cos());
                        let un_rot_x = nx * c + ny * s;
                        let un_rot_y = -nx * s + ny * c;

                        let canvas_x = (un_rot_x / aspect / zoom + 0.5) * w;
                        let canvas_y = (un_rot_y / zoom + 0.5) * h;

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
