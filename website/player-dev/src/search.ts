// ============================================================================
// Search - Fuzzy search and HTML utilities
// ============================================================================

/** Fuzzy match result */
export interface FuzzyMatchResult {
  match: boolean;
  score: number;
  indices: number[];
}

// ============================================================================
// Fuzzy Match
// ============================================================================

export function fuzzyMatch(query: string, text: string): FuzzyMatchResult {
  if (!query) return { match: true, score: 0, indices: [] };
  query = query.toLowerCase();
  text = text.toLowerCase();

  let queryIdx = 0;
  let score = 0;
  const indices: number[] = [];
  let lastMatchIdx = -1;

  for (let i = 0; i < text.length && queryIdx < query.length; i++) {
    if (text[i] === query[queryIdx]) {
      indices.push(i);
      if (lastMatchIdx === i - 1) score += 2;
      else score += 1;
      if (i === 0 || ' _-'.includes(text[i - 1] ?? '')) score += 3;
      lastMatchIdx = i;
      queryIdx++;
    }
  }

  return {
    match: queryIdx === query.length,
    score: queryIdx === query.length ? score : 0,
    indices,
  };
}

// ============================================================================
// HTML Utilities
// ============================================================================

export function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

export function highlightMatches(text: string, indices: number[]): string {
  if (!indices || !indices.length) return escapeHtml(text);
  let result = '';
  let lastIdx = 0;
  for (const idx of indices) {
    result += escapeHtml(text.slice(lastIdx, idx));
    result += `<span class="search-highlight">${escapeHtml(text[idx] ?? '')}</span>`;
    lastIdx = idx + 1;
  }
  result += escapeHtml(text.slice(lastIdx));
  return result;
}
