use crate::pipeline::h264::PipelineH264_1080p;
use crate::pipeline::hevc::PipelineHevcMain;
use crate::pipeline::{OutputStats, PipelineError, RecordPipeline};

pub(super) enum ActivePipeline {
    H264(PipelineH264_1080p),
    Hevc(PipelineHevcMain),
}

impl ActivePipeline {
    pub(super) fn push_frame(
        &mut self,
        surface: capy_shell_mac::IOSurfaceHandle,
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
