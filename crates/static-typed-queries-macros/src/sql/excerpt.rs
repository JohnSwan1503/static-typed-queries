use std::ops::Range;

const WIDTH: usize = 100;

// The line of the template holding `at`, with carets under it. Errors can only span the whole
// literal, so the message has to show where in it the problem is.
pub(crate) fn point(source: &str, at: Range<usize>) -> String {
    let chars: Vec<char> = source.chars().collect();
    let start = at.start.min(chars.len());
    let line_start = chars[..start]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |i| i + 1);
    let line_end = chars[start..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |i| start + i);
    let indent = chars[line_start..line_end]
        .iter()
        .take_while(|c| c.is_whitespace())
        .count();
    let first = (line_start + indent).min(start);
    let line: Vec<char> = chars[first..line_end]
        .iter()
        .map(|&c| if c == '\t' { ' ' } else { c })
        .collect();
    let width = at.end.min(line_end).saturating_sub(start).max(1);

    let caret = start - first;
    let left = caret
        .saturating_sub(WIDTH / 2)
        .min(line.len().saturating_sub(WIDTH));
    let right = (left + WIDTH).min(line.len());
    let mut shown: String = line[left..right].iter().collect();
    let mut offset = caret - left;
    if left > 0 {
        shown.insert(0, '…');
        offset += 1;
    }
    if right < line.len() {
        shown.push('…');
    }
    let label = if chars.contains(&'\n') {
        let number = chars[..start].iter().filter(|&&c| c == '\n').count() + 1;
        format!("line {number}: ")
    } else {
        String::new()
    };
    let pad = " ".repeat(label.chars().count() + offset);
    format!(
        "\n  {label}{}\n  {pad}{}",
        shown.trim_end(),
        "^".repeat(width.min(WIDTH))
    )
}
