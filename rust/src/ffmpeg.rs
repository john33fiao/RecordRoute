use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub build_script_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeInfo {
    pub channels: u32,
    pub channel_layout: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionOutputs {
    pub merged_mono_wav: PathBuf,
    pub split_mono_wavs: Vec<SplitMonoOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMonoOutput {
    pub channel_index: u32,
    pub path: PathBuf,
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
        let build_script_path = repo_root.join("scripts/build_ffmpeg.sh");
        let install_bin = locate_install_bin(repo_root)?;
        let ffmpeg_path = install_bin.join("ffmpeg");
        let ffprobe_path = install_bin.join("ffprobe");

        if ffmpeg_path.is_file() && ffprobe_path.is_file() {
            return Ok(Self {
                ffmpeg_path,
                ffprobe_path,
                build_script_path,
            });
        }

        Err(format!(
            "local ffmpeg toolchain not found. Build it first with {}",
            build_script_path.display()
        ))
    }
}

impl ConversionOutputs {
    pub fn new(job_dir: &Path, channels: u32) -> Self {
        let split_mono_wavs = (0..channels)
            .map(|index| SplitMonoOutput {
                channel_index: index + 1,
                path: job_dir.join(format!("channel_{:02}.wav", index + 1)),
            })
            .collect();

        Self {
            merged_mono_wav: job_dir.join("mono_mix.wav"),
            split_mono_wavs,
        }
    }

    pub fn cleanup_partial_files(&self) {
        for split in &self.split_mono_wavs {
            let _ = fs::remove_file(&split.path);
        }
        let _ = fs::remove_file(&self.merged_mono_wav);
    }
}

pub fn probe_audio_input(toolchain: &Toolchain, input: &Path) -> Result<ProbeInfo, String> {
    let output = Command::new(&toolchain.ffprobe_path)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a:0")
        .arg("-show_entries")
        .arg("stream=channels,channel_layout")
        .arg("-of")
        .arg("json")
        .arg(input)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute ffprobe {}: {error}",
                toolchain.ffprobe_path.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "ffprobe failed for {}: {}",
            input.display(),
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.as_str()
            }
        ));
    }

    let response: ProbeResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse ffprobe output: {error}"))?;
    let stream = response.streams.into_iter().next().ok_or_else(|| {
        format!(
            "ffprobe did not return an audio stream for {}",
            input.display()
        )
    })?;

    let channels = stream.channels.ok_or_else(|| {
        format!(
            "ffprobe did not report channel count for {}",
            input.display()
        )
    })?;
    if channels == 0 {
        return Err(format!(
            "ffprobe reported zero audio channels for {}",
            input.display()
        ));
    }

    Ok(ProbeInfo {
        channels,
        channel_layout: stream.channel_layout,
    })
}

pub fn run_conversion(
    toolchain: &Toolchain,
    input: &Path,
    channels: u32,
    outputs: &ConversionOutputs,
) -> Result<(), String> {
    let mut command = Command::new(&toolchain.ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-vn")
        .arg("-sn")
        .arg("-dn")
        .arg("-filter_complex")
        .arg(build_filter_complex(channels));

    for split in &outputs.split_mono_wavs {
        command
            .arg("-map")
            .arg(format!("[split{:02}]", split.channel_index))
            .arg("-c:a")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg(&split.path);
    }

    command
        .arg("-map")
        .arg("[mix]")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg(&outputs.merged_mono_wav);

    let output = command.output().map_err(|error| {
        format!(
            "failed to execute ffmpeg {}: {error}",
            toolchain.ffmpeg_path.display()
        )
    })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!(
        "ffmpeg conversion failed for {}: {}",
        input.display(),
        if stderr.is_empty() {
            "unknown error"
        } else {
            stderr.as_str()
        }
    ))
}

pub fn build_filter_complex(channels: u32) -> String {
    let merged_inputs = (0..channels)
        .map(|index| format!("c{index}"))
        .collect::<Vec<_>>()
        .join("+");
    let mut filters = vec![format!("[0:a:0]pan=mono|c0<{merged_inputs}[mix]")];

    for index in 0..channels {
        filters.push(format!(
            "[0:a:0]pan=mono|c0=c{index}[split{:02}]",
            index + 1
        ));
    }

    filters.join(";")
}

fn locate_install_bin(repo_root: &Path) -> Result<PathBuf, String> {
    let install_bin = repo_root
        .join(".build/ffmpeg")
        .join(target_dir_name())
        .join("install/bin");

    if install_bin.join("ffmpeg").is_file() && install_bin.join("ffprobe").is_file() {
        Ok(install_bin)
    } else {
        Err(format!(
            "local ffmpeg toolchain not found. Build it first with {}",
            repo_root.join("scripts/build_ffmpeg.sh").display()
        ))
    }
}

pub fn target_dir_name() -> String {
    format!("{}-{}", normalized_os(), normalized_arch())
}

fn normalized_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        other => other,
    }
}

fn normalized_arch() -> &'static str {
    match std::env::consts::ARCH {
        "arm64" => "aarch64",
        "amd64" => "x86_64",
        other => other,
    }
}

#[derive(serde::Deserialize)]
struct ProbeResponse {
    #[serde(default)]
    streams: Vec<ProbeStream>,
}

#[derive(serde::Deserialize)]
struct ProbeStream {
    channels: Option<u32>,
    channel_layout: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_filter_graph_for_mono() {
        assert_eq!(
            build_filter_complex(1),
            "[0:a:0]pan=mono|c0<c0[mix];[0:a:0]pan=mono|c0=c0[split01]"
        );
    }

    #[test]
    fn builds_filter_graph_for_stereo() {
        assert_eq!(
            build_filter_complex(2),
            "[0:a:0]pan=mono|c0<c0+c1[mix];[0:a:0]pan=mono|c0=c0[split01];[0:a:0]pan=mono|c0=c1[split02]"
        );
    }

    #[test]
    fn builds_filter_graph_for_four_channels() {
        assert_eq!(
            build_filter_complex(4),
            "[0:a:0]pan=mono|c0<c0+c1+c2+c3[mix];[0:a:0]pan=mono|c0=c0[split01];[0:a:0]pan=mono|c0=c1[split02];[0:a:0]pan=mono|c0=c2[split03];[0:a:0]pan=mono|c0=c3[split04]"
        );
    }

    #[test]
    fn conversion_outputs_match_expected_names() {
        let outputs = ConversionOutputs::new(Path::new("/tmp/job"), 3);

        assert_eq!(
            outputs.merged_mono_wav,
            PathBuf::from("/tmp/job/mono_mix.wav")
        );
        assert_eq!(
            outputs.split_mono_wavs[0].path,
            PathBuf::from("/tmp/job/channel_01.wav")
        );
        assert_eq!(
            outputs.split_mono_wavs[1].path,
            PathBuf::from("/tmp/job/channel_02.wav")
        );
        assert_eq!(
            outputs.split_mono_wavs[2].path,
            PathBuf::from("/tmp/job/channel_03.wav")
        );
    }

    #[test]
    fn parses_ffprobe_json() {
        let response: ProbeResponse =
            serde_json::from_str(r#"{"streams":[{"channels":4,"channel_layout":"4.0"}]}"#)
                .expect("probe json");

        let stream = response.streams.into_iter().next().expect("stream");
        assert_eq!(stream.channels, Some(4));
        assert_eq!(stream.channel_layout.as_deref(), Some("4.0"));
    }
}
