use std::io::Write;

use crate::error::Result;

pub fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    std::io::stdout().flush()?;

    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(parse_yes(&s))
}

fn parse_yes(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    s == "y" || s == "yes"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yes_accepts_common_yes() {
        assert!(parse_yes("y"));
        assert!(parse_yes("Y"));
        assert!(parse_yes(" yes "));
        assert!(!parse_yes(""));
        assert!(!parse_yes("n"));
        assert!(!parse_yes("no"));
        assert!(!parse_yes("maybe"));
    }
}
