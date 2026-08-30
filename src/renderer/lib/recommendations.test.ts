import { describe, expect, it } from 'vitest';
import type { RecommendationFeed } from '../../shared/domain';
import { addRecommendationFeed, feedMatches, loadRecommendationHistory, parseCollectionSize } from './recommendations';

const feed = (id: string, scope: RecommendationFeed['scope'] = 'allVersions'): RecommendationFeed => ({
  id,
  generatedAt: '2026-01-01T00:00:00Z',
  scope,
  target: scope === 'currentProfile' ? { minecraftVersion: '1.20.1', loader: 'forge', releaseChannels: ['release'] } : undefined,
  packs: [],
  warnings: [],
});

describe('recommendation history', () => {
  it('recovers safely from malformed local data', () => {
    expect(loadRecommendationHistory('{broken')).toEqual([]);
    expect(loadRecommendationHistory(JSON.stringify([feed('one'), { nope: true }]))).toHaveLength(1);
  });

  it('keeps newest feeds and caps history', () => {
    const history = Array.from({ length: 12 }, (_, index) => feed(String(index)));
    const next = addRecommendationFeed(history, feed('new'));
    expect(next).toHaveLength(12);
    expect(next[0].id).toBe('new');
  });

  it('matches current-profile snapshots by version and loader', () => {
    expect(feedMatches(feed('one', 'currentProfile'), 'currentProfile', { minecraftVersion: '1.20.1', loader: 'forge', releaseChannels: ['beta'] })).toBe(true);
    expect(feedMatches(feed('one', 'currentProfile'), 'currentProfile', { minecraftVersion: '1.20.1', loader: 'fabric', releaseChannels: ['release'] })).toBe(false);
  });

  it('uses 30 mods by default and accepts only supported collection sizes', () => {
    expect(parseCollectionSize(null)).toBe(30);
    expect(parseCollectionSize('45')).toBe(45);
    expect(parseCollectionSize('31')).toBe(30);
  });
});
