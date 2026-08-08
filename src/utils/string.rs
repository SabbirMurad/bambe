pub fn strip_prefix(prefix: &str, input: &str) -> Result<String, String> {
    if input.starts_with(prefix) {
        Ok(input[prefix.len()..].to_string())
    } else {
        Err(format!("Input does not start with the given prefix"))
    }
}

pub fn parse_comma_separated(input: &str) -> Result<Vec<&str>, String> {
    let trimmed = input.trim();

    if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
        return Err("Fields input must start with '(' and end with ')'.".to_string());
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0;

    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Err("Mismatched parentheses.".to_string());
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                // Split here
                let part = inner[start..i].trim();
                parts.push(part);
                start = i + 1;
            }
            _ => {}
        }
    }

    // Add the last part
    if start < inner.len() {
        let part = inner[start..].trim();
        parts.push(part);
    }

    if depth != 0 {
        return Err("Mismatched parentheses.".to_string());
    }

    Ok(parts)
}