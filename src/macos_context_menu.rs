use cocoa::{
    appkit::{NSEvent, NSMenu, NSMenuItem},
    base::{BOOL, NO, YES, id, nil},
    foundation::NSString,
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use std::sync::Once;
use std::sync::atomic::{AtomicI32, Ordering};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MacTabContextMenuAction {
    CloseAllTabs = 1,
    CloseOtherTabs = 2,
    RevealInFinder = 3,
}

static SELECTED_TAB_MENU_ACTION: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_close_all_tabs(_: &Object, _: Sel, _: id) {
    SELECTED_TAB_MENU_ACTION.store(
        MacTabContextMenuAction::CloseAllTabs as i32,
        Ordering::SeqCst,
    );
}

extern "C" fn on_close_other_tabs(_: &Object, _: Sel, _: id) {
    SELECTED_TAB_MENU_ACTION.store(
        MacTabContextMenuAction::CloseOtherTabs as i32,
        Ordering::SeqCst,
    );
}

extern "C" fn on_reveal_in_finder(_: &Object, _: Sel, _: id) {
    SELECTED_TAB_MENU_ACTION.store(
        MacTabContextMenuAction::RevealInFinder as i32,
        Ordering::SeqCst,
    );
}

fn target_class() -> &'static Class {
    static REGISTER: Once = Once::new();
    static mut CLASS: *const Class = std::ptr::null();

    REGISTER.call_once(|| unsafe {
        let mut decl = ClassDecl::new("KpdfTabContextMenuTarget", class!(NSObject))
            .expect("KpdfTabContextMenuTarget class registration failed");
        decl.add_method(
            sel!(kpdfCloseAllTabs:),
            on_close_all_tabs as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(kpdfCloseOtherTabs:),
            on_close_other_tabs as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(kpdfRevealInFinder:),
            on_reveal_in_finder as extern "C" fn(&Object, Sel, id),
        );
        CLASS = decl.register();
    });

    unsafe { &*CLASS }
}

unsafe fn ns_string(value: &str) -> id {
    let s: id = unsafe { NSString::alloc(nil).init_str(value) };
    unsafe { msg_send![s, autorelease] }
}

unsafe fn add_menu_item(menu: id, target: id, title: &str, selector: Sel, enabled: bool) {
    let item = unsafe {
        NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            ns_string(title),
            selector,
            ns_string(""),
        )
    };
    unsafe { item.setTarget_(target) };
    let _: () = unsafe { msg_send![item, setEnabled: if enabled { YES } else { NO }] };
    unsafe { menu.addItem_(item) };
    let _: () = unsafe { msg_send![item, release] };
}

pub fn show_tab_context_menu(
    close_all_label: &str,
    close_other_label: &str,
    reveal_label: &str,
    can_close_others: bool,
    can_reveal: bool,
) -> Option<MacTabContextMenuAction> {
    unsafe {
        SELECTED_TAB_MENU_ACTION.store(0, Ordering::SeqCst);

        let target_cls = target_class();
        let target: id = msg_send![target_cls, new];
        if target == nil {
            return None;
        }

        let menu = NSMenu::alloc(nil).initWithTitle_(ns_string("kPDF"));
        menu.setAutoenablesItems(NO);

        add_menu_item(menu, target, close_all_label, sel!(kpdfCloseAllTabs:), true);
        add_menu_item(
            menu,
            target,
            close_other_label,
            sel!(kpdfCloseOtherTabs:),
            can_close_others,
        );
        add_menu_item(
            menu,
            target,
            reveal_label,
            sel!(kpdfRevealInFinder:),
            can_reveal,
        );

        let location = NSEvent::mouseLocation(nil);
        let _: BOOL = msg_send![menu, popUpMenuPositioningItem:nil atLocation:location inView:nil];

        let _: () = msg_send![menu, release];
        let _: () = msg_send![target, release];
    }

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
