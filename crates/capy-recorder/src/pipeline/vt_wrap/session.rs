use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType,
};
use objc2_core_video::{
    kCVImageBufferColorPrimaries_ITU_R_709_2, kCVImageBufferTransferFunction_ITU_R_709_2,
    kCVImageBufferYCbCrMatrix_ITU_R_709_2, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey,
    kCVPixelFormatType_32BGRA,
};
use objc2_video_toolbox::{
    kVTCompressionPropertyKey_AllowFrameReordering, kVTCompressionPropertyKey_AverageBitRate,
    kVTCompressionPropertyKey_ColorPrimaries, kVTCompressionPropertyKey_ConstantBitRate,
    kVTCompressionPropertyKey_DataRateLimits, kVTCompressionPropertyKey_ExpectedFrameRate,
    kVTCompressionPropertyKey_MaxAllowedFrameQP, kVTCompressionPropertyKey_MaxKeyFrameInterval,
    kVTCompressionPropertyKey_ProfileLevel, kVTCompressionPropertyKey_Quality,
    kVTCompressionPropertyKey_RealTime, kVTCompressionPropertyKey_TransferFunction,
    kVTCompressionPropertyKey_YCbCrMatrix, kVTProfileLevel_H264_Main_AutoLevel,
    kVTProfileLevel_HEVC_Main_AutoLevel, kVTPropertyNotSupportedErr, VTCompressionSession,
    VTSession, VTSessionSetProperty,
};

use super::{PipelineError, VideoCodec};

pub(super) fn configure_session(
    session: &VTCompressionSession,
    fps: i32,
    bitrate_bps: i32,
    codec: VideoCodec,
) -> Result<(), PipelineError> {
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_AllowFrameReordering },
        CFBoolean::new(false).as_ref(),
    )?;
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_ExpectedFrameRate },
        CFNumber::new_i32(fps).as_ref(),
    )?;
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_MaxKeyFrameInterval },
        CFNumber::new_i32(60).as_ref(),
    )?;

    let use_constant_bitrate = set_bitrate(session, bitrate_bps, codec)?;
    if codec == VideoCodec::HevcMain8 {
        configure_hevc_quality(session, use_constant_bitrate)?;
    }
    set_codec_profile(session, codec)?;
    set_bt709_color(session)?;
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_RealTime },
        CFBoolean::new(false).as_ref(),
    )
}

fn set_bitrate(
    session: &VTCompressionSession,
    bitrate_bps: i32,
    codec: VideoCodec,
) -> Result<bool, PipelineError> {
    let use_constant_bitrate = if codec == VideoCodec::HevcMain8 {
        try_set_prop(
            session,
            unsafe { kVTCompressionPropertyKey_ConstantBitRate },
            CFNumber::new_i32(bitrate_bps).as_ref(),
        )?
    } else {
        false
    };
    if use_constant_bitrate {
        return Ok(true);
    }

    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_AverageBitRate },
        CFNumber::new_i32(bitrate_bps).as_ref(),
    )?;
    let max_bytes_per_second = ((i64::from(bitrate_bps).max(1) * 2) / 8).max(1);
    let max_bytes = CFNumber::new_i64(max_bytes_per_second);
    let window_seconds = CFNumber::new_f64(1.0);
    let data_rate_limits: CFRetained<CFArray<CFType>> =
        CFArray::from_objects(&[max_bytes.as_ref(), window_seconds.as_ref()]);
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_DataRateLimits },
        data_rate_limits.as_ref(),
    )?;
    Ok(false)
}

fn configure_hevc_quality(
    session: &VTCompressionSession,
    use_constant_bitrate: bool,
) -> Result<(), PipelineError> {
    let quality = CFNumber::new_f64(if use_constant_bitrate { 1.0 } else { 0.9 });
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_Quality },
        quality.as_ref(),
    )?;
    let max_allowed_qp = CFNumber::new_i32(18);
    let _supports_qp_cap = try_set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_MaxAllowedFrameQP },
        max_allowed_qp.as_ref(),
    )?;
    Ok(())
}

fn set_codec_profile(
    session: &VTCompressionSession,
    codec: VideoCodec,
) -> Result<(), PipelineError> {
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_ProfileLevel },
        match codec {
            VideoCodec::H264 => unsafe { kVTProfileLevel_H264_Main_AutoLevel }.as_ref(),
            VideoCodec::HevcMain8 => unsafe { kVTProfileLevel_HEVC_Main_AutoLevel }.as_ref(),
        },
    )
}

fn set_bt709_color(session: &VTCompressionSession) -> Result<(), PipelineError> {
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_ColorPrimaries },
        unsafe { kCVImageBufferColorPrimaries_ITU_R_709_2 }.as_ref(),
    )?;
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_TransferFunction },
        unsafe { kCVImageBufferTransferFunction_ITU_R_709_2 }.as_ref(),
    )?;
    set_prop(
        session,
        unsafe { kVTCompressionPropertyKey_YCbCrMatrix },
        unsafe { kCVImageBufferYCbCrMatrix_ITU_R_709_2 }.as_ref(),
    )
}

fn set_prop(
    session: &VTCompressionSession,
    key: &CFString,
    value: &CFType,
) -> Result<(), PipelineError> {
    let status = unsafe {
        let vt_session: &VTSession = &*(session as *const VTCompressionSession as *const VTSession);
        VTSessionSetProperty(vt_session, key, Some(value))
    };
    if status != 0 {
        return Err(PipelineError::EncoderInitFailed);
    }
    Ok(())
}

fn try_set_prop(
    session: &VTCompressionSession,
    key: &CFString,
    value: &CFType,
) -> Result<bool, PipelineError> {
    let status = unsafe {
        let vt_session: &VTSession = &*(session as *const VTCompressionSession as *const VTSession);
        VTSessionSetProperty(vt_session, key, Some(value))
    };
    if status == 0 {
        return Ok(true);
    }
    if status == kVTPropertyNotSupportedErr {
        return Ok(false);
    }
    Err(PipelineError::EncoderInitFailed)
}

pub(super) fn pixel_buffer_attributes(
    width: i32,
    height: i32,
) -> CFRetained<CFDictionary<CFType, CFType>> {
    let w = CFNumber::new_i32(width);
    let h = CFNumber::new_i32(height);
    let fmt = CFNumber::new_i32(kCVPixelFormatType_32BGRA as i32);
    let iosurface = CFDictionary::<CFType, CFType>::empty();
    unsafe {
        CFDictionary::<CFType, CFType>::from_slices(
            &[
                kCVPixelBufferWidthKey.as_ref(),
                kCVPixelBufferHeightKey.as_ref(),
                kCVPixelBufferPixelFormatTypeKey.as_ref(),
                kCVPixelBufferIOSurfacePropertiesKey.as_ref(),
            ],
            &[w.as_ref(), h.as_ref(), fmt.as_ref(), iosurface.as_ref()],
        )
    }
}
