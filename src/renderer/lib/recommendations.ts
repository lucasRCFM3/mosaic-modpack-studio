import type { ProfileTarget, RecommendationFeed, RecommendationScope } from '../../shared/domain';

export const recommendationHistoryStorageKey = 'mosaic:recommendation-history:v1';
export const collectionSizes = [15, 30, 45, 60] as const;

export function parseCollectionSize(raw: string | null): number {
  const parsed = Number(raw);
  return collectionSizes.some((size) => size === parsed) ? parsed : 30;
}

export function loadRecommendationHistory(raw: string | null): RecommendationFeed[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is RecommendationFeed => Boolean(
        item && typeof item === 'object'
        && 'id' in item && typeof item.id === 'string'
        && 'scope' in item && (item.scope === 'currentProfile' || item.scope === 'allVersions')
        && 'packs' in item && Array.isArray(item.packs),
      ))
      .slice(0, 12);
  } catch {
    return [];
  }
}

export function addRecommendationFeed(history: RecommendationFeed[], feed: RecommendationFeed): RecommendationFeed[] {
  return [feed, ...history.filter(({ id }) => id !== feed.id)].slice(0, 12);
}

export function feedMatches(
  feed: RecommendationFeed,
  scope: RecommendationScope,
  target?: ProfileTarget,
): boolean {
  if (feed.scope !== scope) return false;
  if (scope === 'allVersions') return true;
  return Boolean(
    feed.target && target
    && feed.target.minecraftVersion === target.minecraftVersion
    && feed.target.loader === target.loader,
  );
}
