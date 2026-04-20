use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

pub struct FfmpegWriter {
    child: Child,
    stdin: ChildStdin,
    frame_bytes: usize,
}

impl FfmpegWriter {
    /// Spawn an ffmpeg subprocess writing ProRes 4444 to `out_path`.
    ///
    /// The caller is responsible for writing exactly `width * height * 4`
    /// bytes of RGBA per frame.
    pub fn new(
        width: u32,
        height: u32,
        fps: u32,
        qscale: u32,
        out_path: &Path,
    ) -> anyhow::Result<Self> {
        let size = format!("{}x{}", width, height);
        let fps_str = format!("{}", fps);
        let qscale_str = format!("{}", qscale);

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &size,
            "-framerate", &fps_str,
            "-i", "-",
            "-c:v", "prores_ks",
            "-profile:v", "4444",
            "-pix_fmt", "yuva444p10le",
            "-vendor", "apl0",
            "-qscale:v", &qscale_str,
        ]);
        cmd.arg(out_path);

        cmd.stdin(Stdio::piped())
           .stdout(Stdio::null())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn ffmpeg (is it on PATH?): {}", e))?;
        let stdin = child.stdin.take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg stdin not captured"))?;

        Ok(Self {
            child,
            stdin,
            frame_bytes: (width as usize) * (height as usize) * 4,
        })
    }

    /// Write one frame of RGBA bytes to ffmpeg.
    pub fn write_frame(&mut self, rgba: &[u8]) -> std::io::Result<()> {
        assert_eq!(rgba.len(), self.frame_bytes, "frame size mismatch");
        self.stdin.write_all(rgba)
    }

    /// Close stdin and wait for ffmpeg to exit. Returns Err with stderr on failure.
    pub fn finish(mut self) -> anyhow::Result<()> {
        // Drop stdin so ffmpeg sees EOF.
        drop(self.stdin);
        let output = self.child.wait_with_output()?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffmpeg exited with status {:?}\nstderr:\n{}", output.status, stderr)
        }
    }
}

#[cfg(all(test, feature = "ffmpeg-tests"))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_solid_red_frames_and_decodes() {
        // 10 frames of solid red at 64x64, 10fps. Then use ffprobe to assert
        // the output's properties.
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.mov");
        let (w, h) = (64u32, 64u32);
        let mut writer = FfmpegWriter::new(w, h, 10, 11, &out).unwrap();
        let mut frame = vec![0u8; (w * h * 4) as usize];
        for i in 0..frame.len() / 4 {
            frame[i * 4 + 0] = 255; // R
            frame[i * 4 + 1] = 0;   // G
            frame[i * 4 + 2] = 0;   // B
            frame[i * 4 + 3] = 255; // A
        }
        for _ in 0..10 {
            writer.write_frame(&frame).unwrap();
        }
        writer.finish().unwrap();

        // Assert the output exists and ffprobe reports the right stream.
        assert!(out.exists(), "output file missing");

        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height,r_frame_rate,nb_read_frames,codec_name,pix_fmt",
                "-count_frames",
                "-of", "default=noprint_wrappers=1",
            ])
            .arg(&out)
            .output()
            .expect("ffprobe to run");
        assert!(probe.status.success(), "ffprobe failed: {}", String::from_utf8_lossy(&probe.stderr));
        let out_str = String::from_utf8_lossy(&probe.stdout);
        assert!(out_str.contains("width=64"), "stream info:\n{}", out_str);
        assert!(out_str.contains("height=64"), "stream info:\n{}", out_str);
        assert!(out_str.contains("codec_name=prores"), "stream info:\n{}", out_str);
        // ffmpeg's prores_ks maps the 4444 profile's decoded pixel format to
        // either yuva444p10le or yuva444p12le depending on the libavcodec
        // version (7.x reports 12le). The load-bearing property is
        // "4:4:4 YUV + alpha", so match the yuva444p prefix.
        assert!(
            out_str.contains("pix_fmt=yuva444p"),
            "expected pix_fmt=yuva444p*, stream info:\n{}",
            out_str
        );
        assert!(out_str.contains("nb_read_frames=10"), "stream info:\n{}", out_str);
    }
}
