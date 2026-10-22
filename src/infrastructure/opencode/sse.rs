#[derive(Default)]
pub struct SseDecoder {
    line: Vec<u8>,
    data: Vec<String>,
    frame_bytes: usize,
}

impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        let mut result = Vec::new();
        for &byte in bytes {
            self.frame_bytes += 1;
            if self.frame_bytes > 1024 * 1024 {
                return Err("OpenCode event exceeds 1 MiB".into());
            }
            if byte != b'\n' {
                self.line.push(byte);
                continue;
            }
            if self.line.last() == Some(&b'\r') {
                self.line.pop();
            }
            let line = std::str::from_utf8(&self.line).map_err(|_| "Invalid event UTF-8")?;
            if line.is_empty() {
                if !self.data.is_empty() {
                    result.push(self.data.join("\n"));
                }
                self.data.clear();
                self.frame_bytes = 0;
            } else if let Some(data) = line.strip_prefix("data:") {
                self.data
                    .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
            }
            self.line.clear();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_handles_fragmentation_crlf_and_multiple_data_lines() {
        let mut decoder = SseDecoder::default();
        let mut frames = Vec::new();
        for byte in b": heartbeat\r\ndata: one\r\ndata: two\r\n\r\ndata: three\n\n" {
            frames.extend(decoder.push(&[*byte]).unwrap());
        }
        assert_eq!(frames, ["one\ntwo", "three"]);
    }

    #[test]
    fn sse_rejects_oversize_frames() {
        assert!(
            SseDecoder::default()
                .push(&vec![b'a'; 1024 * 1024 + 1])
                .is_err()
        );
    }
}
