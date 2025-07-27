use mouse_position::mouse_position::Mouse;
use tauri::Window;

const WINDOW_RANGE: &[&f32] = &[&0.05, &0.95, &0.1, &0.75];
const SHOWN_RANGE: &[&f32] = &[&0.0, &1.0, &0.75, &1.0];
const HIDDEN_RANGE: &[&f32] = &[&0.0, &0.075, &0.75, &0.825];

#[tauri::command]
pub fn check_cursor_region(win: Window, hidden: bool) -> Result<(f32, f32, bool), String> {
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => {
            let winp = win.inner_position().map_err(|e| e.to_string())?;
            let wins = win.inner_size().map_err(|e| e.to_string())?;

            let px = (x - winp.x) as f32 / (wins.width as f32);
            let py = (y - winp.y) as f32 / (wins.height as f32);

            let current_range: &[&f32] = if hidden { HIDDEN_RANGE } else { SHOWN_RANGE };

            let inside_region = !(px >= *current_range[0]
                && px <= *current_range[1]
                && py >= *current_range[2]
                && py <= *current_range[3]);

            win.set_ignore_cursor_events(inside_region).ok();
            if !hidden {
                let over_window = (px >= *WINDOW_RANGE[0]
                    && px <= *WINDOW_RANGE[1]
                    && py >= *WINDOW_RANGE[2]
                    && py < *WINDOW_RANGE[3])
                    && inside_region;
                Ok((px - 0.5, py - 0.5, over_window))
            } else {
                Ok((px - 0.5, py - 0.5, false))
            }
        }
        Mouse::Error => Err("Error getting mouse position".to_string()),
    }
}
