use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

pub struct WavSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

fn err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().unwrap())
}

pub fn read_wav(path: &str) -> io::Result<(WavSpec, Vec<f32>)> {
    let mut r = BufReader::new(File::open(path)?);

    let mut riff_header = [0u8; 12];
    r.read_exact(&mut riff_header)?;
    if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
        return Err(err("not a RIFF/WAVE file"));
    }

    let mut spec: Option<WavSpec> = None;
    let mut samples: Option<Vec<f32>> = None;

    // Chunks after the 12-byte header are back-to-back: [id:4][len:4][data:len],
    // padded to an even byte count. We keep whichever ones we recognize and skip the rest.
    loop {
        let mut chunk_header = [0u8; 8];
        if r.read_exact(&mut chunk_header).is_err() {
            break; // end of file
        }
        let chunk_id = &chunk_header[0..4];
        let chunk_len = read_u32(&chunk_header[4..8]) as usize;

        let mut data = vec![0u8; chunk_len];
        r.read_exact(&mut data)?;
        if chunk_len % 2 == 1 {
            let mut pad = [0u8; 1];
            r.read_exact(&mut pad)?; // RIFF pads odd-length chunks to a word boundary
        }

        match chunk_id {
            b"fmt " => {
                let audio_format = read_u16(&data[0..2]);
                if audio_format != 1 {
                    return Err(err("only uncompressed PCM WAV files are supported"));
                }
                spec = Some(WavSpec {
                    channels: read_u16(&data[2..4]),
                    sample_rate: read_u32(&data[4..8]),
                    bits_per_sample: read_u16(&data[14..16]),
                });
            }
            b"data" => {
                let bits = spec
                    .as_ref()
                    .ok_or_else(|| err("data chunk arrived before fmt chunk"))?
                    .bits_per_sample;
                samples = Some(decode_samples(&data, bits)?);
            }
            _ => {} // ignore chunks we don't care about (LIST, fact, etc.)
        }
    }

    let spec = spec.ok_or_else(|| err("missing fmt chunk"))?;
    let samples = samples.ok_or_else(|| err("missing data chunk"))?;
    Ok((spec, samples))
}

fn decode_samples(data: &[u8], bits_per_sample: u16) -> io::Result<Vec<f32>> {
    match bits_per_sample {
        16 => Ok(data
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect()),
        8 => Ok(data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect()),
        other => Err(err(&format!("unsupported bit depth: {other}"))),
    }
}

pub fn write_wav(path: &str, spec: &WavSpec, samples: &[f32]) -> io::Result<()> {
    let bytes_per_sample = (spec.bits_per_sample / 8) as u32;
    let data_len = samples.len() as u32 * bytes_per_sample;
    let byte_rate = spec.sample_rate * spec.channels as u32 * bytes_per_sample;
    let block_align = spec.channels * bytes_per_sample as u16;

    let mut w = BufWriter::new(File::create(path)?);

    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?; // file size minus the 8 bytes for "RIFF"+this field
    w.write_all(b"WAVE")?;

    w.write_all(b"fmt ")?;
    w.write_all(&16u32.to_le_bytes())?; // fmt chunk is always 16 bytes for plain PCM
    w.write_all(&1u16.to_le_bytes())?; // audio format 1 = PCM
    w.write_all(&spec.channels.to_le_bytes())?;
    w.write_all(&spec.sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&spec.bits_per_sample.to_le_bytes())?;

    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let value = (clamped * 32767.0) as i16;
        w.write_all(&value.to_le_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_16_bit() {
        let spec = WavSpec {
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
        };
        let samples: Vec<f32> = vec![0.0, 0.5, -0.5, 1.0, -1.0, 0.25];
        let path = std::env::temp_dir().join("tan_test_roundtrip.wav");
        let path_str = path.to_str().unwrap();

        write_wav(path_str, &spec, &samples).unwrap();
        let (read_spec, read_samples) = read_wav(path_str).unwrap();

        assert_eq!(read_spec.sample_rate, spec.sample_rate);
        assert_eq!(read_spec.channels, spec.channels);
        assert_eq!(read_spec.bits_per_sample, spec.bits_per_sample);
        assert_eq!(read_samples.len(), samples.len());
        for (a, b) in samples.iter().zip(read_samples.iter()) {
            assert!((a - b).abs() < 0.0001, "expected {a}, got {b}");
        }

        std::fs::remove_file(path).ok();
    }
}
