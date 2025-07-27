use mouse_position::mouse_position::Mouse;
use tauri::Window;

//x, y, w, h, offset x
const WINDOW_POLY: &[&f32] = &[&0.15, &0.04, &0.7, &0.61, &0.0];
const SHOWN_POLY: &[&f32] = &[&0.04, &0.65, &0.96, &0.35, &0.04];
const HIDDEN_POLY: &[&f32] = &[&0.06, &0.67, &0.06, &0.06, &0.0];

#[tauri::command]
pub fn check_cursor_region(win: Window, hidden: bool) -> Result<(f32, f32, bool), String> {
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => {
            let winp = win.inner_position().map_err(|e| e.to_string())?;
            let wins = win.inner_size().map_err(|e| e.to_string())?;

            let px = (x - winp.x) as f32 / (wins.width as f32);
            let py = (y - winp.y) as f32 / (wins.height as f32);

            let current_poly: &[&f32] = if hidden { HIDDEN_POLY } else { SHOWN_POLY };
            let _ = win.set_ignore_cursor_events(!in_region(px, py, current_poly));

            if !hidden {
                Ok((px - 0.5, py - 0.5, in_region(px, py, WINDOW_POLY)))
            } else {
                Ok((px - 0.5, py - 0.5, false))
            }

            /*let inside_region = !(px >= *current_range[0]
                && px <= *current_range[1]
                && py >= *current_range[2]
                && py <= *current_range[3]);

            win.set_ignore_cursor_events(inside_region).ok();
            if !hidden {
                let over_window = (px >= *WINDOW_POLY[0]
                    && px <= *WINDOW_POLY[1]
                    && py >= *WINDOW_POLY[2]
                    && py < *WINDOW_POLY[3])
                    && inside_region;
                Ok((px - 0.5, py - 0.5, over_window))
            } else {
                Ok((px - 0.5, py - 0.5, false))
            }*/
        }
        Mouse::Error => Err("Error getting mouse position".to_string()),
    }
}

fn in_region(px: f32, py: f32, poly: &[&f32]) -> bool {
    //(mouse y - parallelogram y) / parallelogram h * offset x = pct down
    let tx = px + (py - *poly[1]) / *poly[3] * *poly[4];
    let inside =
        tx >= *poly[0] && py >= *poly[1] && tx <= *poly[0] + *poly[2] && py <= *poly[1] + *poly[3];
    inside
}
