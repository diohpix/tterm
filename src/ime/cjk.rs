use unicode_width::UnicodeWidthChar;

/// Check if a character is a double-width character (CJK, emoji, etc.)
pub fn is_double_width_char(ch: char) -> bool {
    ch.width().unwrap_or(1) == 2
}

/// Check if a string contains any double-width characters
pub fn contains_double_width_chars(text: &str) -> bool {
    text.chars().any(is_double_width_char)
}

/// Check if a character is Korean jamo (individual consonant/vowel)
pub fn is_korean_jamo(ch: char) -> bool {
    crate::ime::korean::is_consonant(ch) || crate::ime::korean::is_vowel(ch)
}

/// Check if a character is a completed CJK character (not in composition)
pub fn is_completed_cjk_char(ch: char) -> bool {
    is_double_width_char(ch) && !is_korean_jamo(ch)
}

/// Determine the appropriate cursor width for a character
pub fn get_cursor_width_for_char(ch: char) -> f32 {
    if is_double_width_char(ch) {
        2.0
    } else {
        1.0
    }
}

/// Check if we should show double-wide cursor for current text position
/// This considers both Korean composition and completed CJK characters
pub fn should_show_double_cursor(composing_char: Option<char>, text_at_cursor: Option<char>) -> bool {
    // If we have a Korean composition, always show double cursor
    if composing_char.is_some() {
        return true;
    }
    
    // If there's a completed CJK character at cursor position, show double cursor
    if let Some(ch) = text_at_cursor {
        return is_double_width_char(ch);
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double_width_detection() {
        // Korean characters
        assert!(is_double_width_char('한'));
        assert!(is_double_width_char('글'));
        
        // Chinese characters
        assert!(is_double_width_char('中'));
        assert!(is_double_width_char('文'));
        
        // Japanese characters
        assert!(is_double_width_char('日'));
        assert!(is_double_width_char('本'));
        assert!(is_double_width_char('あ'));
        assert!(is_double_width_char('ア'));
        
        // Emoji
        assert!(is_double_width_char('🔥'));
        assert!(is_double_width_char('🚀'));
        
        // ASCII characters (should be single width)
        assert!(!is_double_width_char('a'));
        assert!(!is_double_width_char('1'));
        assert!(!is_double_width_char(' '));
        
        // Korean jamo (should be single width in display but double in composition)
        assert!(!is_double_width_char('ㄱ'));
        assert!(!is_double_width_char('ㅏ'));
    }

    #[test]
    fn test_korean_jamo_detection() {
        assert!(is_korean_jamo('ㄱ'));
        assert!(is_korean_jamo('ㅏ'));
        assert!(!is_korean_jamo('한'));
        assert!(!is_korean_jamo('中'));
        assert!(!is_korean_jamo('a'));
    }

    #[test]
    fn test_completed_cjk_detection() {
        // Completed CJK characters
        assert!(is_completed_cjk_char('한'));
        assert!(is_completed_cjk_char('中'));
        assert!(is_completed_cjk_char('日'));
        assert!(is_completed_cjk_char('🔥'));
        
        // Korean jamo (not completed)
        assert!(!is_completed_cjk_char('ㄱ'));
        assert!(!is_completed_cjk_char('ㅏ'));
        
        // ASCII
        assert!(!is_completed_cjk_char('a'));
    }

    #[test]
    fn test_cursor_width() {
        assert_eq!(get_cursor_width_for_char('한'), 2.0);
        assert_eq!(get_cursor_width_for_char('中'), 2.0);
        assert_eq!(get_cursor_width_for_char('🔥'), 2.0);
        assert_eq!(get_cursor_width_for_char('a'), 1.0);
        assert_eq!(get_cursor_width_for_char('ㄱ'), 1.0);
    }
}
