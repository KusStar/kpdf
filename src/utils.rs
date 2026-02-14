use anyhow::{Context as _, Result, anyhow};
use gpui::RenderImage as GpuiRenderImage;
use image::{Frame as RasterFrame, RgbaImage};
use pdf_rs::document::PDFDocument;
use pdf_rs::objects::{ObjRefTuple, PDFNumber, PDFObject};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use std::time::SystemTime;

use super::PageSummary;

const DEFAULT_PAGE_WIDTH_PT: f32 = 595.0;
const DEFAULT_PAGE_HEIGHT_PT: f32 = 842.0;
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

fn shared_pdfium() -> Result<&'static Pdfium> {
    match PDFIUM_INSTANCE.get_or_init(|| init_pdfium().map_err(|err| format!("{err:#}"))) {
        Ok(pdfium) => Ok(pdfium),
        Err(message) => Err(anyhow!("{message}")),
    }
}

fn init_pdfium() -> Result<Pdfium> {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib"))
        .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")))
        .or_else(|_| Pdfium::bind_to_system_library())
        .context("未找到 Pdfium 动态库（已尝试 ./lib、./ 与系统库）")?;

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

pub(super) fn load_document_summary(path: &Path) -> Result<Vec<PageSummary>> {
    let mut document = PDFDocument::open(path.to_path_buf())
        .with_context(|| format!("无法打开文件: {}", path.to_string_lossy()))?;
    let page_ids = document.get_page_ids();
    let mut pages = Vec::with_capacity(document.get_page_num());
    let mut page_refs = Vec::with_capacity(page_ids.len());

    for (ix, page_id) in page_ids.into_iter().enumerate() {
        if let Some(page) = document.get_page(page_id) {
            page_refs.push((ix, page.get_page_obj_ref(), page.get_parent_obj_ref()));
        }
    }

    for (index, page_obj_ref, parent_ref) in page_refs {
        let (w, h) = resolve_page_size(&mut document, page_obj_ref, parent_ref);
        pages.push(PageSummary {
            index,
            width_pt: w,
            height_pt: h,
            thumbnail_image: None,
            thumbnail_render_width: 0,
            thumbnail_failed: false,
            display_image: None,
            display_render_width: 0,
            display_failed: false,
        });
    }

    Ok(pages)
}

pub(super) fn load_display_images(
    path: &Path,
    page_indices: &[usize],
    target_width: u32,
) -> Result<Vec<(usize, Arc<GpuiRenderImage>)>> {
    if page_indices.is_empty() {
        return Ok(Vec::new());
    }

    let render_config = PdfRenderConfig::new().set_target_width(target_width as i32);
    let mut display_images = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let file_name = display_file_name(path);
    let cache_key = document_cache_key(path);
    let mut cached_document_guard = document_cache()
        .lock()
        .map_err(|_| anyhow!("Pdfium 文档缓存锁已中毒"))?;

    let cache_hit = cached_document_guard
        .as_ref()
        .map(|cached| cached.key == cache_key)
        .unwrap_or(false);

    if !cache_hit {
        let pdfium = shared_pdfium()?;
        let document = pdfium
            .load_pdf_from_file(&cache_key.canonical_path, None)
            .with_context(|| format!("Pdfium 无法打开文件: {}", path.to_string_lossy()))?;

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
        match bitmap_to_gpui_render_image(&bitmap) {
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
fn bitmap_to_gpui_render_image(bitmap: &PdfBitmap) -> Result<Arc<GpuiRenderImage>> {
    let width = bitmap.width() as u32;
    let height = bitmap.height() as u32;
    if width == 0 || height == 0 {
        return Err(anyhow!("位图尺寸无效: {}x{}", width, height));
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
            return Err(anyhow!(
                "位图数据长度异常: got={}, expected={}",
                bytes.len(),
                expected_len
            ));
        }
    }

    if matches!(format, PdfBitmapFormat::BGRx | PdfBitmapFormat::BRGx) {
        for pixel in bytes.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }

    let buffer = RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow!("无法创建图像缓冲区: {}x{}", width, height))?;
    let frame = RasterFrame::new(buffer);

    Ok(Arc::new(GpuiRenderImage::new([frame])))
}

fn rgba_to_bgra(mut rgba: Vec<u8>) -> Vec<u8> {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    rgba
}

fn resolve_page_size(
    document: &mut PDFDocument,
    page_obj_ref: ObjRefTuple,
    page_parent_ref: Option<ObjRefTuple>,
) -> (f32, f32) {
    let mut parent = page_parent_ref;
    if let Some(page_obj) = read_object(document, page_obj_ref) {
        if let Some(page_dict) = as_dictionary(&page_obj) {
            if let Some(media_box) = page_dict
                .get("MediaBox")
                .and_then(|obj| resolve_media_box(document, obj))
            {
                return media_box;
            }
            if parent.is_none() {
                parent = page_dict.get("Parent").and_then(resolve_object_ref);
            }
        }
    }

    while let Some(parent_ref) = parent {
        let Some(parent_obj) = read_object(document, parent_ref) else {
            break;
        };

        let Some(parent_dict) = as_dictionary(&parent_obj) else {
            break;
        };

        if let Some(media_box) = parent_dict
            .get("MediaBox")
            .and_then(|obj| resolve_media_box(document, obj))
        {
            return media_box;
        }

        parent = parent_dict.get("Parent").and_then(resolve_object_ref);
    }

    (DEFAULT_PAGE_WIDTH_PT, DEFAULT_PAGE_HEIGHT_PT)
}

fn read_object(document: &mut PDFDocument, obj_ref: ObjRefTuple) -> Option<PDFObject> {
    document.read_object_with_ref(obj_ref).ok().flatten()
}

fn as_dictionary(object: &PDFObject) -> Option<&pdf_rs::objects::Dictionary> {
    match object {
        PDFObject::Dict(dict) => Some(dict),
        PDFObject::IndirectObject(_, _, inner) => inner.as_dict(),
        _ => None,
    }
}

fn resolve_object_ref(object: &PDFObject) -> Option<ObjRefTuple> {
    match object {
        PDFObject::ObjectRef(num, generation) => Some((*num, *generation)),
        PDFObject::IndirectObject(_, _, inner) => resolve_object_ref(inner),
        _ => None,
    }
}

fn resolve_media_box(document: &mut PDFDocument, object: &PDFObject) -> Option<(f32, f32)> {
    match object {
        PDFObject::Array(values) => media_box_from_array(values.as_slice()),
        PDFObject::ObjectRef(obj_num, obj_gen) => read_object(document, (*obj_num, *obj_gen))
            .and_then(|obj| resolve_media_box(document, &obj)),
        PDFObject::IndirectObject(_, _, inner) => resolve_media_box(document, inner),
        _ => None,
    }
}

fn media_box_from_array(values: &[PDFObject]) -> Option<(f32, f32)> {
    if values.len() != 4 {
        return None;
    }

    let left = values[0].as_number().map(number_as_f32)?;
    let bottom = values[1].as_number().map(number_as_f32)?;
    let right = values[2].as_number().map(number_as_f32)?;
    let top = values[3].as_number().map(number_as_f32)?;

    let width = (right - left).abs();
    let height = (top - bottom).abs();
    if width > 1.0 && height > 1.0 {
        Some((width, height))
    } else {
        None
    }
}

fn number_as_f32(number: &PDFNumber) -> f32 {
    match number {
        PDFNumber::Signed(v) => *v as f32,
        PDFNumber::Unsigned(v) => *v as f32,
        PDFNumber::Real(v) => *v as f32,
    }
}
