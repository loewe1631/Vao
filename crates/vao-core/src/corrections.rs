//! Explicit, local correction suggestions. Matching never authorizes replacement.
use aho_corasick::AhoCorasick;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use unicode_segmentation::UnicodeSegmentation;

pub const FORMAT: &str = "vao-personal-corrections";
pub const MAX_FILE: usize = 16 * 1024 * 1024;
pub const MAX_GROUPS: usize = 2_000;
pub const MAX_ALIASES: usize = 10_000;
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Alias {
    pub wrong: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub correct: String,
    pub enabled: bool,
    pub aliases: Vec<Alias>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub revision: u64,
    pub enabled: bool,
    pub groups: Vec<Group>,
}
impl Default for Book {
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: true,
            groups: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exchange {
    pub format: String,
    pub schema_version: u32,
    pub exported_at: String,
    pub groups: Vec<Group>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    pub wrong: String,
    pub correct: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub key: String,
    pub wrong: String,
    pub existing: String,
    pub incoming: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeReport {
    pub book: Book,
    pub added_groups: usize,
    pub added_aliases: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub conflicts: Vec<Conflict>,
}
pub fn term(value: &str) -> Result<String, String> {
    let value: String = value.trim().nfc().collect();
    if value.is_empty()
        || value.chars().count() > 160
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err("词语不能为空、超过160个字符或包含控制字符 / Invalid correction phrase".into());
    }
    Ok(value)
}
pub fn key(value: &str) -> String {
    let mut result = String::new();
    let mut space = false;
    for c in value.nfc() {
        if c.is_whitespace() && !matches!(c, '\n' | '\r') {
            if !space {
                result.push(' ');
            }
            space = true;
        } else {
            result.push(c.to_ascii_lowercase());
            space = false;
        }
    }
    result
}
fn validate_groups(groups: &[Group]) -> Result<(), String> {
    if groups.len() > MAX_GROUPS
        || groups.iter().map(|g| g.aliases.len()).sum::<usize>() > MAX_ALIASES
    {
        return Err("个人纠错超过2000组或10000个错误写法 / Correction library is too large".into());
    }
    for group in groups {
        term(&group.correct)?;
        if group.aliases.is_empty() {
            return Err("纠错组缺少错误写法 / Empty correction group".into());
        }
        for alias in &group.aliases {
            term(&alias.wrong)?;
        }
    }
    Ok(())
}
pub fn parse_exchange(text: &str) -> Result<Exchange, String> {
    if text.len() > MAX_FILE {
        return Err("纠错文件超过16 MiB / Correction file exceeds 16 MiB".into());
    }
    let exchange: Exchange = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "纠错文件格式无效，请使用个人纠错 JSON / Invalid personal corrections JSON")?;
    if exchange.format != FORMAT || exchange.schema_version != 1 {
        return Err("不支持此纠错文件版本 / Unsupported correction file version".into());
    }
    if time::OffsetDateTime::parse(
        &exchange.exported_at,
        &time::format_description::well_known::Rfc3339,
    )
    .is_err()
    {
        return Err("导出日期格式无效 / Invalid export timestamp".into());
    }
    validate_groups(&exchange.groups)?;
    Ok(exchange)
}
impl Book {
    /// Normalize and coalesce identical targets/aliases without choosing conflicts.
    pub fn canonicalize(&mut self) -> Result<(), String> {
        validate_groups(&self.groups)?;
        let mut groups: Vec<Group> = vec![];
        for input in &self.groups {
            let correct = term(&input.correct)?;
            let i = if let Some(i) = groups.iter().position(|g| g.correct == correct) {
                groups[i].enabled &= input.enabled;
                i
            } else {
                groups.push(Group {
                    correct: correct.clone(),
                    enabled: input.enabled,
                    aliases: vec![],
                });
                groups.len() - 1
            };
            for alias in &input.aliases {
                let wrong = term(&alias.wrong)?;
                if wrong == correct {
                    continue;
                }
                if let Some(a) = groups[i]
                    .aliases
                    .iter_mut()
                    .find(|a| key(&a.wrong) == key(&wrong))
                {
                    a.enabled &= alias.enabled;
                } else {
                    groups[i].aliases.push(Alias {
                        wrong,
                        enabled: alias.enabled,
                    });
                }
            }
        }
        groups.retain(|g| !g.aliases.is_empty());
        let normalized = Book {
            revision: self.revision,
            enabled: self.enabled,
            groups,
        };
        normalized.validate()?;
        *self = normalized;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        validate_groups(&self.groups)?;
        let mut targets = HashSet::new();
        let mut aliases = HashSet::new();
        for group in &self.groups {
            if !targets.insert(term(&group.correct)?) {
                return Err("重复正确词条 / Duplicate correction group".into());
            }
            for alias in &group.aliases {
                if !aliases.insert(key(&term(&alias.wrong)?)) {
                    return Err("错误写法对应多个记录 / Conflicting correction alias".into());
                }
            }
        }
        Ok(())
    }
    pub fn remember(&mut self, pairs: &[Pair]) -> Result<(), String> {
        let mut next = self.clone();
        for pair in pairs {
            let correct = term(&pair.correct)?;
            let wrong = term(&pair.wrong)?;
            if correct == wrong {
                continue;
            }
            let alias_key = key(&wrong);
            if let Some(group) = next
                .groups
                .iter()
                .find(|g| g.aliases.iter().any(|a| key(&a.wrong) == alias_key))
            {
                if group.correct != correct {
                    return Err(format!("「{wrong}」已属于「{}」，请在个人纠错中调整 / Alias already belongs to another group",group.correct));
                }
                continue;
            }
            if let Some(group) = next.groups.iter_mut().find(|g| g.correct == correct) {
                group.aliases.push(Alias {
                    wrong,
                    enabled: yes(),
                });
            } else {
                next.groups.push(Group {
                    correct,
                    enabled: yes(),
                    aliases: vec![Alias {
                        wrong,
                        enabled: yes(),
                    }],
                });
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Import choices are keyed by normalized alias; omitted choices retain local data.
    /// Conflicts internal to a new file require an explicit chosen target.
    pub fn merge(
        &self,
        incoming: &[Group],
        choices: &HashMap<String, String>,
    ) -> Result<MergeReport, String> {
        validate_groups(incoming)?;
        let mut report = MergeReport {
            book: self.clone(),
            added_groups: 0,
            added_aliases: 0,
            duplicates: 0,
            skipped: 0,
            conflicts: vec![],
        };
        let mut staged: Vec<Group> = vec![];
        for group in incoming {
            let correct = term(&group.correct)?;
            let gi = match staged.iter().position(|g| g.correct == correct) {
                Some(i) => {
                    staged[i].enabled &= group.enabled;
                    i
                }
                None => {
                    staged.push(Group {
                        correct,
                        enabled: group.enabled,
                        aliases: vec![],
                    });
                    staged.len() - 1
                }
            };
            for alias in &group.aliases {
                let wrong = term(&alias.wrong)?;
                if wrong == staged[gi].correct {
                    report.skipped += 1;
                    continue;
                }
                if let Some(old) = staged[gi]
                    .aliases
                    .iter_mut()
                    .find(|a| key(&a.wrong) == key(&wrong))
                {
                    old.enabled &= alias.enabled;
                    report.duplicates += 1;
                } else {
                    staged[gi].aliases.push(Alias {
                        wrong,
                        enabled: alias.enabled,
                    });
                }
            }
        }
        let mut incoming_targets: HashMap<String, Vec<String>> = HashMap::new();
        for g in &staged {
            for a in &g.aliases {
                incoming_targets
                    .entry(key(&a.wrong))
                    .or_default()
                    .push(g.correct.clone());
            }
        }
        for group in staged {
            for alias in group.aliases {
                let k = key(&alias.wrong);
                let local = report
                    .book
                    .groups
                    .iter()
                    .find(|g| g.aliases.iter().any(|a| key(&a.wrong) == k))
                    .map(|g| g.correct.clone());
                let original = self
                    .groups
                    .iter()
                    .find(|g| g.aliases.iter().any(|a| key(&a.wrong) == k))
                    .map(|g| g.correct.clone());
                let internal = incoming_targets[&k].len() > 1;
                let conflict = local.as_ref().is_some_and(|c| *c != group.correct) || internal;
                if conflict {
                    let existing = original.clone().or(local.clone()).unwrap_or_default();
                    if !report
                        .conflicts
                        .iter()
                        .any(|c| c.key == k && c.incoming == group.correct)
                    {
                        report.conflicts.push(Conflict {
                            key: k.clone(),
                            wrong: alias.wrong.clone(),
                            existing,
                            incoming: group.correct.clone(),
                        });
                    }
                    let selected = choices.get(&k).cloned().or(original);
                    if selected.as_deref() != Some(group.correct.as_str()) {
                        report.skipped += 1;
                        continue;
                    }
                }
                if local.as_deref() == Some(group.correct.as_str()) {
                    report.duplicates += 1;
                    continue;
                }
                if local.is_some() {
                    for g in &mut report.book.groups {
                        g.aliases.retain(|a| key(&a.wrong) != k);
                    }
                    report.book.groups.retain(|g| !g.aliases.is_empty());
                }
                if let Some(g) = report
                    .book
                    .groups
                    .iter_mut()
                    .find(|g| g.correct == group.correct)
                {
                    g.aliases.push(alias);
                } else {
                    report.book.groups.push(Group {
                        correct: group.correct.clone(),
                        enabled: group.enabled,
                        aliases: vec![alias],
                    });
                    report.added_groups += 1;
                }
                report.added_aliases += 1;
            }
        }
        report.book.validate()?;
        Ok(report)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: usize,
    pub start: usize,
    pub end: usize,
    pub found: String,
    pub correct: String,
}
pub struct Matcher {
    machine: Option<AhoCorasick>,
    targets: Vec<String>,
}
impl Matcher {
    pub fn new(book: &Book) -> Result<Self, String> {
        book.validate()?;
        let mut patterns = vec![];
        let mut targets = vec![];
        if book.enabled {
            for group in &book.groups {
                if group.enabled {
                    for alias in &group.aliases {
                        if alias.enabled {
                            patterns.push(key(&alias.wrong));
                            targets.push(group.correct.clone());
                        }
                    }
                }
            }
        }
        let machine = if patterns.is_empty() {
            None
        } else {
            Some(
                AhoCorasick::new(patterns)
                    .map_err(|_| "无法构建纠错索引 / Could not build correction index")?,
            )
        };
        Ok(Self { machine, targets })
    }
    pub fn suggest(&self, text: &str, original: &str) -> Result<Vec<Suggestion>, String> {
        let Some(machine) = &self.machine else {
            return Ok(vec![]);
        };
        let keep = |t: &str| {
            let t = t.to_ascii_lowercase();
            ["原样保留", "逐字保留", "verbatim", "keep exactly"]
                .iter()
                .any(|s| t.contains(s))
        };
        if keep(text) || keep(original) {
            return Ok(vec![]);
        }
        // Map NFC graphemes and collapsed horizontal whitespace back to source bytes.
        let mut projected = String::new();
        let mut ranges: Vec<(usize, usize)> = vec![];
        let mut previous_space = false;
        for (start, grapheme) in text.grapheme_indices(true) {
            let end = start + grapheme.len();
            for ch in grapheme.nfc() {
                let space = ch.is_whitespace() && !matches!(ch, '\n' | '\r');
                if space && previous_space {
                    if let Some(last) = ranges.last_mut() {
                        last.1 = end;
                    }
                    continue;
                }
                let c = if space { ' ' } else { ch.to_ascii_lowercase() };
                projected.push(c);
                ranges.extend(std::iter::repeat_n((start, end), c.len_utf8()));
                previous_space = space;
            }
        }
        let protected = protected_ranges(text);
        // Chinese naturally adjoins English without spaces (例如“和Tablets比较”).
        let word = |c: char| {
            (c.is_alphanumeric() && !matches!(c,'\u{3400}'..='\u{9fff}'|'\u{20000}'..='\u{3134f}'))
                || c == '_'
                || is_combining_mark(c)
        };
        let mut result = vec![];
        for hit in machine.find_overlapping_iter(&projected) {
            let (start, _) = ranges[hit.start()];
            let (_, end) = ranges[hit.end() - 1];
            if (hit.start() > 0 && ranges[hit.start() - 1].0 == start)
                || (hit.end() < ranges.len() && ranges[hit.end()].1 == end)
            {
                continue;
            }
            if start >= end || protected.iter().any(|&(s, e)| start < e && end > s) {
                continue;
            }
            let found = &text[start..end];
            let correct = &self.targets[hit.pattern().as_usize()];
            if found == correct {
                continue;
            }
            let first = found.chars().next().unwrap();
            let last = found.chars().next_back().unwrap();
            if first.is_ascii_alphanumeric() && text[..start].chars().next_back().is_some_and(word)
            {
                continue;
            }
            if last.is_ascii_alphanumeric() && text[end..].chars().next().is_some_and(word) {
                continue;
            }
            if result
                .iter()
                .any(|s: &Suggestion| s.start == start && s.end == end && s.correct == *correct)
            {
                continue;
            }
            if result.len() >= 512 {
                return Err("本次纠错命中过多，请取消并分段输入 / Too many suggestions; cancel and split this input".into());
            }
            result.push(Suggestion {
                id: 0,
                start,
                end,
                found: found.into(),
                correct: correct.clone(),
            });
        }
        result.sort_by_key(|s| (s.start, std::cmp::Reverse(s.end - s.start)));
        for (id, item) in result.iter_mut().enumerate() {
            item.id = id;
        }
        Ok(result)
    }
}
fn protected_ranges(text: &str) -> Vec<(usize, usize)> {
    static PROTECTED: OnceLock<Regex> = OnceLock::new();
    let re=PROTECTED.get_or_init(||Regex::new(r"(?is)```.*?(?:```|$)|`[^`\r\n]*(?:`|$)|(?:https?://|www\.)[^\s<>]+|[\w.+-]+@[\w.-]+\.[a-z]{2,}|(?:[A-Z]:\\|/|~/)[^\s<>，。；]+(?:[/\\][^\s<>，。；]*)?").unwrap());
    re.find_iter(text).map(|m| (m.start(), m.end())).collect()
}
pub fn apply(text: &str, suggestions: &[Suggestion], selected: &[usize]) -> Result<String, String> {
    let ids: HashSet<usize> = selected.iter().copied().collect();
    if ids.len() != selected.len()
        || ids
            .iter()
            .any(|id| !suggestions.iter().any(|s| s.id == *id))
    {
        return Err("纠错选择已过期 / Invalid correction selection".into());
    }
    let mut edits = suggestions
        .iter()
        .filter(|s| ids.contains(&s.id))
        .collect::<Vec<_>>();
    edits.sort_by_key(|s| s.start);
    let mut result = String::new();
    let mut cursor = 0;
    for edit in edits {
        if edit.start < cursor
            || edit.end <= edit.start
            || text.get(edit.start..edit.end) != Some(edit.found.as_str())
        {
            return Err("纠错位置重叠或内容已变化 / Overlapping or stale correction".into());
        }
        result.push_str(&text[cursor..edit.start]);
        result.push_str(&edit.correct);
        cursor = edit.end;
    }
    result.push_str(&text[cursor..]);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn book() -> Book {
        let mut b = Book::default();
        b.remember(&[
            Pair {
                wrong: "Tablets".into(),
                correct: "typeless".into(),
            },
            Pair {
                wrong: "华池追问".into(),
                correct: "划词追问".into(),
            },
            Pair {
                wrong: "cloud file".into(),
                correct: "cloudflare".into(),
            },
        ])
        .unwrap();
        b
    }
    #[test]
    fn grouping_is_explicit_and_conflicts_are_atomic() {
        let mut b = book();
        b.remember(&[Pair {
            wrong: "华慈追问".into(),
            correct: "划词追问".into(),
        }])
        .unwrap();
        assert_eq!(b.groups.len(), 3);
        let before = serde_json::to_string(&b).unwrap();
        assert!(b
            .remember(&[Pair {
                wrong: "tablets".into(),
                correct: "Typeless".into()
            }])
            .is_err());
        assert_eq!(before, serde_json::to_string(&b).unwrap());
    }
    #[test]
    fn exact_spans_and_word_boundaries() {
        let m = Matcher::new(&book()).unwrap();
        let text = "🙂 Cloud  File，华池追问功能；Tablets。cloud files tabletsx";
        let s = m.suggest(text, "").unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(
            apply(text, &s, &[0, 1, 2]).unwrap(),
            "🙂 cloudflare，划词追问功能；typeless。cloud files tabletsx"
        );
    }
    #[test]
    fn user_keeps_text_and_selects_one_occurrence() {
        let m = Matcher::new(&book()).unwrap();
        let t = "Tablets，然后Tablets。";
        let s = m.suggest(t, "").unwrap();
        assert_eq!(apply(t, &s, &[]).unwrap(), t);
        assert_eq!(apply(t, &s, &[1]).unwrap(), "Tablets，然后typeless。");
        assert!(apply(t, &s, &[55]).is_err());
    }
    #[test]
    fn protected_and_current_instructions() {
        let m = Matcher::new(&book()).unwrap();
        assert!(m
            .suggest("`Tablets` https://site.test/Tablets /tmp/Tablets", " ")
            .unwrap()
            .is_empty());
        assert!(m.suggest("Tablets", "这次原样保留").unwrap().is_empty());
        assert!(m.suggest("typeless", "").unwrap().is_empty());
    }
    #[test]
    fn nfc_and_no_chain() {
        let mut b = Book::default();
        b.remember(&[
            Pair {
                wrong: "cafe\u{301}".into(),
                correct: "café!".into(),
            },
            Pair {
                wrong: "café!".into(),
                correct: "second".into(),
            },
        ])
        .unwrap();
        let m = Matcher::new(&b).unwrap();
        let t = "cafe\u{301}，完";
        let s = m.suggest(t, "").unwrap();
        assert_eq!(apply(t, &s, &[0]).unwrap(), "café!，完");
    }
    #[test]
    fn case_only_and_disabled() {
        let mut b = Book::default();
        b.remember(&[Pair {
            wrong: "github".into(),
            correct: "GitHub".into(),
        }])
        .unwrap();
        assert!(Matcher::new(&b)
            .unwrap()
            .suggest("GitHub", "")
            .unwrap()
            .is_empty());
        assert_eq!(
            Matcher::new(&b)
                .unwrap()
                .suggest("github", "")
                .unwrap()
                .len(),
            1
        );
        b.enabled = false;
        assert!(Matcher::new(&b)
            .unwrap()
            .suggest("github", "")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn importing_merges_preserves_disable_and_requires_conflict_choice() {
        let mut b = book();
        b.groups[0].enabled = false;
        let imported = b.groups.clone();
        let r = b.merge(&imported, &HashMap::new()).unwrap();
        assert_eq!(r.added_aliases, 0);
        assert!(!r.book.groups[0].enabled);
        let g = Group {
            correct: "another".into(),
            enabled: true,
            aliases: vec![Alias {
                wrong: "TABLETS".into(),
                enabled: true,
            }],
        };
        let r = b.merge(std::slice::from_ref(&g), &HashMap::new()).unwrap();
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.book.groups[0].correct, "typeless");
        let r = b
            .merge(&[g], &HashMap::from([("tablets".into(), "another".into())]))
            .unwrap();
        assert!(r.book.groups.iter().any(|g| g.correct == "another"));
    }
    #[test]
    fn overlap_cannot_be_applied_together() {
        let mut b = Book::default();
        b.remember(&[
            Pair {
                wrong: "华池".into(),
                correct: "划词".into(),
            },
            Pair {
                wrong: "华池追问".into(),
                correct: "划词追问".into(),
            },
        ])
        .unwrap();
        let t = "华池追问";
        let s = Matcher::new(&b).unwrap().suggest(t, "").unwrap();
        assert_eq!(s[0].found, t);
        assert!(apply(t, &s, &[0, 1]).is_err());
        assert_eq!(apply(t, &s, &[0]).unwrap(), "划词追问");
    }
    #[test]
    fn waiting_state_is_busy_and_cancellable() {
        let mut s = crate::Session::default();
        let id = s.begin().unwrap();
        s.advance(id, crate::Phase::Listening).unwrap();
        s.advance(id, crate::Phase::Processing).unwrap();
        s.advance(id, crate::Phase::AwaitingCorrection).unwrap();
        assert!(s.begin().is_err());
        s.cancel();
        assert!(s.claim_delivery(id).is_err());
    }
    #[test]
    fn canonical_group_edits_deduplicate_without_choosing_a_conflict() {
        let mut b = Book {
            groups: vec![
                Group {
                    correct: " GitHub ".into(),
                    enabled: true,
                    aliases: vec![Alias {
                        wrong: "GIT HUB".into(),
                        enabled: true,
                    }],
                },
                Group {
                    correct: "GitHub".into(),
                    enabled: false,
                    aliases: vec![Alias {
                        wrong: "git hub".into(),
                        enabled: false,
                    }],
                },
            ],
            ..Book::default()
        };
        b.canonicalize().unwrap();
        assert_eq!(b.groups.len(), 1);
        assert_eq!(b.groups[0].aliases.len(), 1);
        assert!(!b.groups[0].enabled);
        assert!(!b.groups[0].aliases[0].enabled);
        let previous = b.clone();
        b.groups.push(Group {
            correct: "Other".into(),
            enabled: true,
            aliases: vec![Alias {
                wrong: "git hub".into(),
                enabled: true,
            }],
        });
        assert!(b.canonicalize().is_err());
        assert_eq!(previous.groups[0].correct, "GitHub");
    }
    #[test]
    fn exchange_validation_is_strict_and_match_overflow_fails_closed() {
        let value = serde_json::json!({"format":FORMAT,"schema_version":1,"exported_at":"2026-10-24T00:00:00Z","groups":book().groups});
        assert!(parse_exchange(&value.to_string()).is_ok());
        for change in [
            serde_json::json!({"schema_version":2}),
            serde_json::json!({"exported_at":"invalid"}),
            serde_json::json!({"history":"forbidden"}),
        ] {
            let mut changed = value.clone();
            for (k, v) in change.as_object().unwrap() {
                changed[k] = v.clone();
            }
            assert!(parse_exchange(&changed.to_string()).is_err());
        }
        assert!(term("bad\nphrase").is_err());
        assert!(Matcher::new(&book())
            .unwrap()
            .suggest(&"Tablets ".repeat(513), "")
            .is_err());
    }
    #[test]
    fn partial_grapheme_match_does_not_consume_neighbor_marks() {
        let mut b = Book::default();
        b.remember(&[Pair {
            wrong: "é".into(),
            correct: "accent".into(),
        }])
        .unwrap();
        assert!(Matcher::new(&b)
            .unwrap()
            .suggest("e\u{301}\u{323}", "")
            .unwrap()
            .is_empty());
    }
    #[test]
    #[ignore = "explicit release performance measurement"]
    fn maximum_library_performance() {
        let b = Book {
            revision: 0,
            enabled: true,
            groups: (0..2000)
                .map(|i| Group {
                    correct: format!("Canonical{i}"),
                    enabled: true,
                    aliases: (0..5)
                        .map(|j| Alias {
                            wrong: format!("variant{i}x{j}"),
                            enabled: true,
                        })
                        .collect(),
                })
                .collect(),
        };
        let build = std::time::Instant::now();
        let m = Matcher::new(&b).unwrap();
        let build_ms = build.elapsed().as_secs_f64() * 1000.;
        let mut text: String = "这是一段固定的性能样本文字。"
            .chars()
            .cycle()
            .take(19_980)
            .collect();
        text.push_str(" variant1999x4 ");
        while text.chars().count() < 20_000 {
            text.push('。');
        }
        assert_eq!(m.suggest(&text, "").unwrap().len(), 1);
        for _ in 0..10 {
            m.suggest(&text, "").unwrap();
        }
        let mut samples: Vec<f64> = (0..100)
            .map(|_| {
                let t = std::time::Instant::now();
                m.suggest(&text, "").unwrap();
                t.elapsed().as_secs_f64() * 1000.
            })
            .collect();
        samples.sort_by(f64::total_cmp);
        println!("groups=2000 aliases=10000 characters={} index_build_ms={build_ms:.3} match_p50_ms={:.3} match_p95_ms={:.3}",text.chars().count(),samples[50],samples[95]);
        assert!(samples[95] < 50.);
    }
}
