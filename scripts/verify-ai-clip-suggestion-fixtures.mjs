export function inspectSourceVideo(importResult) {
  return {
    filename: importResult.metadata.filename,
    duration_ms: importResult.metadata.duration_ms,
    width: importResult.metadata.width,
    height: importResult.metadata.height
  };
}

export function initialQueue(importA, importB) {
  return {
    schema_version: "capy.project-video-clip-queue.v1",
    project_id: "",
    project_name: "",
    updated_at: Date.now(),
    items: [
      {
        id: "queue-initial-camera-a",
        sequence: 1,
        composition_path: importA.composition_path,
        render_source_path: "",
        clip_id: "source",
        track_id: "video",
        scene: "Camera A opening detail",
        start_ms: 500,
        end_ms: 1700,
        duration_ms: 1200,
        source_video: inspectSourceVideo(importA),
        updated_at: Date.now()
      },
      {
        id: "queue-initial-camera-b",
        sequence: 2,
        composition_path: importB.composition_path,
        render_source_path: "",
        clip_id: "source",
        track_id: "video",
        scene: "Camera B product closeup",
        start_ms: 1000,
        end_ms: 2500,
        duration_ms: 1500,
        source_video: inspectSourceVideo(importB),
        updated_at: Date.now()
      }
    ]
  };
}

export function queueTotalDuration(items) {
  return items.reduce((total, item) => total + Math.max(1, Number(item.duration_ms || 0)), 0);
}
