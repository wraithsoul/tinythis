use std::path::Path;

pub fn is_supported_video(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp4" | "mov" | "avi" | "webm" | "ogv" | "asx" | "mpeg" | "m4v" | "wmv" | "mpg"
    )
}
