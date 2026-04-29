use capy_shell_mac::IOSurfaceHandle;

use crate::pipeline::h264::PipelineH264_1080p;
use crate::pipeline::hevc::PipelineHevcMain;
use crate::pipeline::{ColorSpec, OutputStats, PipelineError, RecordOpts, RecordPipeline, VideoCodec};
use crate::record_loop::RecordConfig;

pub(super) enum ActivePipeline {
    H264(PipelineH264_1080p),
    Hevc(PipelineHevcMain),
}

impl ActivePipeline {
    pub(super) fn new(cfg: &RecordConfig) -> Result<Self, PipelineError> {
        let opts = RecordOpts {
            width: cfg.width,
            height: cfg.height,
            fps: cfg.fps,
            bitrate_bps: cfg.bitrate_bps,
            codec: cfg.codec,
            output: cfg.output.clone(),
            color: ColorSpec::BT709_SDR_8bit,
        };
        match cfg.codec {
            VideoCodec::H264 => Ok(Self::H264(PipelineH264_1080p::new(opts)?)),
            VideoCodec::HevcMain8 => Ok(Self::Hevc(PipelineHevcMain::new(opts)?)),
        }
    }

    pub(super) fn push_frame(
        &mut self,
        surface: IOSurfaceHandle,
        pts_ms: u64,
    ) -> Result<(), PipelineError> {
        match self {
            Self::H264(p) => p.push_frame(surface, pts_ms),
            Self::Hevc(p) => p.push_frame(surface, pts_ms),
        }
    }

    pub(super) fn finish(self) -> Result<OutputStats, PipelineError> {
        match self {
            Self::H264(p) => p.finish(),
            Self::Hevc(p) => p.finish(),
        }
    }
}
