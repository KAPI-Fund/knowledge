//! Chat context helpers ported from upstream_llm_wiki:
//! src/lib/context-budget.ts and src/lib/greeting-detector.ts.

const DEFAULT_MAX_CTX: usize = 204_800;
const PER_PAGE_FLOOR: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBudget {
    pub max_ctx: usize,
    pub response_reserve: usize,
    pub index_budget: usize,
    pub page_budget: usize,
    pub max_page_size: usize,
}

pub fn compute_context_budget(max_context_size: Option<usize>) -> ContextBudget {
    let max_ctx = match max_context_size {
        Some(value) if value > 0 => value,
        _ => DEFAULT_MAX_CTX,
    };

    let response_reserve = max_ctx * 15 / 100;
    let index_budget = max_ctx * 5 / 100;
    let page_budget = max_ctx / 2;
    let max_page_size = (page_budget * 3 / 10).max(PER_PAGE_FLOOR).min(page_budget);

    ContextBudget {
        max_ctx,
        response_reserve,
        index_budget,
        page_budget,
        max_page_size,
    }
}

const MAX_GREETING_LEN: usize = 20;

pub fn is_greeting(text: &str) -> bool {
    let stripped = text.trim().trim_end_matches(is_trailing_punct).trim();
    if stripped.is_empty() || stripped.chars().count() > MAX_GREETING_LEN {
        return false;
    }
    // CJK chars are unaffected by lowercasing, so one normalized form serves
    // both the ASCII and CJK word lists (mirrors upstream toLowerCase()).
    let normalized = stripped.to_lowercase();

    matches_english(&normalized) || matches_cjk(&normalized) || matches_european(&normalized)
}

fn is_trailing_punct(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '!' | '！' | '。' | '.' | '?' | '？' | '~' | ',' | '，' | '、' | ';' | '；' | ':' | '：'
        )
}

fn matches_english(text: &str) -> bool {
    const BASES: &[&str] = &[
        "hi", "hello", "hey", "yo", "sup", "howdy", "hiya", "heya", "hullo",
    ];
    const SUFFIXES: &[&str] = &[" there", " y'all", " you", " folks", " everyone"];

    if BASES.contains(&text) || text == "greetings" {
        return true;
    }
    for base in BASES {
        if let Some(rest) = text.strip_prefix(base) {
            if SUFFIXES.contains(&rest) {
                return true;
            }
        }
    }
    if let Some(rest) = text.strip_prefix("good ") {
        return ["morning", "afternoon", "evening", "day", "night"].contains(&rest);
    }
    ["what's up", "whats up", "wassup", "whaddup"].contains(&text)
}

fn matches_cjk(text: &str) -> bool {
    const PARTICLES: &[char] = &['啊', '呀', '吖', '呢', '么', '呗', '哦', '哈'];
    const STANDALONE: &[&str] = &[
        "你好", "您好", "大家好", "嗨", "哈喽", "哈啰", "哈囉", "哈罗", "喂",
    ];
    const TIME_OF_DAY: &[&str] = &[
        "早", "早啊", "早安", "早上好", "中午好", "下午好", "晚上好", "晚安",
    ];
    const ARE_YOU_THERE: &[&str] = &[
        "在吗", "在嗎", "在不在", "有人吗", "有人嗎", "有人在吗", "有人在嗎",
    ];
    const JAPANESE: &[&str] = &[
        "こんにちは",
        "こんばんは",
        "おはよう",
        "おはようございます",
        "やあ",
        "どうも",
        "はじめまして",
    ];
    const KOREAN: &[&str] = &["안녕", "안녕하세요", "안녕하십니까"];

    if ARE_YOU_THERE.contains(&text) || JAPANESE.contains(&text) || KOREAN.contains(&text) {
        return true;
    }

    let base = text.strip_suffix(PARTICLES).unwrap_or(text);
    STANDALONE.contains(&base) || TIME_OF_DAY.contains(&base)
}

fn matches_european(text: &str) -> bool {
    [
        "hola", "bonjour", "salut", "coucou", "hallo", "servus", "hej", "hejsan", "ciao",
        "saluton", "ola", "olá", "privet", "привет",
    ]
    .contains(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_falls_back_to_default_for_missing_or_zero_context() {
        let budget = compute_context_budget(None);
        assert_eq!(budget.max_ctx, 204_800);
        assert_eq!(budget.response_reserve, 30_720);
        assert_eq!(budget.index_budget, 10_240);
        assert_eq!(budget.page_budget, 102_400);
        assert_eq!(budget.max_page_size, 30_720);
        assert_eq!(compute_context_budget(Some(0)).max_ctx, 204_800);
    }

    #[test]
    fn budget_per_page_cap_never_exceeds_page_budget_on_tiny_configs() {
        let budget = compute_context_budget(Some(8_000));
        assert_eq!(budget.page_budget, 4_000);
        assert_eq!(budget.max_page_size, 4_000);
    }

    #[test]
    fn budget_per_page_cap_scales_with_large_configs() {
        let budget = compute_context_budget(Some(1_000_000));
        assert_eq!(budget.page_budget, 500_000);
        assert_eq!(budget.max_page_size, 150_000);
    }

    #[test]
    fn detects_plain_greetings_across_languages() {
        for text in [
            "hi",
            "Hello!!",
            "hey there",
            "good morning",
            "what's up?",
            "你好啊",
            "早上好",
            "在吗",
            "こんにちは",
            "안녕하세요",
            "bonjour",
            "привет",
        ] {
            assert!(is_greeting(text), "expected greeting: {text}");
        }
    }

    #[test]
    fn rejects_questions_long_messages_and_empty_input() {
        for text in [
            "hello, how do I train a transformer?",
            "hi I have a question about attention",
            "",
            "   ",
            "the index page is broken",
        ] {
            assert!(!is_greeting(text), "expected non-greeting: {text}");
        }
    }
}
