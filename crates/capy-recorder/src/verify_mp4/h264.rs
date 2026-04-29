use super::{describe_matrix, describe_primaries, describe_transfer, MoovInfo};

pub(super) fn strip_emulation_prevention(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut i = 0;
    while i < nal.len() {
        if i + 2 < nal.len() && nal[i] == 0 && nal[i + 1] == 0 && nal[i + 2] == 3 {
            out.push(0);
            out.push(0);
            i += 3;
        } else {
            out.push(nal[i]);
            i += 1;
        }
    }
    out
}

/// Minimal bit reader for exp-Golomb on a byte slice (MSB-first).
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize, // bit position
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_bit(&mut self) -> Option<u32> {
        let byte_idx = self.pos / 8;
        let bit_idx = 7 - (self.pos % 8);
        if byte_idx >= self.data.len() {
            return None;
        }
        let b = (self.data[byte_idx] >> bit_idx) & 1;
        self.pos += 1;
        Some(u32::from(b))
    }

    fn read_bits(&mut self, n: u32) -> Option<u32> {
        if n > 32 {
            return None;
        }
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.read_bit()?;
        }
        Some(v)
    }

    /// Unsigned exp-Golomb (ue(v)).
    fn read_ue(&mut self) -> Option<u32> {
        let mut zeros = 0u32;
        while self.read_bit()? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        if zeros == 0 {
            return Some(0);
        }
        let suffix = self.read_bits(zeros)?;
        Some((1u32 << zeros) - 1 + suffix)
    }

    /// Signed exp-Golomb (se(v)).
    fn read_se(&mut self) -> Option<i32> {
        let k = self.read_ue()?;
        if k == 0 {
            return Some(0);
        }
        let mag = (k as i64 + 1) / 2;
        if k & 1 == 1 {
            Some(mag as i32)
        } else {
            Some(-(mag as i32))
        }
    }
}

/// Parse SPS RBSP up through VUI colour_description + ref_frames hint (7.3.2.1.1).
///
/// Only fields we need:
/// - profile_idc (to know if chroma_format_idc appears)
/// - chroma_format_idc (if profile in {100,110,122,244,44,83,86,118,128,138,139,134,135})
/// - log2_max_frame_num_minus4 ... etc (ignored via skip)
/// - max_num_ref_frames (we keep — used for has_b_frames hint)
/// - frame_mbs_only_flag
/// - frame_cropping ...
/// - vui_parameters_present_flag
///     - aspect / overscan / video_signal_type_present_flag
///         - colour_description_present_flag
///             - colour_primaries · transfer · matrix (each u8)
pub(super) fn parse_sps_vui(rbsp: &[u8], info: &mut MoovInfo) {
    let mut r = BitReader::new(rbsp);
    let Some(profile_idc) = r.read_bits(8) else {
        return;
    };
    // constraint_set_flags (8) + reserved_zero_2bits (already in 8)
    if r.read_bits(8).is_none() {
        return;
    }
    // level_idc
    if r.read_bits(8).is_none() {
        return;
    }
    // seq_parameter_set_id ue
    if r.read_ue().is_none() {
        return;
    }

    let has_chroma = matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    );
    if has_chroma {
        let chroma = match r.read_ue() {
            Some(v) => v,
            None => return,
        };
        if chroma == 3 {
            // separate_colour_plane_flag
            if r.read_bit().is_none() {
                return;
            }
        }
        // bit_depth_luma_minus8 ue
        if r.read_ue().is_none() {
            return;
        }
        // bit_depth_chroma_minus8 ue
        if r.read_ue().is_none() {
            return;
        }
        // qpprime_y_zero_transform_bypass_flag u1
        if r.read_bit().is_none() {
            return;
        }
        // seq_scaling_matrix_present_flag u1
        let scaling_present = match r.read_bit() {
            Some(v) => v,
            None => return,
        };
        if scaling_present == 1 {
            // 8 or 12 scaling_list flags each possibly followed by a list · complex.
            // Skip pragmatically — if present, we fail gracefully and leave VUI unset
            // (caller falls back to colr box). For Apple-written MP4 this is usually 0.
            return;
        }
    }

    // log2_max_frame_num_minus4 ue
    if r.read_ue().is_none() {
        return;
    }
    // pic_order_cnt_type ue
    let poc_type = match r.read_ue() {
        Some(v) => v,
        None => return,
    };
    if poc_type == 0 {
        if r.read_ue().is_none() {
            return;
        } // log2_max_pic_order_cnt_lsb_minus4
    } else if poc_type == 1 {
        if r.read_bit().is_none() {
            return;
        } // delta_pic_order_always_zero_flag
        if r.read_se().is_none() {
            return;
        } // offset_for_non_ref_pic
        if r.read_se().is_none() {
            return;
        } // offset_for_top_to_bottom_field
        let num_ref = match r.read_ue() {
            Some(v) => v,
            None => return,
        };
        for _ in 0..num_ref {
            if r.read_se().is_none() {
                return;
            }
        }
    }

    // max_num_ref_frames ue
    let _max_ref = match r.read_ue() {
        Some(v) => v,
        None => return,
    };
    // gaps_in_frame_num_value_allowed_flag u1
    if r.read_bit().is_none() {
        return;
    }
    // pic_width_in_mbs_minus1 ue
    if r.read_ue().is_none() {
        return;
    }
    // pic_height_in_map_units_minus1 ue
    if r.read_ue().is_none() {
        return;
    }
    // frame_mbs_only_flag u1
    let frame_mbs_only = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if frame_mbs_only == 0 && r.read_bit().is_none() {
        return; // mb_adaptive_frame_field_flag
    }
    // direct_8x8_inference_flag u1
    if r.read_bit().is_none() {
        return;
    }
    // frame_cropping_flag u1
    let crop = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if crop == 1 {
        if r.read_ue().is_none() {
            return;
        }
        if r.read_ue().is_none() {
            return;
        }
        if r.read_ue().is_none() {
            return;
        }
        if r.read_ue().is_none() {
            return;
        }
    }
    // vui_parameters_present_flag
    let vui = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if vui == 0 {
        return;
    }

    // VUI (E.1.1): aspect / overscan / video_signal_type / chroma_loc / timing / ...
    // aspect_ratio_info_present_flag
    let ar_present = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if ar_present == 1 {
        let idc = match r.read_bits(8) {
            Some(v) => v,
            None => return,
        };
        if idc == 255 {
            if r.read_bits(16).is_none() {
                return;
            }
            if r.read_bits(16).is_none() {
                return;
            }
        }
    }
    // overscan_info_present_flag
    let ov = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if ov == 1 && r.read_bit().is_none() {
        return; // overscan_appropriate_flag
    }
    // video_signal_type_present_flag
    let vs = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if vs == 0 {
        return;
    }
    // video_format u3
    if r.read_bits(3).is_none() {
        return;
    }
    // video_full_range_flag u1
    if r.read_bit().is_none() {
        return;
    }
    // colour_description_present_flag
    let cdp = match r.read_bit() {
        Some(v) => v,
        None => return,
    };
    if cdp == 0 {
        return;
    }
    let prim = match r.read_bits(8) {
        Some(v) => v,
        None => return,
    };
    let tf = match r.read_bits(8) {
        Some(v) => v,
        None => return,
    };
    let mat = match r.read_bits(8) {
        Some(v) => v,
        None => return,
    };
    info.color_primaries = describe_primaries(prim as u16);
    info.transfer = describe_transfer(tf as u16);
    info.matrix = describe_matrix(mat as u16);
}
