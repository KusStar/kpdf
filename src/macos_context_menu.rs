use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSEvent, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString, ns_string};
use std::sync::atomic::{AtomicI32, Ordering};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MacTabContextMenuAction {
    CloseAllTabs = 1,
    CloseOtherTabs = 2,
    RevealInFinder = 3,
}

static SELECTED_TAB_MENU_ACTION: AtomicI32 = AtomicI32::new(0);

define_class!(
    // SAFETY: NSObject has no extra subclassing requirements.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct TabContextMenuTarget;

    // SAFETY: NSObjectProtocol has no extra safety requirements.
    unsafe impl NSObjectProtocol for TabContextMenuTarget {}

    impl TabContextMenuTarget {
        #[unsafe(method(kpdfCloseAllTabs:))]
        fn on_close_all_tabs(&self, _sender: &AnyObject) {
            SELECTED_TAB_MENU_ACTION.store(
                MacTabContextMenuAction::CloseAllTabs as i32,
                Ordering::SeqCst,
            );
        }

        #[unsafe(method(kpdfCloseOtherTabs:))]
        fn on_close_other_tabs(&self, _sender: &AnyObject) {
            SELECTED_TAB_MENU_ACTION.store(
                MacTabContextMenuAction::CloseOtherTabs as i32,
                Ordering::SeqCst,
            );
        }

        #[unsafe(method(kpdfRevealInFinder:))]
        fn on_reveal_in_finder(&self, _sender: &AnyObject) {
            SELECTED_TAB_MENU_ACTION.store(
                MacTabContextMenuAction::RevealInFinder as i32,
                Ordering::SeqCst,
            );
        }
    }
);

impl TabContextMenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        // SAFETY: `init` is sent to an object returned from `alloc`.
        unsafe { msg_send![this, init] }
    }
}

fn make_menu_item(
    mtm: MainThreadMarker,
    title: &NSString,
    selector: objc2::runtime::Sel,
    target: &TabContextMenuTarget,
    enabled: bool,
) -> Retained<NSMenuItem> {
    // SAFETY: selector is defined on TabContextMenuTarget.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            title,
            Some(selector),
            ns_string!(""),
        )
    };
    // SAFETY: target implements the action selectors.
    unsafe { item.setTarget(Some(target)) };
    item.setEnabled(enabled);
    item
}

pub fn show_tab_context_menu(
    close_all_label: &str,
    close_other_label: &str,
    reveal_label: &str,
    can_close_others: bool,
    can_reveal: bool,
) -> Option<MacTabContextMenuAction> {
    let mtm = MainThreadMarker::new()?;
    SELECTED_TAB_MENU_ACTION.store(0, Ordering::SeqCst);

    let target = TabContextMenuTarget::new(mtm);
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("kPDF"));
    menu.setAutoenablesItems(false);

    let close_all_title = NSString::from_str(close_all_label);
    let close_other_title = NSString::from_str(close_other_label);
    let reveal_title = NSString::from_str(reveal_label);

    let close_all_item = make_menu_item(
        mtm,
        &close_all_title,
        sel!(kpdfCloseAllTabs:),
        &target,
        true,
    );
    let close_other_item = make_menu_item(
        mtm,
        &close_other_title,
        sel!(kpdfCloseOtherTabs:),
        &target,
        can_close_others,
    );
    let reveal_item = make_menu_item(
        mtm,
        &reveal_title,
        sel!(kpdfRevealInFinder:),
        &target,
        can_reveal,
    );

    menu.addItem(&close_all_item);
    menu.addItem(&close_other_item);
    menu.addItem(&reveal_item);

    let location = NSEvent::mouseLocation();
    let _ = menu.popUpMenuPositioningItem_atLocation_inView(None, location, None);

    match SELECTED_TAB_MENU_ACTION.load(Ordering::SeqCst) {
        x if x == MacTabContextMenuAction::CloseAllTabs as i32 => {
            Some(MacTabContextMenuAction::CloseAllTabs)
        }
        x if x == MacTabContextMenuAction::CloseOtherTabs as i32 => {
            Some(MacTabContextMenuAction::CloseOtherTabs)
        }
        x if x == MacTabContextMenuAction::RevealInFinder as i32 => {
            Some(MacTabContextMenuAction::RevealInFinder)
        }
        _ => None,
    }
}
