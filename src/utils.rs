use crate::i18n::{I18n, Language};
use anyhow::{anyhow, Context as _, Result};
use gpui::RenderImage as GpuiRenderImage;
use image::{Frame as RasterFrame, RgbaImage};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use std::time::SystemTime;

use super::PageSummary;

static PDFIUM_INSTANCE: OnceLock<Result<Pdfium, String>> = OnceLock::new();
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
    match PDFIUM_INSTANCE.get_or_init(|| init_pdfium(language).map_err(|err| format!("{err:#}"))) {
        Ok(pdfium) => Ok(pdfium),
        Err(message) => Err(anyhow!("{message}")),
    }
}

fn init_pdfium(language: Language) -> Result<Pdfium> {
    let i18n = I18n::new(language);

    eprintln!("[pdfium] starting init...");

    let lib_path = "./lib";
    eprintln!("[pdfium] trying path: {}", lib_path);
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(lib_path));
    match &bindings {
        Ok(_) => eprintln!("[pdfium] loaded from {}", lib_path),
        Err(e) => eprintln!("[pdfium] {} failed: {}", lib_path, e),
    }
    let bindings = bindings.or_else(|_| {
        eprintln!("[pdfium] trying path: ./");
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
    });
    match &bindings {
        Ok(_) => eprintln!("[pdfium] loaded from ./"),
        Err(e) => eprintln!("[pdfium] ./ failed: {}", e),
    }
    let bindings = bindings.or_else(|_| {
        eprintln!("[pdfium] trying system library");
        Pdfium::bind_to_system_library()
    });
    match &bindings {
        Ok(_) => eprintln!("[pdfium] loaded from system"),
        Err(e) => eprintln!("[pdfium] system failed: {}", e),
    }

    let bindings = bindings.context(i18n.pdfium_not_found())?;

    eprintln!("[pdfium] init success!");
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
    let i18n = I18n::new(language);
    eprintln!("[pdf][load] opening: {}", path.display());

    let pdfium = shared_pdfium(language)?;
    eprintln!("[pdf][load] pdfium loaded");

    let document = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| i18n.pdfium_cannot_open_file(path))?;
    eprintln!(
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

    eprintln!("[pdf][load] summary loaded, {} pages", pages.len());
    Ok(pages)
}

pub(super) fn load_display_images(
    path: &Path,
    page_indices: &[usize],
    target_width: u32,
    language: Language,
) -> Result<Vec<(usize, Arc<GpuiRenderImage>)>> {
    if page_indices.is_empty() {
        return Ok(Vec::new());
    }

    eprintln!(
        "[pdf][render] loading {} pages, target_width={}",
        page_indices.len(),
        target_width
    );

    let render_config = PdfRenderConfig::new().set_target_width(target_width as i32);
    let mut display_images = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let file_name = display_file_name(path);
    let cache_key = document_cache_key(path);
    let i18n = I18n::new(language);
    let mut cached_document_guard = document_cache()
        .lock()
        .map_err(|_| anyhow!(i18n.pdfium_cache_lock_poisoned()))?;

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

    // eprintln!(
    //     "[pdf][document] {} {}",
    //     file_name,
    //     if cache_hit { "cache_hit" } else { "cache_miss" }
    // );

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
            eprintln!(
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
                eprintln!(
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
                eprintln!(
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
                // eprintln!(
                //     "[pdf][render] {} p{} ok | total={}ms render={}ms upload={}ms target_width={} format=raw_bgra",
                //     file_name,
                //     page_num,
                //     started_at.elapsed().as_millis(),
                //     render_elapsed_ms,
                //     convert_started_at.elapsed().as_millis(),
                //     target_width
                // );
            }
            Err(err) => {
                eprintln!(
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
