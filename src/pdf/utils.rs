use anyhow::{Context as _, Result};
use pdf_rs::document::PDFDocument;
use pdf_rs::objects::{ObjRefTuple, PDFNumber, PDFObject};
use std::path::Path;

use super::PageSummary;

const DEFAULT_PAGE_WIDTH_PT: f32 = 595.0;
const DEFAULT_PAGE_HEIGHT_PT: f32 = 842.0;

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
            object_ref: page_obj_ref,
            width_pt: w,
            height_pt: h,
        });
    }

    Ok(pages)
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
