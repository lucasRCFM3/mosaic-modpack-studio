use crate::{domain::*, providers::ProviderRegistry};
pub async fn find_alternate_download(
    providers: &ProviderRegistry,
    source: &ProjectSummary,
    target: &ProfileTarget,
) -> Option<(ProjectSummary, ProjectVersion)> {
    find_equivalent_project(
        providers,
        source,
        target,
        ProviderRegistry::alternate_id(source.provider),
        true,
    )
    .await
}

pub async fn find_equivalent_project(
    providers: &ProviderRegistry,
    source: &ProjectSummary,
    target: &ProfileTarget,
    target_provider: ProviderId,
    require_download: bool,
) -> Option<(ProjectSummary, ProjectVersion)> {
    let provider = providers.get(target_provider);
    if !provider.is_enabled() {
        return None;
    }
    for query in [&source.slug, &source.name] {
        if query.trim().is_empty() {
            continue;
        }
        let filters = SearchFilters {
            query: query.clone(),
            minecraft_version: target.minecraft_version.clone(),
            loader: target.loader,
            release_channels: target.release_channels.clone(),
            providers: vec![provider.id()],
            side: SearchSide::Any,
            sort: SearchSort::Relevance,
            limit: Some(10),
        };
        let Ok(result) = provider.search(&filters).await else {
            continue;
        };
        let mut candidates = result.projects;
        candidates.retain(|candidate| projects_equivalent(source, candidate));
        candidates.sort_by(|left, right| {
            identity_score(source, right)
                .cmp(&identity_score(source, left))
                .then_with(|| right.downloads.cmp(&left.downloads))
        });
        for candidate in candidates {
            if let Ok(Some(version)) = provider
                .get_compatible_version(&candidate.project_id, target, None)
                .await
            {
                if !require_download
                    || primary_file(&version).is_some_and(|file| file.url.is_some())
                {
                    return Some((candidate, version));
                }
            }
        }
    }
    None
}

pub fn projects_equivalent(source: &ProjectSummary, candidate: &ProjectSummary) -> bool {
    identity_score(source, candidate) > 0
}

pub fn provider_label(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Modrinth => "Modrinth",
        ProviderId::Curseforge => "CurseForge",
    }
}

fn primary_file(version: &ProjectVersion) -> Option<&DownloadFile> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
}

fn normalized_identity(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalized_title(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| {
            !matches!(
                part.to_ascii_lowercase().as_str(),
                "fabric" | "forge" | "neoforge" | "quilt" | "mod"
            )
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

fn identity_score(source: &ProjectSummary, candidate: &ProjectSummary) -> u8 {
    let slug_matches = !source.slug.is_empty()
        && normalized_identity(&source.slug) == normalized_identity(&candidate.slug);
    let name_matches = normalized_identity(&source.name) == normalized_identity(&candidate.name);
    let title = normalized_title(&source.name);
    let title_matches = title.len() >= 5 && title == normalized_title(&candidate.name);
    let mut score = if slug_matches {
        6
    } else if name_matches {
        5
    } else if title_matches {
        3
    } else {
        return 0;
    };
    if !source.author.is_empty()
        && normalized_identity(&source.author) == normalized_identity(&candidate.author)
    {
        score += 2;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(provider: ProviderId, slug: &str, name: &str, author: &str) -> ProjectSummary {
        ProjectSummary {
            provider,
            project_id: slug.into(),
            slug: slug.into(),
            name: name.into(),
            summary: String::new(),
            author: author.into(),
            icon_url: None,
            website_url: String::new(),
            downloads: 1,
            updated_at: String::new(),
            categories: Vec::new(),
            supported_versions: Vec::new(),
            supported_loaders: Vec::new(),
            side: ProjectSide::Unknown,
            featured: None,
        }
    }

    #[test]
    fn accepts_loader_suffixes_but_rejects_other_projects() {
        let source = summary(
            ProviderId::Curseforge,
            "entityculling",
            "Entity Culling Fabric/Forge",
            "tr7zw",
        );
        let equivalent = summary(
            ProviderId::Modrinth,
            "entityculling",
            "Entity Culling",
            "tr7zw",
        );
        let unrelated = summary(
            ProviderId::Modrinth,
            "moreculling",
            "More Culling",
            "fxmorin",
        );
        assert!(identity_score(&source, &equivalent) >= 6);
        assert_eq!(identity_score(&source, &unrelated), 0);
    }
}
