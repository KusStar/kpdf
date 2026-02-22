use anyhow::anyhow;
use gpui::*;
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// An asset source that loads assets from the `./assets` folder.
#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

use gpui_component::IconNamed;

pub enum IconName {
    ChevronLast,
    ChevronFirst,
    ChevronRight,
    ChevronLeft,
    File,
    FolderOpen,
    Minimize,
    LoaderCircle,
    Plus,
    Minus,
    WindowMaximize,
    WindowClose,
    WindowMinimize,
    WindowRestore,
    Bookmark,
    BookmarkMinus,
    BookmarkCheck,
    PanelLeftDashed,
}

impl IconNamed for IconName {
    fn path(self) -> gpui::SharedString {
        match self {
            Self::ChevronLast => "icons/chevron-last.svg",
            Self::ChevronFirst => "icons/chevron-first.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::File => "icons/file.svg",
            Self::FolderOpen => "icons/folder-open.svg",
            Self::Minimize => "icons/minimize.svg",
            Self::LoaderCircle => "icons/loader-circle.svg",
            Self::Plus => "icons/plus.svg",
            Self::Minus => "icons/minus.svg",
            Self::WindowMaximize => "icons/window-maximize.svg",
            Self::WindowClose => "icons/window-close.svg",
            Self::WindowMinimize => "icons/window-minimize.svg",
            Self::WindowRestore => "icons/window-restore.svg",
            Self::Bookmark => "icons/bookmark.svg",
            Self::BookmarkMinus => "icons/bookmark-minus.svg",
            Self::BookmarkCheck => "icons/bookmark-check.svg",
            Self::PanelLeftDashed => "icons/panel-left-dashed.svg",
        }
        .into()
    }
}
