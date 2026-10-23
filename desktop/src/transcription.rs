/// Join overlapping final ASR chunks from a complete recording.
pub fn merge_text(previous: &str, next: &str) -> String {
    if previous.is_empty() {
        return next.to_owned();
    }
    let a: Vec<char> = previous.chars().collect();
    let b: Vec<char> = next.chars().collect();
    let overlap = (4..=a.len().min(b.len()).min(120))
        .rev()
        .find(|&n| a[a.len() - n..] == b[..n])
        .unwrap_or(0);
    if overlap > 0 {
        format!("{previous}{}", b[overlap..].iter().collect::<String>())
    } else {
        format!("{previous}\n{next}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlap_and_unicode() {
        assert_eq!(
            merge_text("项目 Vao 开始测试", "开始测试然后核对日志"),
            "项目 Vao 开始测试然后核对日志"
        );
        assert_eq!(merge_text("第一段。", "第二段。"), "第一段。\n第二段。");
    }
}
