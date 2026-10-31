/// Turns a number of bytes into something a person can read, like "4.2 GB".
///
/// We count in thousands, the way macOS, Ubuntu and disk manufacturers do,
/// so the numbers Roomy shows match the ones on the box.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];

    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for u in UNITS {
        value /= 1000.0;
        unit = u;
        if value < 1000.0 {
            break;
        }
    }
    if value < 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn small_numbers_are_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
    }

    #[test]
    fn big_numbers_get_units() {
        assert_eq!(format_size(1_000), "1.0 KB");
        assert_eq!(format_size(1_536_000), "1.5 MB");
        assert_eq!(format_size(42_000_000_000), "42 GB");
        assert_eq!(format_size(u64::MAX), "18 EB");
    }
}
