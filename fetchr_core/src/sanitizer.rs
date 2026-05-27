use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Windows reserved names that cannot be used as filenames, even with extensions.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
];

/// Sanitizes a string to make it safe for filenames across Windows, macOS, and Linux.
/// Especially focused on preventing video editors (Premiere/Resolve) from crashing due to cursed characters.
pub fn sanitize_filename(filename: &str) -> String {
    if filename.is_empty() {
        return "download".to_string();
    }

    // 1. Unicode Normalization (NFKD)
    let normalized: String = filename.nfkd().collect();

    // 2. Remove emojis and keep only safe characters (alphanumeric, spaces, common punctuation)
    // Professional editing software hates emojis, smart quotes, and strange symbols.
    let mut sanitized = String::with_capacity(normalized.len());
    for c in normalized.chars() {
        if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '(' || c == ')' || c == '.' || c == '[' || c == ']' {
            // Replace characters that are illegal on Windows or dangerous: \ / : * ? " < > |
            sanitized.push(c);
        } else {
            // Replace symbols, emojis, and other unsafe chars with a safe placeholder
            match c {
                '&' => sanitized.push_str("and"),
                '+' => sanitized.push('_'),
                '@' => sanitized.push_str("at"),
                '#' => sanitized.push_str("num"),
                '%' => sanitized.push_str("pct"),
                _ => {
                    // Skip or replace emojis with a simple space or underscore
                    if !sanitized.ends_with('_') && !sanitized.is_empty() {
                        sanitized.push('_');
                    }
                }
            }
        }
    }

    // 3. Clean up leading/trailing spaces, dots, or underscores
    let mut cleaned = sanitized.trim().trim_end_matches('.').trim_end_matches('_').trim().to_string();

    if cleaned.is_empty() {
        cleaned = "media".to_string();
    }

    // 4. Windows Reserved Names Check
    let base_upper = cleaned.to_uppercase();
    let is_reserved = WINDOWS_RESERVED_NAMES.iter().any(|&r| {
        base_upper == r || base_upper.starts_with(&format!("{}.", r))
    });

    if is_reserved {
        cleaned = format!("fetchr_{}", cleaned);
    }

    // Ensure the filename is not insanely long (maximum file name is 255 bytes on most filesystems)
    // We truncate to 200 to leave safety buffer for path collisions / extensions
    if cleaned.len() > 200 {
        cleaned.truncate(200);
        cleaned = cleaned.trim().trim_end_matches('.').trim_end_matches('_').trim().to_string();
    }

    cleaned
}

/// Resolves path collisions safely. If the file already exists, appends " (1)", " (2)", etc.
/// Also enforces absolute path length boundaries.
pub fn resolve_path_collision(directory: &Path, filename: &str, extension: &str) -> PathBuf {
    let sanitized_base = sanitize_filename(filename);
    let ext = extension.trim_start_matches('.');

    // Enforce folder safety by standardizing paths
    let base_dir = directory.to_path_buf();
    
    // Construct initial path
    let mut final_filename = if ext.is_empty() {
        sanitized_base.clone()
    } else {
        format!("{}.{}", sanitized_base, ext)
    };

    // Path length protection: Windows MAX_PATH is 260 chars.
    // If the folder path + filename is too long, we aggressively truncate the filename.
    let dir_len = base_dir.to_string_lossy().len();
    let max_filename_len = if dir_len + 15 < 250 {
        250 - dir_len - 15 // leave 15 chars buffer for extensions and duplicate suffix " (99)"
    } else {
        50 // extreme absolute minimum
    };

    if sanitized_base.len() > max_filename_len {
        let truncated = &sanitized_base[..max_filename_len];
        final_filename = if ext.is_empty() {
            truncated.trim().to_string()
        } else {
            format!("{}.{}", truncated.trim(), ext)
        };
    }

    let mut target_path = base_dir.join(&final_filename);

    // If it doesn't exist, we are good to go!
    if !target_path.exists() {
        return target_path;
    }

    // If it does exist, loop and append counter
    let base_without_ext = if ext.is_empty() {
        sanitized_base
    } else {
        // use truncated if it was too long
        if sanitized_base.len() > max_filename_len {
            sanitized_base[..max_filename_len].trim().to_string()
        } else {
            sanitized_base
        }
    };

    let mut counter = 1;
    loop {
        let suffix = format!(" ({})", counter);
        let candidate_filename = if ext.is_empty() {
            format!("{}{}", base_without_ext, suffix)
        } else {
            format!("{}{}.{}", base_without_ext, suffix, ext)
        };

        let candidate_path = base_dir.join(&candidate_filename);
        if !candidate_path.exists() {
            return candidate_path;
        }
        counter += 1;
        
        // Safety breaker to prevent infinite loops
        if counter > 9999 {
            return base_dir.join(format!("{}_collision_{}.{}", base_without_ext, uuid::Uuid::new_v4().simple(), ext));
        }
    }
}

/// Hardens filesystem path from directory traversal and unsafe symlinks.
/// Normalizes paths before writing.
pub fn is_path_safe(base_dir: &Path, target_path: &Path) -> bool {
    let canonical_base = match base_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return false, // If base doesn't exist yet, we can't verify easily
    };

    // If target path doesn't exist yet, we check parent
    let parent = target_path.parent().unwrap_or(target_path);
    let canonical_target = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Target parent doesn't exist, check standard components
            // Simple traversal check: look for ".." in components
            for component in target_path.components() {
                if let std::path::Component::ParentDir = component {
                    return false;
                }
            }
            return true;
        }
    };

    canonical_target.starts_with(canonical_base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitization() {
        assert_eq!(sanitize_filename("🔥 THIS VIDEO CHANGED MY LIFE?!?!?!"), "THIS VIDEO CHANGED MY LIFE");
        assert_eq!(sanitize_filename("CON"), "fetchr_CON");
        assert_eq!(sanitize_filename("com1.mp4"), "fetchr_com1.mp4");
        assert_eq!(sanitize_filename("   leading and trailing dots...   "), "leading and trailing dots");
        assert_eq!(sanitize_filename("emoji 🎥 test 🚀"), "emoji _ test");
    }
}
