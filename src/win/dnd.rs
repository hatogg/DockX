//! OLE drag & drop: dropping files onto the dock / island, dragging files and text out of the island.

use super::util::*;
use windows::core::implement;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DropKind {
    Dock,
    Island,
}

/// What the main loop does with drag events (it owns the dock and island).
pub trait DropSink {
    fn drag_over(&self, kind: DropKind, x: i32, y: i32, files: bool, allowed: DROPEFFECT) -> DROPEFFECT;
    fn drag_leave(&self, kind: DropKind);
    fn drop_files(&self, kind: DropKind, files: Vec<String>);
}

#[implement(IDropTarget)]
pub struct DropTarget {
    kind: DropKind,
    sink: &'static dyn DropSink,
    has_files: std::cell::Cell<bool>,
}

impl DropTarget {
    pub fn new(kind: DropKind, sink: &'static dyn DropSink) -> DropTarget {
        DropTarget { kind, sink, has_files: std::cell::Cell::new(false) }
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(&self, data: Option<&IDataObject>, _keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let files = data.map(|d| has_hdrop(d)).unwrap_or(false);
        self.has_files.set(files);
        unsafe {
            let allowed = *effect;
            *effect = self.sink.drag_over(self.kind, pt.x, pt.y, files, allowed);
        }
        Ok(())
    }
    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        unsafe {
            let allowed = *effect;
            *effect = self.sink.drag_over(self.kind, pt.x, pt.y, self.has_files.get(), allowed);
        }
        Ok(())
    }
    fn DragLeave(&self) -> windows::core::Result<()> {
        self.sink.drag_leave(self.kind);
        Ok(())
    }
    fn Drop(&self, data: Option<&IDataObject>, _keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let files = data.map(|d| files_of(d)).unwrap_or_default();
        unsafe {
            let allowed = *effect;
            let e = self.sink.drag_over(self.kind, pt.x, pt.y, !files.is_empty(), allowed);
            *effect = e;
            if e != DROPEFFECT_NONE {
                self.sink.drop_files(self.kind, files);
            } else {
                self.sink.drag_leave(self.kind);
            }
        }
        Ok(())
    }
}

fn hdrop_fmt() -> FORMATETC {
    FORMATETC { cfFormat: 15, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 }
}

fn has_hdrop(d: &IDataObject) -> bool {
    unsafe { d.QueryGetData(&hdrop_fmt()).is_ok() }
}

pub fn files_of(d: &IDataObject) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let Ok(mut med) = d.GetData(&hdrop_fmt()) else { return out };
        let hdrop = HDROP(med.u.hGlobal.0);
        let n = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        for i in 0..n {
            let len = DragQueryFileW(hdrop, i, None);
            let mut buf = vec![0u16; len as usize + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            out.push(from_wide(&buf));
        }
        ReleaseStgMedium(&mut med);
    }
    out
}

/// Drags files out (shell data object, real drag images). Returns the effect (NONE = cancelled).
pub fn drag_files_out(hwnd: HWND, paths: &[String]) -> DROPEFFECT {
    unsafe {
        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
        for p in paths {
            let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            if SHParseDisplayName(&h(p), None, &mut pidl, 0, None).is_ok() && !pidl.is_null() {
                pidls.push(pidl);
            }
        }
        let mut effect = DROPEFFECT_NONE;
        if !pidls.is_empty() {
            let consts: Vec<*const ITEMIDLIST> = pidls.iter().map(|p| *p as *const _).collect();
            if let Ok(arr) = SHCreateShellItemArrayFromIDLists(&consts) {
                if let Ok(data) = arr.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) {
                    effect = SHDoDragDrop(hwnd, &data, None, DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK).unwrap_or(DROPEFFECT_NONE);
                }
            }
        }
        for p in pidls {
            CoTaskMemFree(Some(p as *const _));
        }
        effect
    }
}

/// Drags plain text out (into a chat, a document…).
pub fn drag_text_out(hwnd: HWND, text: &str) -> DROPEFFECT {
    unsafe {
        let Ok(data) = SHCreateDataObject::<_, IDataObject>(None, None, None) else { return DROPEFFECT_NONE };
        let Some(g) = hglobal_text(text) else { return DROPEFFECT_NONE };
        let fmt = FORMATETC { cfFormat: 13, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 };
        let med = STGMEDIUM { tymed: TYMED_HGLOBAL.0 as u32, u: STGMEDIUM_0 { hGlobal: g }, pUnkForRelease: std::mem::ManuallyDrop::new(None) };
        if data.SetData(&fmt, &med, true).is_err() {
            return DROPEFFECT_NONE;
        }
        SHDoDragDrop(hwnd, &data, None, DROPEFFECT_COPY).unwrap_or(DROPEFFECT_NONE)
    }
}
