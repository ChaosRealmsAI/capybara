use std::path::Path;
use std::process::{Command, Output};

use crate::config::{NextFrameConfig, ResolvedNextFrameConfig};
use crate::error::{NextFrameError, NextFrameErrorCode};
use crate::ports::{
    CompileReport, CompositionArtifact, ExportOptions, ExportReport, NextFrameProjectPort,
    NextFrameRecorderPort, SnapshotOptions, SnapshotReport, ValidationReport,
};

#[derive(Debug, Clone)]
pub struct BinaryAdapter {
    resolved: ResolvedNextFrameConfig,
}

impl BinaryAdapter {
    pub fn new(config: NextFrameConfig) -> Result<Self, NextFrameError> {
        let resolved = config.resolve()?;
        if !resolved.nf.found || !resolved.recorder.found {
            return Err(NextFrameError::not_found("nf or nf-recorder was not found"));
        }
        Ok(Self { resolved })
    }

    pub fn new_recorder(config: NextFrameConfig) -> Result<Self, NextFrameError> {
        let resolved = config.resolve()?;
        if !resolved.recorder.found {
            return Err(NextFrameError::not_found("nf-recorder was not found"));
        }
        Ok(Self { resolved })
    }

    pub fn resolved(&self) -> &ResolvedNextFrameConfig {
        &self.resolved
    }

    fn nf_path(&self) -> Result<&Path, NextFrameError> {
        self.resolved
            .nf
            .path
            .as_deref()
            .ok_or_else(|| NextFrameError::not_found("nf path is unavailable"))
    }

    fn recorder_path(&self) -> Result<&Path, NextFrameError> {
        self.resolved
            .recorder
            .path
            .as_deref()
            .ok_or_else(|| NextFrameError::not_found("nf-recorder path is unavailable"))
    }
}

impl NextFrameProjectPort for BinaryAdapter {
    fn validate(&self, artifact: &CompositionArtifact) -> Result<ValidationReport, NextFrameError> {
        let args = vec![
            "composition".to_string(),
            "validate".to_string(),
            "--project".to_string(),
            artifact.project_slug.clone(),
            "--composition".to_string(),
            artifact.composition_id.clone(),
        ];
        let summary = run_command(
            self.nf_path()?,
            args,
            NextFrameErrorCode::CompositionInvalid,
        )?;
        Ok(ValidationReport {
            ok: true,
            command: summary.command,
            stdout: summary.stdout,
            stderr: summary.stderr,
        })
    }

    fn compile(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
    ) -> Result<CompileReport, NextFrameError> {
        let args = vec![
            "composition".to_string(),
            "compile".to_string(),
            "--project".to_string(),
            artifact.project_slug.clone(),
            "--composition".to_string(),
            artifact.composition_id.clone(),
            "--out".to_string(),
            out.display().to_string(),
        ];
        let summary = run_command(self.nf_path()?, args, NextFrameErrorCode::CompileFailed)?;
        Ok(CompileReport {
            ok: true,
            output: out.to_path_buf(),
            command: summary.command,
            stdout: summary.stdout,
            stderr: summary.stderr,
        })
    }
}

impl NextFrameRecorderPort for BinaryAdapter {
    fn snapshot(
        &self,
        source: &Path,
        out: &Path,
        options: SnapshotOptions,
    ) -> Result<SnapshotReport, NextFrameError> {
        let mut args = vec![
            "snapshot-source".to_string(),
            "--source".to_string(),
            source.display().to_string(),
            "--t-ms".to_string(),
            options.t_ms.to_string(),
            "--output".to_string(),
            out.display().to_string(),
        ];
        if let Some(resolution) = options.resolution.filter(|value| !value.trim().is_empty()) {
            args.push("--resolution".to_string());
            args.push(resolution);
        }
        let summary = run_command(
            self.recorder_path()?,
            args,
            NextFrameErrorCode::SnapshotFailed,
        )?;
        Ok(SnapshotReport {
            ok: true,
            output: out.to_path_buf(),
            command: summary.command,
            stdout: summary.stdout,
            stderr: summary.stderr,
        })
    }

    fn export(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, NextFrameError> {
        let args = vec![
            "export".to_string(),
            "--project".to_string(),
            artifact.project_slug.clone(),
            "--composition".to_string(),
            artifact.composition_id.clone(),
            "--profile".to_string(),
            options.profile,
            "--out".to_string(),
            out.display().to_string(),
        ];
        let summary = run_command(self.nf_path()?, args, NextFrameErrorCode::ExportFailed)?;
        Ok(ExportReport {
            ok: true,
            output: out.to_path_buf(),
            command: summary.command,
            stdout: summary.stdout,
            stderr: summary.stderr,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandSummary {
    command: Vec<String>,
    stdout: String,
    stderr: String,
}

fn run_command(
    program: &Path,
    args: Vec<String>,
    code: NextFrameErrorCode,
) -> Result<CommandSummary, NextFrameError> {
    let output = Command::new(program).args(&args).output().map_err(|err| {
        NextFrameError::new(
            code,
            format!("spawn {} failed: {err}", program.display()),
            "rerun capy nextframe doctor and verify the configured binary path",
        )
    })?;
    command_summary(program, args, output, code)
}

fn command_summary(
    program: &Path,
    args: Vec<String>,
    output: Output,
    code: NextFrameErrorCode,
) -> Result<CommandSummary, NextFrameError> {
    let mut command = vec![program.display().to_string()];
    command.extend(args);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(NextFrameError::new(
            code,
            format!("command failed: {}", command.join(" ")),
            "inspect stderr and rerun the same command after correcting the NextFrame input",
        ));
    }
    Ok(CommandSummary {
        command,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::BinaryAdapter;
    use crate::config::NextFrameConfig;

    #[test]
    fn adapter_rejects_missing_binaries() -> Result<(), String> {
        let result = BinaryAdapter::new(NextFrameConfig {
            nf_bin: Some("/definitely/not/nf".into()),
            recorder_bin: Some("/definitely/not/nf-recorder".into()),
            home: None,
            mode: None,
        });

        let err = match result {
            Ok(_) => return Err("missing binaries should fail".to_string()),
            Err(err) => err,
        };
        assert_eq!(err.body.code, "NEXTFRAME_NOT_FOUND");
        Ok(())
    }
}
