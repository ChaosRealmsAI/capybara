use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_foundation::{CFArray, CFBoolean, CFDictionary, CFRetained, CFString};
use objc2_core_media::{
    kCMSampleAttachmentKey_NotSync, CMBlockBuffer, CMSampleBuffer, CMTime, CMVideoFormatDescription,
};
use objc2_video_toolbox::VTEncodeInfoFlags;

use super::{CompressedFrame, SendableFormatDescription, VtCallbackState};

#[allow(unsafe_code)]
pub(super) unsafe extern "C-unwind" fn vt_output_callback(
    output_callback_ref_con: *mut c_void,
    _source_frame_ref_con: *mut c_void,
    status: i32,
    info_flags: VTEncodeInfoFlags,
    sample_buffer: *mut CMSampleBuffer,
) {
    let state: &VtCallbackState = unsafe { &*(output_callback_ref_con as *const VtCallbackState) };
    state.release_source_buffer();

    if status != 0 {
        state.record_error(format!("VT output callback status {status}"));
        return;
    }
    if info_flags.contains(VTEncodeInfoFlags::FrameDropped) {
        state.record_error("VT dropped a frame".to_string());
        return;
    }

    let Some(sample_nn) = NonNull::new(sample_buffer) else {
        state.record_error("VT callback returned null CMSampleBuffer".to_string());
        return;
    };
    let sample = unsafe { CFRetained::retain(sample_nn) };
    let sample_ref: &CMSampleBuffer = &sample;

    let frame = match compressed_frame(sample_ref) {
        Ok(frame) => frame,
        Err(msg) => {
            state.record_error(msg);
            return;
        }
    };
    state.output_queue.push(frame);
}

fn compressed_frame(sample: &CMSampleBuffer) -> Result<CompressedFrame, String> {
    let (pts_ms, dts_ms) = timestamps_ms(sample);
    let data = copy_sample_bytes(sample)?;
    let Some(format_description) = format_description(sample) else {
        return Err("CMSampleBuffer missing format description".to_string());
    };
    Ok(CompressedFrame {
        data,
        pts_ms,
        dts_ms,
        is_keyframe: is_sync_frame(sample),
        format_description: SendableFormatDescription::new(format_description),
    })
}

fn timestamps_ms(sample: &CMSampleBuffer) -> (u64, u64) {
    let pts = unsafe { sample.presentation_time_stamp() };
    let dts = unsafe { sample.decode_time_stamp() };
    let pts_ms = cmtime_to_ms(pts);
    let dts_ms = if dts.timescale <= 0 || dts.value < 0 {
        pts_ms
    } else {
        cmtime_to_ms(dts)
    };
    (pts_ms, dts_ms)
}

fn cmtime_to_ms(t: CMTime) -> u64 {
    if t.timescale <= 0 || t.value < 0 {
        return 0;
    }
    let num = (t.value as i128) * 1000i128;
    let den = t.timescale as i128;
    if den == 0 {
        return 0;
    }
    let ms = num / den;
    if ms < 0 {
        0
    } else {
        ms as u64
    }
}

fn is_sync_frame(sample: &CMSampleBuffer) -> bool {
    let attachments_opt = unsafe { sample.sample_attachments_array(false) };
    let Some(attachments) = attachments_opt else {
        return true;
    };
    let count = CFArray::count(&attachments);
    if count == 0 {
        return true;
    }
    let dict_ptr = unsafe { CFArray::value_at_index(&attachments, 0) };
    if dict_ptr.is_null() {
        return true;
    }
    let dict = unsafe { &*(dict_ptr as *const CFDictionary) };
    let not_sync_key = unsafe { kCMSampleAttachmentKey_NotSync };
    let value_ptr = unsafe { dict.value(not_sync_key as *const CFString as *const _) };
    if value_ptr.is_null() {
        return true;
    }
    let b = unsafe { &*(value_ptr as *const CFBoolean) };
    !b.as_bool()
}

fn copy_sample_bytes(sample: &CMSampleBuffer) -> Result<Vec<u8>, String> {
    let Some(block) = (unsafe { sample.data_buffer() }) else {
        return Err("CMSampleBuffer had no data buffer".to_string());
    };
    copy_block_bytes(&block)
}

fn copy_block_bytes(block: &CMBlockBuffer) -> Result<Vec<u8>, String> {
    let total = unsafe { block.data_length() };
    if total == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; total];
    let Some(dest) = NonNull::new(out.as_mut_ptr() as *mut c_void) else {
        return Err("CMBlockBuffer dest pointer null".to_string());
    };
    let status = unsafe { block.copy_data_bytes(0, total, dest) };
    if status != 0 {
        return Err(format!("CMBlockBufferCopyDataBytes status {status}"));
    }
    Ok(out)
}

fn format_description(sample: &CMSampleBuffer) -> Option<CFRetained<CMVideoFormatDescription>> {
    unsafe { sample.format_description() }
}
