use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents a text selection range on a specific page
#[derive(Clone, Debug, PartialEq)]
pub struct TextSelection {
    pub page_index: usize,
    pub start_char_index: usize,
    pub end_char_index: usize,
}

impl TextSelection {
    pub fn new(page_index: usize, start_char_index: usize, end_char_index: usize) -> Self {
        let (start, end) = if start_char_index <= end_char_index {
            (start_char_index, end_char_index)
        } else {
            (end_char_index, start_char_index)
        };
        Self {
            page_index,
            start_char_index: start,
            end_char_index: end,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start_char_index == self.end_char_index
    }

    pub fn char_count(&self) -> usize {
        self.end_char_index.saturating_sub(self.start_char_index)
    }
}

/// Represents a character with its bounds and index
#[derive(Clone, Debug)]
pub struct TextCharInfo {
    pub char_index: usize,
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl TextCharInfo {
    /// Check if a point (in PDF coordinates) is inside this character's bounds
    /// PDF coordinates: origin at bottom-left, y increases upward
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        // Add some tolerance for easier selection
        let tolerance = 2.0;
        x >= (self.left - tolerance)
            && x <= (self.right + tolerance)
            && y >= (self.bottom - tolerance)
            && y <= (self.top + tolerance)
    }

    /// Get center point of the character
    pub fn center(&self) -> (f32, f32) {
        (
            (self.left + self.right) / 2.0,
            (self.bottom + self.top) / 2.0,
        )
    }
}

/// Cache for page text information
#[derive(Clone)]
pub struct PageTextCache {
    pub page_index: usize,
    pub chars: Vec<TextCharInfo>,
    pub page_width: f32,
    pub page_height: f32,
}

impl PageTextCache {
    /// Find the closest character to a point
    pub fn find_char_at_position(&self, x: f32, y: f32) -> Option<usize> {
        // First, try to find a character that contains the point
        for (index, char_info) in self.chars.iter().enumerate() {
            if char_info.contains_point(x, y) {
                return Some(index);
            }
        }

        // If no character contains the point, find the closest one
        // Consider both distance and character size for better selection accuracy
        let mut closest_index: Option<usize> = None;
        let mut closest_distance = f32::INFINITY;

        for (index, char_info) in self.chars.iter().enumerate() {
            // Calculate distance to the center of the character
            let center = char_info.center();
            let dx = x - center.0;
            let dy = y - center.1;
            let distance = (dx * dx + dy * dy).sqrt();

            // Apply character size adjustment: larger characters get a small bonus
            // This helps with different font sizes in the same document
            let char_width = char_info.right - char_info.left;
            let char_height = char_info.top - char_info.bottom;
            let char_size = char_width.max(char_height);
            let size_factor = 1.0 + (char_size / 100.0).min(1.0);

            let adjusted_distance = distance / size_factor;

            if adjusted_distance < closest_distance {
                closest_distance = adjusted_distance;
                closest_index = Some(index);
            }
        }

        // Only return if within reasonable distance (e.g., 50 PDF points)
        if closest_distance < 50.0 {
            closest_index
        } else {
            None
        }
    }

    pub fn get_char_bounds(&self, char_index: usize) -> Option<(f32, f32, f32, f32)> {
        self.chars
            .get(char_index)
            .map(|c| (c.left, c.top, c.right, c.bottom))
    }

    pub fn get_selection_bounds(&self, selection: &TextSelection) -> Vec<(f32, f32, f32, f32)> {
        let start = selection.start_char_index.min(self.chars.len());
        let end = selection.end_char_index.min(self.chars.len());

        if start >= end {
            return Vec::new();
        }

        // Group characters by line (using bottom position with tolerance)
        // Use a larger tolerance based on typical line height
        let line_tolerance = 8.0; // Tolerance for grouping characters on the same line
        let mut lines: Vec<Vec<&TextCharInfo>> = Vec::new();

        for i in start..end {
            if let Some(char_info) = self.chars.get(i) {
                // Find existing line or create new one
                // Use bottom position for more reliable line grouping
                let line_found = lines.iter_mut().find(|line| {
                    line.first().map_or(false, |first| {
                        (first.bottom - char_info.bottom).abs() < line_tolerance
                    })
                });

                if let Some(line) = line_found {
                    line.push(char_info);
                } else {
                    lines.push(vec![char_info]);
                }
            }
        }

        // Create merged rectangles for each line
        let mut bounds = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }

            // Find the bounding box for all characters in the line
            let min_left = line.iter().map(|c| c.left).fold(f32::INFINITY, f32::min);
            let max_right = line
                .iter()
                .map(|c| c.right)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_bottom = line.iter().map(|c| c.bottom).fold(f32::INFINITY, f32::min);
            let max_top = line.iter().map(|c| c.top).fold(f32::NEG_INFINITY, f32::max);

            bounds.push((min_left, max_top, max_right, min_bottom));
        }

        bounds
    }

    pub fn get_text(&self, selection: &TextSelection) -> String {
        let start = selection.start_char_index.min(self.chars.len());
        let end = selection.end_char_index.min(self.chars.len());

        self.chars[start..end]
            .iter()
            .map(|c| c.text.as_str())
            .collect()
    }

    /// Find character at screen position with proper conversion from screen to PDF coordinates
    pub fn find_char_at_screen_position(
        &self,
        screen_x: f32,
        screen_y: f32,
        page_bounds: (f32, f32, f32, f32), // (left, top, right, bottom) in screen coordinates
        page_scale: f32,
    ) -> Option<usize> {
        let (page_left, page_top, page_right, page_bottom) = page_bounds;
        let page_width_screen = page_right - page_left;
        let page_height_screen = page_bottom - page_top;

        // Calculate the actual rendered content dimensions considering ObjectFit::Contain
        let content_aspect = self.page_width / self.page_height;
        let container_aspect = page_width_screen / page_height_screen;

        let (content_width_screen, content_height_screen) = if content_aspect > container_aspect {
            // Width is limiting factor
            (page_width_screen, self.page_height * page_scale)
        } else {
            // Height is limiting factor
            (self.page_width * page_scale, page_height_screen)
        };

        // Calculate centering offsets
        let x_offset = (page_width_screen - content_width_screen) / 2.0;
        let y_offset = (page_height_screen - content_height_screen) / 2.0;

        // Convert screen coordinates to content-relative coordinates
        let content_relative_x = screen_x - (page_left + x_offset);
        let content_relative_y = screen_y - (page_top + y_offset);

        // Check if the point is within the rendered content area
        if content_relative_x < 0.0
            || content_relative_x > content_width_screen
            || content_relative_y < 0.0
            || content_relative_y > content_height_screen
        {
            return None;
        }

        // Convert to PDF coordinates
        // PDF coordinates: origin at bottom-left, y increases upward
        // Content coordinates: origin at top-left, y increases downward
        let pdf_x = content_relative_x / page_scale;
        let pdf_y = (content_height_screen - content_relative_y) / page_scale;

        // Use the existing find_char_at_position method
        self.find_char_at_position(pdf_x, pdf_y)
    }
}

/// Manager for text selection functionality
pub struct TextSelectionManager {
    text_caches: Arc<Mutex<HashMap<usize, PageTextCache>>>,
    current_selection: Option<TextSelection>,
    is_selecting: bool,
    selection_start: Option<(usize, usize)>, // (page_index, char_index)
}

impl TextSelectionManager {
    pub fn new() -> Self {
        Self {
            text_caches: Arc::new(Mutex::new(HashMap::new())),
            current_selection: None,
            is_selecting: false,
            selection_start: None,
        }
    }

    pub fn get_page_cache(&self, page_index: usize) -> Option<PageTextCache> {
        self.text_caches.lock().ok()?.get(&page_index).cloned()
    }

    pub fn start_selection(&mut self, page_index: usize, char_index: usize) {
        self.is_selecting = true;
        self.selection_start = Some((page_index, char_index));
        self.current_selection = Some(TextSelection::new(page_index, char_index, char_index));
        eprintln!(
            "[selection] Started at page {}, char {}",
            page_index, char_index
        );
    }

    pub fn update_selection(&mut self, page_index: usize, char_index: usize) {
        if let Some((start_page, start_char)) = self.selection_start {
            if start_page == page_index {
                self.current_selection =
                    Some(TextSelection::new(page_index, start_char, char_index));
            }
        }
    }

    pub fn end_selection(&mut self) {
        self.is_selecting = false;
        if let Some(ref selection) = self.current_selection {
            eprintln!(
                "[selection] Ended: page {}, chars {} to {}",
                selection.page_index, selection.start_char_index, selection.end_char_index
            );
            if let Some(cache) = self.get_page_cache(selection.page_index) {
                eprintln!(
                    "[selection] Cache has {} chars, page size: {}x{}",
                    cache.chars.len(),
                    cache.page_width,
                    cache.page_height
                );

                // Log first 20 chars to understand text order
                for i in 0..20.min(cache.chars.len()) {
                    if let Some(c) = cache.chars.get(i) {
                        eprintln!(
                            "[selection] char[{}] = '{}' at pdf({},{})",
                            i, c.text, c.left, c.top
                        );
                    }
                }

                // Also log around the selected start position
                let search_idx = 155.min(cache.chars.len().saturating_sub(5));
                for i in search_idx..(search_idx + 10).min(cache.chars.len()) {
                    if let Some(c) = cache.chars.get(i) {
                        eprintln!(
                            "[selection] char_near[{}] = '{}' at pdf({},{})",
                            i, c.text, c.left, c.top
                        );
                    }
                }

                let start_idx = selection
                    .start_char_index
                    .min(cache.chars.len().saturating_sub(1));
                let end_idx = selection.end_char_index.min(cache.chars.len());

                if let Some(start_char) = cache.chars.get(start_idx) {
                    eprintln!(
                        "[selection] Start char[{}]: '{}' at pdf({},{})",
                        start_idx, start_char.text, start_char.left, start_char.top
                    );
                }
                if let Some(end_char) = cache.chars.get(end_idx.saturating_sub(1)) {
                    eprintln!(
                        "[selection] End char[{}]: '{}' at pdf({},{})",
                        end_idx.saturating_sub(1),
                        end_char.text,
                        end_char.left,
                        end_char.top
                    );
                }

                let text = self.get_selected_text();
                if let Some(text) = text {
                    eprintln!("[selection] Text: '{}'", text);
                } else {
                    eprintln!("[selection] Text: (empty)");
                }
            } else {
                eprintln!("[selection] No cache for page {}", selection.page_index);
            }
        }
    }

    pub fn clear_selection(&mut self) {
        self.current_selection = None;
        self.selection_start = None;
        self.is_selecting = false;
        eprintln!("[selection] Cleared");
    }

    pub fn get_current_selection(&self) -> Option<&TextSelection> {
        self.current_selection.as_ref()
    }

    pub fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let selection = self.current_selection.as_ref()?;
        let cache = self.get_page_cache(selection.page_index)?;
        let text = cache.get_text(selection);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn get_selection_rects(&self, page_index: usize) -> Option<Vec<(f32, f32, f32, f32)>> {
        let selection = self.current_selection.as_ref()?;
        if selection.page_index != page_index {
            return None;
        }
        let cache = self.get_page_cache(page_index)?;
        Some(cache.get_selection_bounds(selection))
    }

    pub fn clear_cache(&mut self) {
        if let Ok(mut caches) = self.text_caches.lock() {
            caches.clear();
        }
    }

    /// Load cached text data directly (used by async loading)
    pub fn load_cached_text(
        &mut self,
        page_index: usize,
        page_width: f32,
        page_height: f32,
        chars: Vec<TextCharInfo>,
    ) {
        let cache = PageTextCache {
            page_index,
            chars,
            page_width,
            page_height,
        };
        if let Ok(mut caches) = self.text_caches.lock() {
            caches.insert(page_index, cache);
        }
    }
}

impl Default for TextSelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to copy text to system clipboard
#[cfg(target_os = "macos")]
pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut echo = Command::new("echo")
        .arg("-n") // Don't add newline
        .arg(text)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let echo_stdout = echo.stdout.take().ok_or("Failed to get echo stdout")?;

    let mut pbcopy = Command::new("pbcopy").stdin(echo_stdout).spawn()?;

    echo.wait()?;
    pbcopy.wait()?;

    eprintln!("[clipboard] Copied {} characters on macOS", text.len());
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut echo = Command::new("echo")
        .arg("-n")
        .arg(text)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let echo_stdout = echo.stdout.take().ok_or("Failed to get echo stdout")?;

    let mut clipboard_cmd = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(echo_stdout)
        .spawn()
        .or_else(|_| Command::new("wl-copy").stdin(echo_stdout).spawn())?;

    echo.wait()?;
    clipboard_cmd.wait()?;

    eprintln!("[clipboard] Copied {} characters on Linux", text.len());
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    // Use PowerShell for more reliable clipboard access on Windows
    let mut ps = Command::new("powershell")
        .args([
            "-command",
            &format!("Set-Clipboard -Value '{}'", text.replace("'", "''")),
        ])
        .spawn()?;

    ps.wait()?;

    eprintln!("[clipboard] Copied {} characters on Windows", text.len());
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn copy_to_clipboard(_text: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("Clipboard not supported on this platform".into())
}
