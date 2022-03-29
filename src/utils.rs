use tower_lsp::lsp_types::{Position, Range};

pub fn range_to_lsp_range(range: &std::ops::Range<usize>, s: &str) -> Range {
    let to_start = &s[..range.start];
    let chars_start = to_start.chars().rev().take_while(|c| *c != '\n').count() as u32;
    let lines_start = to_start.lines().count() as u32;

    let to_end = &s[..range.end];
    let chars_end = to_end.chars().rev().take_while(|c| *c != '\n').count() as u32;
    let lines_end = to_end.lines().count() as u32;

    Range {
        start: Position { line: lines_start, character: chars_start },
        end: Position { line: lines_end, character: chars_end },
    }
}
