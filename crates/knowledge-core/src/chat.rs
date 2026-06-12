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
}
