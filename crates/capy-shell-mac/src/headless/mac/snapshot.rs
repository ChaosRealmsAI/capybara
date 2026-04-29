use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2::MainThreadMarker;
use objc2_app_kit::NSImage;
use objc2_core_foundation::{CFDictionary, CFRetained, CFString, CGPoint, CGRect, CGSize};
use objc2_core_image::CIImage;
use objc2_foundation::{
    NSCopying, NSDictionary, NSError, NSMutableDictionary, NSNumber, NSPoint, NSRect, NSSize,
    NSString,
};
use objc2_io_surface::{
    kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfacePixelFormat,
    kIOSurfaceWidth, IOSurfaceRef,
};
use objc2_quartz_core::CATransaction;
use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};

use super::MacHeadlessShell;
use crate::iosurface::PIXEL_FORMAT_BGRA;
use crate::webview::pump_main_run_loop;
use crate::{IOSurfaceHandle, ShellError};

impl MacHeadlessShell {
    pub(super) fn snapshot_iosurface(&self) -> Result<IOSurfaceHandle, ShellError> {
        autoreleasepool(|_| {
            let mtm = MainThreadMarker::new()
                .ok_or_else(|| ShellError::SnapshotFailed("snapshot not on main thread".into()))?;
            let cg_image = match take_snapshot_blocking(&self.web_view, mtm) {
                Ok(image) => cg_image_from_ns_image(&image)?,
                Err(first_take_snapshot_err) => {
                    self.web_view.displayIfNeeded();
                    CATransaction::flush();
                    pump_main_run_loop(Duration::from_millis(16));
                    let retry_mtm = MainThreadMarker::new().ok_or_else(|| {
                        ShellError::SnapshotFailed("snapshot retry not on main thread".into())
                    })?;
                    match take_snapshot_blocking(&self.web_view, retry_mtm) {
                        Ok(image) => cg_image_from_ns_image(&image)?,
                        Err(_retry_err) => cache_display_cg_image(&self.web_view).map_err(
                            |cache_err| {
                                ShellError::SnapshotFailed(format!(
                                    "takeSnapshot failed: {first_take_snapshot_err}; cacheDisplay fallback failed: {cache_err}"
                                ))
                            },
                        )?,
                    }
                }
            };

            let ci_image: Retained<CIImage> = unsafe { CIImage::imageWithCGImage(&cg_image) };
            let (w, h) = self.viewport;
            let target_w = f64::from(w);
            let target_h = f64::from(h);
            let cg_w = objc2_core_graphics::CGImage::width(Some(&cg_image)) as f64;
            let cg_h = objc2_core_graphics::CGImage::height(Some(&cg_image)) as f64;
            let scaled_ci = scale_ci_image(ci_image, cg_w, cg_h, target_w, target_h);

            let bounds = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: target_w,
                    height: target_h,
                },
            };
            let per_frame_surface_ref = create_output_iosurface(w, h).ok_or_else(|| {
                ShellError::SnapshotFailed("IOSurfaceCreate per-frame returned nil".into())
            })?;
            unsafe {
                self.ci_context.render_toIOSurface_bounds_colorSpace(
                    &scaled_ci,
                    &per_frame_surface_ref,
                    bounds,
                    Some(&self.color_space),
                );
            }

            Ok(IOSurfaceHandle::from_surface(per_frame_surface_ref))
        })
    }
}

fn scale_ci_image(
    ci_image: Retained<CIImage>,
    cg_w: f64,
    cg_h: f64,
    target_w: f64,
    target_h: f64,
) -> Retained<CIImage> {
    if (cg_w - target_w).abs() < 0.5 && (cg_h - target_h).abs() < 0.5 {
        return ci_image;
    }
    let tm = objc2_core_foundation::CGAffineTransform {
        a: target_w / cg_w,
        b: 0.0,
        c: 0.0,
        d: target_h / cg_h,
        tx: 0.0,
        ty: 0.0,
    };
    unsafe { ci_image.imageByApplyingTransform_highQualityDownsample(tm, true) }
}

fn take_snapshot_blocking(
    web_view: &WKWebView,
    mtm: MainThreadMarker,
) -> Result<Retained<NSImage>, ShellError> {
    autoreleasepool(|_| {
        let config = unsafe { WKSnapshotConfiguration::new(mtm) };
        unsafe {
            config.setAfterScreenUpdates(true);
        }
        let size = web_view.frame().size;
        let local_rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size,
        };
        unsafe {
            config.setRect(local_rect);
        }
        let snap_w = NSNumber::new_f64(size.width);
        unsafe {
            config.setSnapshotWidth(Some(&snap_w));
        }

        type Slot = Rc<RefCell<Option<Result<Retained<NSImage>, String>>>>;
        let slot: Slot = Rc::new(RefCell::new(None));
        let slot_for_block = slot.clone();

        let handler = RcBlock::new(move |image_ptr: *mut NSImage, err_ptr: *mut NSError| {
            let result = if !image_ptr.is_null() {
                match unsafe { Retained::retain(image_ptr) } {
                    Some(img) => Ok(img),
                    None => Err("Retained::retain returned None for NSImage".into()),
                }
            } else if !err_ptr.is_null() {
                let err_ref = unsafe { &*err_ptr };
                Err(err_ref.localizedDescription().to_string())
            } else {
                Err("takeSnapshot: both image and error are null".into())
            };
            *slot_for_block.borrow_mut() = Some(result);
        });

        unsafe {
            web_view.takeSnapshotWithConfiguration_completionHandler(Some(&config), &handler);
        }

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            pump_main_run_loop(Duration::from_millis(8));
            if slot.borrow().is_some() {
                break;
            }
        }

        let taken = slot
            .borrow_mut()
            .take()
            .ok_or_else(|| ShellError::SnapshotFailed("takeSnapshot timeout (3s)".into()))?;
        taken.map_err(ShellError::SnapshotFailed)
    })
}

fn cg_image_from_ns_image(
    image: &NSImage,
) -> Result<Retained<objc2_core_graphics::CGImage>, ShellError> {
    unsafe { image.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None) }
        .ok_or_else(|| {
            ShellError::SnapshotFailed("NSImage.CGImageForProposedRect returned nil".into())
        })
}

fn cache_display_cg_image(
    web_view: &WKWebView,
) -> Result<Retained<objc2_core_graphics::CGImage>, String> {
    let size = web_view.frame().size;
    let local_rect = NSRect {
        origin: NSPoint::new(0.0, 0.0),
        size: NSSize::new(size.width, size.height),
    };
    web_view.displayIfNeeded();
    let bitmap = web_view
        .bitmapImageRepForCachingDisplayInRect(local_rect)
        .ok_or_else(|| "bitmapImageRepForCachingDisplayInRect returned nil".to_string())?;
    web_view.cacheDisplayInRect_toBitmapImageRep(local_rect, &bitmap);
    bitmap
        .CGImage()
        .ok_or_else(|| "NSBitmapImageRep.CGImage returned nil".to_string())
}

pub(super) fn create_output_iosurface(w: u32, h: u32) -> Option<CFRetained<IOSurfaceRef>> {
    const BYTES_PER_ELEMENT: isize = 4;

    let dict: Retained<NSMutableDictionary<NSString, NSNumber>> = NSMutableDictionary::new();
    let entries: [(&CFString, Retained<NSNumber>); 5] = [
        (unsafe { kIOSurfaceWidth }, NSNumber::new_isize(w as isize)),
        (unsafe { kIOSurfaceHeight }, NSNumber::new_isize(h as isize)),
        (
            unsafe { kIOSurfaceBytesPerElement },
            NSNumber::new_isize(BYTES_PER_ELEMENT),
        ),
        (
            unsafe { kIOSurfaceBytesPerRow },
            NSNumber::new_isize(BYTES_PER_ELEMENT * (w as isize)),
        ),
        (
            unsafe { kIOSurfacePixelFormat },
            NSNumber::new_u32(PIXEL_FORMAT_BGRA),
        ),
    ];
    for (cf_key, value) in entries {
        let ns_key: &NSString = unsafe { &*(cf_key as *const CFString as *const NSString) };
        let key_proto: &ProtocolObject<dyn NSCopying> = ProtocolObject::from_ref(ns_key);
        unsafe {
            dict.setObject_forKey(&*value, key_proto);
        }
    }
    let ns_dict: &NSDictionary<NSString, NSNumber> = &dict;
    let cf_dict: &CFDictionary =
        unsafe { &*(ns_dict as *const NSDictionary<NSString, NSNumber> as *const CFDictionary) };
    unsafe { IOSurfaceRef::new(cf_dict) }
}
