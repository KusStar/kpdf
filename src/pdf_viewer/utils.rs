use crate::i18n::{I18n, Language};
use anyhow::{Context as _, Result, anyhow};
use gpui::RenderImage as GpuiRenderImage;
use image::{Frame as RasterFrame, RgbaImage};
use pdfium_render::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;
use std::time::SystemTime;

#[derive(Clone)]
pub struct PageSummary {
    pub index: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub thumbnail_image: Option<Arc<GpuiRenderImage>>,
    pub thumbnail_render_width: u32,
    pub thumbnail_failed: bool,
    pub display_image: Option<Arc<GpuiRenderImage>>,
    pub display_render_width: u32,
    pub display_failed: bool,
}

static PDFIUM_INSTANCE: OnceLock<Pdfium> = OnceLock::new();
static PDFIUM_INIT_LOCK: Mutex<()> = Mutex::new(());
static PDFIUM_ACCESS_LOCK: Mutex<()> = Mutex::new(());
static PDFIUM_DOCUMENT_CACHE: OnceLock<Mutex<Option<CachedPdfDocument>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedPdfDocumentKey {
    canonical_path: PathBuf,
    file_len: Option<u64>,
    modified: Option<SystemTime>,
}

struct CachedPdfDocument {
    key: CachedPdfDocumentKey,
    document: PdfDocument<'static>,
}

fn shared_pdfium(language: Language) -> Result<&'static Pdfium> {
    if let Some(pdfium) = PDFIUM_INSTANCE.get() {
        return Ok(pdfium);
    }

    let _init_guard = PDFIUM_INIT_LOCK
        .lock()
        .map_err(|_| anyhow!("Pdfium init lock is poisoned"))?;

    if let Some(pdfium) = PDFIUM_INSTANCE.get() {
        return Ok(pdfium);
    }

    let pdfium = init_pdfium(language)?;
    let _ = PDFIUM_INSTANCE.set(pdfium);
    PDFIUM_INSTANCE
        .get()
        .ok_or_else(|| anyhow!("Pdfium initialized but instance is unavailable"))
}

fn pdfium_access_guard() -> Result<MutexGuard<'static, ()>> {
    PDFIUM_ACCESS_LOCK
        .lock()
        .map_err(|_| anyhow!("Pdfium global access lock is poisoned"))
}

pub(super) fn ensure_pdfium_ready(language: Language) -> Result<()> {
    let _access_guard = pdfium_access_guard()?;
    shared_pdfium(language).map(|_| ())
}

fn app_resources_lib_dir(current_exe: &Path) -> Option<PathBuf> {
    let macos_dir = current_exe.parent()?;
    if macos_dir.file_name()?.to_string_lossy() != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()?.to_string_lossy() != "Contents" {
        return None;
    }

    Some(contents_dir.join("Resources").join("lib"))
}

fn push_library_dir(
    candidates: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    candidate: PathBuf,
) {
    if candidate.as_os_str().is_empty() {
        return;
    }

    let normalized = if candidate.exists() {
        candidate.canonicalize().unwrap_or(candidate)
    } else if candidate.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(&candidate))
            .unwrap_or(candidate)
    } else {
        candidate
    };

    if seen.insert(normalized.clone()) {
        candidates.push(normalized);
    }
}

fn collect_library_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(override_dir) = std::env::var("KPDF_PDFIUM_LIB_DIR") {
        let override_dir = override_dir.trim();
        if !override_dir.is_empty() {
            push_library_dir(&mut candidates, &mut seen, PathBuf::from(override_dir));
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(resources_lib_dir) = app_resources_lib_dir(&current_exe) {
            push_library_dir(&mut candidates, &mut seen, resources_lib_dir);
        }

        if let Some(exe_dir) = current_exe.parent() {
            push_library_dir(&mut candidates, &mut seen, exe_dir.join("lib"));
            push_library_dir(&mut candidates, &mut seen, exe_dir.to_path_buf());

            for ancestor in exe_dir.ancestors().take(6) {
                push_library_dir(&mut candidates, &mut seen, ancestor.join("lib"));
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_library_dir(&mut candidates, &mut seen, current_dir.join("lib"));
        push_library_dir(&mut candidates, &mut seen, current_dir);
    }

    push_library_dir(&mut candidates, &mut seen, PathBuf::from("./lib"));
    push_library_dir(&mut candidates, &mut seen, PathBuf::from("./"));

    candidates
}

fn init_pdfium(language: Language) -> Result<Pdfium> {
    let i18n = I18n::new(language);

    crate::debug_log!("[pdfium] starting init...");

    for lib_dir in collect_library_dirs() {
        let lib_path = Pdfium::pdfium_platform_library_name_at_path(&lib_dir);
        let display = lib_path.to_string_lossy().into_owned();
        crate::debug_log!("[pdfium] trying path: {}", display);

        if !lib_path.exists() {
            crate::debug_log!("[pdfium] {} skipped: not found", display);
            continue;
        }

        match Pdfium::bind_to_library(lib_path) {
            Ok(bindings) => {
                crate::debug_log!("[pdfium] loaded from {}", display);
                crate::debug_log!("[pdfium] init success!");
                return Ok(Pdfium::new(bindings));
            }
            Err(e) => crate::debug_log!("[pdfium] {} failed: {}", display, e),
        }
    }

    crate::debug_log!("[pdfium] trying system library");
    let bindings = Pdfium::bind_to_system_library();
    match &bindings {
        Ok(_) => crate::debug_log!("[pdfium] loaded from system"),
        Err(e) => crate::debug_log!("[pdfium] system failed: {}", e),
    }

    let bindings = bindings.context(i18n.pdfium_not_found)?;
    crate::debug_log!("[pdfium] init success!");
    Ok(Pdfium::new(bindings))
}

fn document_cache() -> &'static Mutex<Option<CachedPdfDocument>> {
    PDFIUM_DOCUMENT_CACHE.get_or_init(|| Mutex::new(None))
}

fn document_cache_key(path: &Path) -> CachedPdfDocumentKey {
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let metadata = std::fs::metadata(&canonical_path).ok();

    CachedPdfDocumentKey {
        canonical_path,
        file_len: metadata.as_ref().map(|meta| meta.len()),
        modified: metadata.and_then(|meta| meta.modified().ok()),
    }
}

pub(super) fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

pub(super) fn load_document_summary(path: &Path, language: Language) -> Result<Vec<PageSummary>> {
    let _access_guard = pdfium_access_guard()?;
    let i18n = I18n::new(language);
    crate::debug_log!("[pdf][load] opening: {}", path.display());

    let pdfium = shared_pdfium(language)?;
    crate::debug_log!("[pdf][load] pdfium loaded");

    let document = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| i18n.pdfium_cannot_open_file(path))?;
    crate::debug_log!(
        "[pdf][load] document loaded, pages: {}",
        document.pages().len()
    );

    let total_pages = document.pages().len() as usize;
    let mut pages = Vec::with_capacity(total_pages);

    for ix in 0..total_pages {
        let page = document.pages().get(ix as u16)?;
        let width_pt = page.width().value as f32;
        let height_pt = page.height().value as f32;

        pages.push(PageSummary {
            index: ix,
            width_pt,
            height_pt,
            thumbnail_image: None,
            thumbnail_render_width: 0,
            thumbnail_failed: false,
            display_image: None,
            display_render_width: 0,
            display_failed: false,
        });
    }

    crate::debug_log!("[pdf][load] summary loaded, {} pages", pages.len());
    Ok(pages)
}

pub(super) fn load_display_images(
    path: &Path,
    page_indices: &[usize],
    target_width: u32,
    language: Language,
) -> Result<Vec<(usize, Arc<GpuiRenderImage>)>> {
    let _access_guard = pdfium_access_guard()?;
    if page_indices.is_empty() {
        return Ok(Vec::new());
    }

    let render_config = PdfRenderConfig::new().set_target_width(target_width as i32);
    let mut display_images = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let file_name = display_file_name(path);
    let cache_key = document_cache_key(path);
    let i18n = I18n::new(language);
    let mut cached_document_guard = document_cache()
        .lock()
        .map_err(|_| anyhow!(i18n.pdfium_cache_lock_poisoned))?;

    let cache_hit = cached_document_guard
        .as_ref()
        .map(|cached| cached.key == cache_key)
        .unwrap_or(false);

    if !cache_hit {
        let pdfium = shared_pdfium(language)?;
        let document = pdfium
            .load_pdf_from_file(&cache_key.canonical_path, None)
            .with_context(|| i18n.pdfium_cannot_open_file(path))?;

        *cached_document_guard = Some(CachedPdfDocument {
            key: cache_key,
            document,
        });
    }

    let document = &cached_document_guard
        .as_ref()
        .expect("Pdfium document cache should be initialized")
        .document;
    let total_pages = document.pages().len() as usize;
    let requested: Vec<usize> = page_indices
        .iter()
        .copied()
        .filter(|ix| seen.insert(*ix))
        .collect();

    for ix in requested {
        let started_at = Instant::now();
        let page_num = ix + 1;

        if ix >= total_pages || ix > u16::MAX as usize {
            crate::debug_log!(
                "[pdf][render] {} p{} skipped: out of range (total_pages={}) | {}ms",
                file_name,
                page_num,
                total_pages,
                started_at.elapsed().as_millis()
            );
            continue;
        }

        let page = match document.pages().get(ix as u16) {
            Ok(page) => page,
            Err(err) => {
                crate::debug_log!(
                    "[pdf][render] {} p{} failed: get_page error: {} | {}ms",
                    file_name,
                    page_num,
                    err,
                    started_at.elapsed().as_millis()
                );
                continue;
            }
        };

        let render_started_at = Instant::now();
        let bitmap = match page.render_with_config(&render_config) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                crate::debug_log!(
                    "[pdf][render] {} p{} failed: render error: {} | total={}ms render={}ms",
                    file_name,
                    page_num,
                    err,
                    started_at.elapsed().as_millis(),
                    render_started_at.elapsed().as_millis()
                );
                continue;
            }
        };
        let render_elapsed_ms = render_started_at.elapsed().as_millis();

        let convert_started_at = Instant::now();
        match bitmap_to_gpui_render_image(&bitmap, language) {
            Ok(image) => {
                display_images.push((ix, image));
            }
            Err(err) => {
                crate::debug_log!(
                    "[pdf][render] {} p{} failed: upload error: {} | total={}ms render={}ms upload={}ms",
                    file_name,
                    page_num,
                    err,
                    started_at.elapsed().as_millis(),
                    render_elapsed_ms,
                    convert_started_at.elapsed().as_millis()
                );
            }
        }
    }

    Ok(display_images)
}

#[allow(deprecated)]
fn bitmap_to_gpui_render_image(
    bitmap: &PdfBitmap,
    language: Language,
) -> Result<Arc<GpuiRenderImage>> {
    let i18n = I18n::new(language);
    let width = bitmap.width() as u32;
    let height = bitmap.height() as u32;
    if width == 0 || height == 0 {
        return Err(anyhow!(i18n.invalid_bitmap_size(width, height)));
    }

    let format = bitmap.format().unwrap_or(PdfBitmapFormat::BGRA);
    let mut bytes = match format {
        PdfBitmapFormat::BGRA | PdfBitmapFormat::BGRx | PdfBitmapFormat::BRGx => {
            bitmap.as_raw_bytes()
        }
        _ => rgba_to_bgra(bitmap.as_rgba_bytes()),
    };

    let expected_len = width as usize * height as usize * 4;
    if bytes.len() != expected_len {
        bytes = rgba_to_bgra(bitmap.as_rgba_bytes());
        if bytes.len() != expected_len {
            return Err(anyhow!(i18n.bitmap_len_mismatch(bytes.len(), expected_len)));
        }
    }

    if matches!(format, PdfBitmapFormat::BGRx | PdfBitmapFormat::BRGx) {
        for pixel in bytes.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }

    let buffer = RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow!(i18n.cannot_create_image_buffer(width, height)))?;
    let frame = RasterFrame::new(buffer);

    Ok(Arc::new(GpuiRenderImage::new([frame])))
}

fn rgba_to_bgra(mut rgba: Vec<u8>) -> Vec<u8> {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    rgba
}

/// Load text information from a page for text selection using pdfium-render high-level API
pub fn load_page_text_for_selection(
    path: &Path,
    page_index: usize,
) -> Result<Option<(usize, f32, f32, Vec<super::text_selection::TextCharInfo>)>> {
    let _access_guard = pdfium_access_guard()?;
    let cache_key = document_cache_key(path);

    let mut cached_document_guard = match document_cache().lock() {
        Ok(guard) => guard,
        Err(_) => return Ok(None),
    };

    let cache_hit = cached_document_guard
        .as_ref()
        .map(|cached| cached.key == cache_key)
        .unwrap_or(false);

    if !cache_hit {
        let pdfium = match shared_pdfium(Language::EnUs) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let document = match pdfium.load_pdf_from_file(&cache_key.canonical_path, None) {
            Ok(doc) => doc,
            Err(err) => {
                crate::debug_log!(
                    "[text] Failed to load document for text extraction: {} | {:?}",
                    path.display(),
                    err
                );
                return Ok(None);
            }
        };

        *cached_document_guard = Some(CachedPdfDocument {
            key: cache_key,
            document,
        });
    }

    let document = match cached_document_guard.as_ref() {
        Some(cached) => &cached.document,
        None => return Ok(None),
    };

    let total_pages = document.pages().len() as usize;
    if page_index >= total_pages {
        crate::debug_log!(
            "[text] Page index {} out of range (total: {})",
            page_index,
            total_pages
        );
        return Ok(None);
    }

    let page = match document.pages().get(page_index as u16) {
        Ok(p) => p,
        Err(err) => {
            crate::debug_log!(
                "[text] Failed to get page {} from document: {:?}",
                page_index,
                err
            );
            return Ok(None);
        }
    };

    let page_width = page.width().value as f32;
    let page_height = page.height().value as f32;

    let page_text = match page.text() {
        Ok(text) => text,
        Err(err) => {
            crate::debug_log!(
                "[text] Failed to get text from page {}: {:?}",
                page_index,
                err
            );
            return Ok(None);
        }
    };

    let chars_collection = page_text.chars();

    if chars_collection.is_empty() {
        return Ok(Some((page_index, page_width, page_height, Vec::new())));
    }

    let mut chars = Vec::new();

    for char in chars_collection.iter() {
        let bounds = match char.tight_bounds() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let text = char
            .unicode_char()
            .map(|c| c.to_string())
            .unwrap_or_default();

        chars.push(super::text_selection::TextCharInfo {
            text,
            left: bounds.left().value as f32,
            top: bounds.top().value as f32,
            right: bounds.right().value as f32,
            bottom: bounds.bottom().value as f32,
        });
    }

    Ok(Some((page_index, page_width, page_height, chars)))
}
