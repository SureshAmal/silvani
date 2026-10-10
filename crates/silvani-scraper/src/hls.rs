use crate::models::VideoStream;

/// Discover muxed HLS variants while retaining the master as the Auto choice.
/// Separate audio groups require the master playlist, so those variants are left to Auto.
pub(crate) fn variants(master_url: &str, manifest: &str) -> Vec<VideoStream> {
    let Ok(base) = reqwest::Url::parse(master_url) else {
        return Vec::new();
    };
    let mut quality = None;
    let mut streams = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if let Some(attributes) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            quality = if attributes
                .split(',')
                .any(|attribute| attribute.trim().starts_with("AUDIO="))
            {
                None
            } else {
                attributes
                    .split(',')
                    .find_map(|attribute| attribute.trim().strip_prefix("RESOLUTION="))
                    .and_then(|resolution| resolution.split_once('x'))
                    .and_then(|(_, height)| height.parse::<u32>().ok())
                    .map(|height| format!("{height}p"))
            };
        } else if !line.is_empty()
            && !line.starts_with('#')
            && let Some(quality) = quality.take()
            && let Ok(url) = base.join(line)
            && !streams
                .iter()
                .any(|stream: &VideoStream| stream.quality == quality)
        {
            streams.push(VideoStream {
                quality,
                url: url.into(),
            });
        }
    }
    streams.sort_by_key(|stream| {
        std::cmp::Reverse(
            stream
                .quality
                .trim_end_matches('p')
                .parse::<u32>()
                .unwrap_or(0),
        )
    });
    streams
}

#[cfg(test)]
mod tests {
    #[test]
    fn resolves_variants_and_preserves_separate_audio_groups() {
        let variants = super::variants(
            "https://example.test/video/master.m3u8",
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1280x720\n720/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=500000,RESOLUTION=640x360\n/360.m3u8\n#EXT-X-STREAM-INF:RESOLUTION=1920x1080,AUDIO=\"audio\"\n1080.m3u8",
        );
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].quality, "720p");
        assert_eq!(variants[0].url, "https://example.test/video/720/index.m3u8");
        assert_eq!(variants[1].url, "https://example.test/360.m3u8");
    }
}
