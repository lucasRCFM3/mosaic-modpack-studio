use crate::{
    domain::*,
    error::{AppError, AppResult},
    infrastructure::store::JsonStore,
    providers::ProviderRegistry,
};
use chrono::Utc;
use futures_util::{StreamExt, stream};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct RecommendationService {
    providers: Arc<ProviderRegistry>,
    store: Arc<JsonStore>,
    candidates: RwLock<HashMap<String, CachedCandidate>>,
}

#[derive(Clone)]
struct CachedCandidate {
    pack: RecommendedPack,
    source: CandidateSource,
}

#[derive(Clone)]
enum CandidateSource {
    Official {
        provider: ProviderId,
        project_id: String,
    },
    Mosaic {
        recipe_id: &'static str,
    },
}

#[derive(Clone, Copy)]
struct MosaicRecipe {
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    tags: &'static [&'static str],
    default_target: (&'static str, ModLoader),
}

const RECIPES: &[MosaicRecipe] = &[
    MosaicRecipe {
        id: "performance",
        name: "Performance essencial",
        summary: "Mais FPS, menos memória e carregamento mais suave sem mudar a essência do jogo.",
        tags: &["Performance", "Leve", "Qualidade de vida"],
        default_target: ("1.21.1", ModLoader::Fabric),
    },
    MosaicRecipe {
        id: "magic-machines",
        name: "Magia, aventura e máquinas",
        summary: "Progressão técnica, magia, exploração e automação em uma base equilibrada.",
        tags: &["Tecnologia", "Magia", "Aventura"],
        default_target: ("1.20.1", ModLoader::Forge),
    },
    MosaicRecipe {
        id: "exploration",
        name: "Exploração renovada",
        summary: "Biomas, estruturas, mapas e viagens para transformar cada mundo em uma expedição.",
        tags: &["Exploração", "Estruturas", "Aventura"],
        default_target: ("1.20.1", ModLoader::Forge),
    },
    MosaicRecipe {
        id: "quality",
        name: "Qualidade de vida completa",
        summary: "Interface, receitas, informações e pequenos confortos que combinam com quase qualquer pack.",
        tags: &["Utilidade", "Interface", "Essencial"],
        default_target: ("1.21.1", ModLoader::Fabric),
    },
    MosaicRecipe {
        id: "storage-tech",
        name: "Armazenamento e tecnologia",
        summary: "Organização de itens, energia e automação para bases que crescem sem virar bagunça.",
        tags: &["Armazenamento", "Automação", "Tecnologia"],
        default_target: ("1.20.1", ModLoader::Forge),
    },
];

impl RecommendationService {
    pub fn new(providers: Arc<ProviderRegistry>, store: Arc<JsonStore>) -> Self {
        Self {
            providers,
            store,
            candidates: RwLock::new(HashMap::new()),
        }
    }

    pub async fn feed(
        &self,
        scope: RecommendationScope,
        target: Option<ProfileTarget>,
        seed: u64,
    ) -> AppResult<RecommendationFeed> {
        if scope == RecommendationScope::CurrentProfile && target.is_none() {
            return Err(AppError::Message(
                "Selecione um modpack para receber recomendações compatíveis.".into(),
            ));
        }
        let offset = ((seed % 6) * 6) as u32;
        let modrinth = self.providers.get(ProviderId::Modrinth);
        let curseforge = self.providers.get(ProviderId::Curseforge);
        let target_ref = target.as_ref();
        let (modrinth_result, curseforge_result) =
            tokio::join!(modrinth.search_modpacks(target_ref, offset, 6), async {
                if curseforge.is_enabled() {
                    curseforge.search_modpacks(target_ref, offset, 6).await
                } else {
                    Err(AppError::Message(
                        "Configure a chave da CurseForge para incluir os modpacks desse catálogo."
                            .into(),
                    ))
                }
            });

        let mut warnings = Vec::new();
        let mut raw_official = Vec::new();
        match modrinth_result {
            Ok(result) => raw_official.extend(result.projects),
            Err(error) => warnings.push(format!("Modrinth: {error}")),
        }
        match curseforge_result {
            Ok(result) => raw_official.extend(result.projects),
            Err(error) => warnings.push(format!("CurseForge: {error}")),
        }
        raw_official.sort_by(|left, right| right.downloads.cmp(&left.downloads));

        let mut candidates = Vec::new();
        for project in raw_official.into_iter().take(10) {
            let id = Uuid::new_v4().to_string();
            let provider = project.provider;
            let project_id = project.project_id.clone();
            let pack = RecommendedPack {
                id,
                kind: RecommendedPackKind::Official,
                provider: Some(provider),
                name: project.name,
                summary: project.summary,
                author: project.author,
                icon_url: project.icon_url,
                website_url: Some(project.website_url),
                downloads: project.downloads,
                updated_at: project.updated_at,
                tags: project.categories.into_iter().take(4).collect(),
                reason: target
                    .as_ref()
                    .map(|target| {
                        format!(
                            "Compatível com Minecraft {} + {}",
                            target.minecraft_version,
                            target.loader.as_str()
                        )
                    })
                    .unwrap_or_else(|| "Em destaque nos catálogos oficiais".into()),
                target: target.clone(),
            };
            candidates.push(CachedCandidate {
                pack,
                source: CandidateSource::Official {
                    provider,
                    project_id,
                },
            });
        }

        let recipe_count = 4.min(RECIPES.len());
        for index in 0..recipe_count {
            let recipe = RECIPES[(index + seed as usize) % RECIPES.len()];
            let recipe_target = target.clone().unwrap_or_else(|| ProfileTarget {
                minecraft_version: recipe.default_target.0.into(),
                loader: recipe.default_target.1,
                release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
            });
            let pack = RecommendedPack {
                id: Uuid::new_v4().to_string(),
                kind: RecommendedPackKind::Mosaic,
                provider: None,
                name: recipe.name.into(),
                summary: recipe.summary.into(),
                author: "Curadoria Mosaic".into(),
                icon_url: None,
                website_url: None,
                downloads: 0,
                updated_at: Utc::now().to_rfc3339(),
                tags: recipe.tags.iter().map(|tag| (*tag).into()).collect(),
                reason: if target.is_some() {
                    "Coleção adaptada ao seu modpack atual".into()
                } else {
                    "Receita modular criada pelo Mosaic".into()
                },
                target: Some(recipe_target),
            };
            candidates.push(CachedCandidate {
                pack,
                source: CandidateSource::Mosaic {
                    recipe_id: recipe.id,
                },
            });
        }

        let feed = RecommendationFeed {
            id: Uuid::new_v4().to_string(),
            generated_at: Utc::now().to_rfc3339(),
            scope,
            target,
            packs: candidates
                .iter()
                .map(|candidate| candidate.pack.clone())
                .collect(),
            warnings,
        };
        let records: Vec<_> = candidates.iter().map(stored_candidate).collect();
        let mut cache = self.candidates.write().await;
        for candidate in candidates {
            cache.insert(candidate.pack.id.clone(), candidate);
        }
        while cache.len() > 240 {
            if let Some(id) = cache.keys().next().cloned() {
                cache.remove(&id);
            }
        }
        drop(cache);
        self.store
            .update(|database| {
                for record in records {
                    database
                        .recommendation_candidates
                        .retain(|current| current.pack.id != record.pack.id);
                    database.recommendation_candidates.push(record);
                }
                if database.recommendation_candidates.len() > 240 {
                    let remove = database.recommendation_candidates.len() - 240;
                    database.recommendation_candidates.drain(0..remove);
                }
            })
            .await?;
        Ok(feed)
    }

    pub async fn preview(
        &self,
        recommendation_id: &str,
        desired_mod_count: Option<u16>,
    ) -> AppResult<RecommendedPackDetails> {
        let candidate =
            if let Some(candidate) = self.candidates.read().await.get(recommendation_id).cloned() {
                candidate
            } else {
                let record = self
                    .store
                    .snapshot()
                    .await
                    .recommendation_candidates
                    .into_iter()
                    .find(|candidate| candidate.pack.id == recommendation_id)
                    .ok_or_else(|| {
                        AppError::Message(
                        "Esta recomendação expirou. Atualize a lista para carregá-la novamente."
                            .into(),
                    )
                    })?;
                cached_candidate(record)?
            };
        match candidate.source {
            CandidateSource::Official {
                provider,
                project_id,
            } => {
                self.preview_official(candidate.pack, provider, &project_id)
                    .await
            }
            CandidateSource::Mosaic { recipe_id } => {
                self.preview_mosaic(
                    candidate.pack,
                    recipe_id,
                    desired_mod_count.unwrap_or(30) as usize,
                )
                .await
            }
        }
    }

    async fn preview_official(
        &self,
        pack: RecommendedPack,
        provider_id: ProviderId,
        project_id: &str,
    ) -> AppResult<RecommendedPackDetails> {
        let provider = self.providers.get(provider_id);
        let content = provider
            .get_modpack_content(project_id, pack.target.as_ref())
            .await?;
        let providers = self.providers.clone();
        let results = stream::iter(content.projects)
            .map(move |reference| {
                let providers = providers.clone();
                async move {
                    let result = providers
                        .get(reference.provider)
                        .get_project(&reference.project_id)
                        .await;
                    (reference, result)
                }
            })
            .buffer_unordered(12)
            .collect::<Vec<_>>()
            .await;
        let mut projects = Vec::new();
        let mut warnings = content.warnings;
        for (reference, result) in results {
            match result {
                Ok(project) => projects.push(project),
                Err(error) => warnings.push(format!(
                    "Não foi possível carregar {}: {error}",
                    reference.key()
                )),
            }
        }
        projects.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(RecommendedPackDetails {
            pack,
            target: content.target,
            projects,
            source_file_count: content.source_file_count,
            unresolved_file_count: content.unresolved_file_count,
            has_overrides: content.has_overrides,
            warnings,
        })
    }

    async fn preview_mosaic(
        &self,
        pack: RecommendedPack,
        recipe_id: &'static str,
        desired_mod_count: usize,
    ) -> AppResult<RecommendedPackDetails> {
        let target = pack.target.clone().ok_or_else(|| {
            AppError::Message("A coleção Mosaic não possui um destino definido.".into())
        })?;
        let queries = recipe_queries(recipe_id, target.loader);
        let mut projects = Vec::new();
        let mut warnings = Vec::new();
        let mut seen = HashSet::new();
        let mut seen_names = HashSet::new();
        let mut unavailable: Vec<String> = Vec::new();
        for query_chunk in queries.chunks(8) {
            let target_ref = &target;
            let owned_queries = query_chunk
                .iter()
                .map(|query| (*query).to_string())
                .collect::<Vec<_>>();
            let mut results = stream::iter(owned_queries.into_iter().enumerate())
                .map(|(index, query)| async move {
                    let result = self.find_recipe_project(&query, target_ref).await;
                    (index, query, result)
                })
                .buffer_unordered(6)
                .collect::<Vec<_>>()
                .await;
            results.sort_by_key(|(index, _, _)| *index);
            for (_, query, result) in results {
                match result {
                    Some(project)
                        if seen.insert(project_key(&project))
                            && seen_names.insert(normalize(&project.name)) =>
                    {
                        projects.push(project)
                    }
                    Some(_) => {}
                    None => unavailable.push(query),
                }
                if projects.len() >= desired_mod_count {
                    break;
                }
            }
            if projects.len() >= desired_mod_count {
                break;
            }
        }
        if projects.len() < desired_mod_count {
            for term in recipe_discovery_terms(recipe_id) {
                let candidates = self.search_recipe_candidates(term, &target).await;
                for project in candidates {
                    if is_user_facing_project(&project)
                        && seen.insert(project_key(&project))
                        && seen_names.insert(normalize(&project.name))
                    {
                        projects.push(project);
                    }
                    if projects.len() >= desired_mod_count {
                        break;
                    }
                }
                if projects.len() >= desired_mod_count {
                    break;
                }
            }
        }
        if projects.is_empty() {
            return Err(AppError::Message(
                "Nenhum mod desta coleção está disponível para o destino escolhido.".into(),
            ));
        }
        if !unavailable.is_empty() {
            let shown = unavailable
                .iter()
                .take(5)
                .map(String::as_str)
                .collect::<Vec<_>>();
            let remaining = unavailable.len().saturating_sub(shown.len());
            warnings.push(if remaining > 0 {
                format!(
                    "{} itens do núcleo curado não estavam disponíveis nesse destino: {} e mais {remaining}.",
                    unavailable.len(),
                    shown.join(", ")
                )
            } else {
                format!(
                    "Alguns itens do núcleo curado não estavam disponíveis nesse destino: {}.",
                    shown.join(", ")
                )
            });
        }
        if projects.len() < desired_mod_count {
            warnings.push(format!(
                "A coleção encontrou {} dos {desired_mod_count} mods principais solicitados para Minecraft {} + {}.",
                projects.len(),
                target.minecraft_version,
                target.loader.as_str()
            ));
        }
        projects.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(RecommendedPackDetails {
            pack,
            target,
            source_file_count: projects.len(),
            unresolved_file_count: unavailable.len(),
            projects,
            has_overrides: false,
            warnings,
        })
    }

    async fn find_recipe_project(
        &self,
        query: &str,
        target: &ProfileTarget,
    ) -> Option<ProjectSummary> {
        let filters = SearchFilters {
            query: query.into(),
            minecraft_version: target.minecraft_version.clone(),
            loader: target.loader,
            release_channels: target.release_channels.clone(),
            providers: vec![ProviderId::Modrinth, ProviderId::Curseforge],
            side: SearchSide::Any,
            sort: SearchSort::Relevance,
            limit: Some(8),
        };
        let modrinth = self.providers.get(ProviderId::Modrinth);
        if let Ok(result) = modrinth.search(&filters).await
            && let Some(project) = best_recipe_match(query, result.projects)
        {
            return Some(project);
        }
        let curseforge = self.providers.get(ProviderId::Curseforge);
        if !curseforge.is_enabled() {
            return None;
        }
        curseforge
            .search(&filters)
            .await
            .ok()
            .and_then(|result| best_recipe_match(query, result.projects))
    }

    async fn search_recipe_candidates(
        &self,
        query: &str,
        target: &ProfileTarget,
    ) -> Vec<ProjectSummary> {
        let filters = SearchFilters {
            query: query.into(),
            minecraft_version: target.minecraft_version.clone(),
            loader: target.loader,
            release_channels: target.release_channels.clone(),
            providers: vec![ProviderId::Modrinth, ProviderId::Curseforge],
            side: SearchSide::Any,
            sort: SearchSort::Downloads,
            limit: Some(20),
        };
        let modrinth = self.providers.get(ProviderId::Modrinth);
        let curseforge = self.providers.get(ProviderId::Curseforge);
        let (modrinth_result, curseforge_result) = tokio::join!(modrinth.search(&filters), async {
            if curseforge.is_enabled() {
                curseforge.search(&filters).await.ok()
            } else {
                None
            }
        });
        let mut projects = modrinth_result
            .ok()
            .map(|result| result.projects)
            .unwrap_or_default();
        if let Some(result) = curseforge_result {
            projects.extend(result.projects);
        }
        projects.sort_by(|left, right| right.downloads.cmp(&left.downloads));
        projects
    }
}

fn best_recipe_match(query: &str, mut projects: Vec<ProjectSummary>) -> Option<ProjectSummary> {
    projects.sort_by_key(|project| recipe_score(query, project));
    projects
        .into_iter()
        .find(|project| recipe_score(query, project).0 < 3)
}

fn recipe_queries(recipe_id: &str, loader: ModLoader) -> Vec<&'static str> {
    let fabric_family = matches!(loader, ModLoader::Fabric | ModLoader::Quilt);
    match recipe_id {
        "performance" => vec![
            if fabric_family { "Sodium" } else { "Embeddium" },
            if fabric_family {
                "Iris Shaders"
            } else {
                "Oculus"
            },
            "Lithium",
            "FerriteCore",
            "ImmediatelyFast",
            "Entity Culling",
            "ModernFix",
            "Dynamic FPS",
            "Memory Leak Fix",
            "BadOptimizations",
            "Clumps",
            "Alternate Current",
            "FastQuit",
            "Smooth Boot",
            "More Culling",
            "Enhanced Block Entities",
            "Let Me Despawn",
            "spark",
            "Chunky",
            "ServerCore",
            "Fast IP Ping",
            "Better Biome Blend",
            "LazyDFU",
            "Krypton",
        ],
        "magic-machines" if fabric_family => vec![
            "Modern Industrialization",
            "Tech Reborn",
            "Applied Energistics 2",
            "Botania",
            "Spectrum",
            "Bewitchment",
            "Create Fabric",
            "CC Tweaked",
            "Tom's Simple Storage Mod",
            "Iron Chests",
            "Traveler's Backpack",
            "Waystones",
            "Artifacts",
            "Mythic Metals",
            "Ad Astra",
            "Powah",
            "Patchouli",
            "Farmer's Delight Refabricated",
            "EMI",
            "Jade",
            "AppleSkin",
            "Mouse Tweaks",
            "Nature's Compass",
            "Comforts",
        ],
        "magic-machines" => vec![
            "Create",
            "Ars Nouveau",
            "Applied Energistics 2",
            "Mekanism",
            "Thermal Expansion",
            "Immersive Engineering",
            "Industrial Foregoing",
            "Botania",
            "Occultism",
            "Iron's Spells 'n Spellbooks",
            "Blood Magic",
            "PneumaticCraft Repressurized",
            "Powah",
            "Flux Networks",
            "Pipez",
            "Sophisticated Storage",
            "Sophisticated Backpacks",
            "Storage Drawers",
            "CC Tweaked",
            "Create Crafts & Additions",
            "Create Steam 'n' Rails",
            "Farmer's Delight",
            "Just Enough Items",
            "Jade",
            "Waystones",
            "Patchouli",
        ],
        "exploration" => vec![
            "Waystones",
            "YUNG's Better Dungeons",
            "YUNG's Better Strongholds",
            "YUNG's Better Mineshafts",
            "YUNG's Better Desert Temples",
            "YUNG's Better Nether Fortresses",
            "When Dungeons Arise",
            "Towns and Towers",
            "Repurposed Structures",
            "Dungeon Crawl",
            "Terralith",
            "Tectonic",
            if fabric_family {
                "BetterNether"
            } else {
                "Biomes O' Plenty"
            },
            if fabric_family {
                "BetterEnd"
            } else {
                "The Aether"
            },
            "Regions Unexplored",
            "Deeper and Darker",
            "End Remastered",
            "Naturalist",
            "Friends&Foes",
            "Simply Swords",
            "Better Combat",
            "Artifacts",
            "Lootr",
            "Nature's Compass",
            "Explorer's Compass",
            "Xaero's Minimap",
            "Xaero's World Map",
            "Traveler's Backpack",
            "Small Ships",
            "Goblin Traders",
        ],
        "quality" => vec![
            if fabric_family {
                "EMI"
            } else {
                "Just Enough Items"
            },
            "Jade",
            "AppleSkin",
            "Mouse Tweaks",
            "ShulkerBoxTooltip",
            "Controlling",
            "Inventory Profiles Next",
            "BetterF3",
            "Better Advancements",
            "Crafting Tweaks",
            "Carry On",
            "Light Overlay",
            "Enchantment Descriptions",
            "Legendary Tooltips",
            "Not Enough Animations",
            "Eating Animation",
            "3D Skin Layers",
            "Presence Footsteps",
            "Sound Physics Remastered",
            "Falling Leaves",
            "Polymorph",
            "TrashSlot",
            "Comforts",
            "Easy Magic",
            "Easy Anvils",
            "Chat Heads",
            if fabric_family {
                "Zoomify"
            } else {
                "Just Zoom"
            },
            "Pick Up Notifier",
            "Advancement Plaques",
            "Better Third Person",
            "Xaero's Minimap",
            "Xaero's World Map",
        ],
        "storage-tech" if fabric_family => vec![
            "Applied Energistics 2",
            "Modern Industrialization",
            "Tech Reborn",
            "Tom's Simple Storage Mod",
            "Iron Chests",
            "Storage Drawers",
            "Simple Copper Pipes",
            "Create Fabric",
            "CC Tweaked",
            "Powah",
            "Ad Astra",
            "Industrial Revolution",
            "Traveler's Backpack",
            "Dank Storage",
            "Functional Storage",
            "Inventory Profiles Next",
            "Mouse Tweaks",
            "Jade",
            "EMI",
        ],
        "storage-tech" => vec![
            "Applied Energistics 2",
            "Mekanism",
            "Sophisticated Storage",
            "Sophisticated Backpacks",
            "Storage Drawers",
            "Functional Storage",
            "Iron Chests",
            "Refined Storage",
            "Tom's Simple Storage Mod",
            "Create",
            "Thermal Expansion",
            "Industrial Foregoing",
            "Immersive Engineering",
            "Powah",
            "Flux Networks",
            "Pipez",
            "Iron Furnaces",
            "Dank Storage",
            "CC Tweaked",
            "RFTools Utility",
            "Extreme Reactors",
            "Ender IO",
            "Inventory Sorter",
            "Jade",
            "Just Enough Items",
        ],
        _ => Vec::new(),
    }
}

fn recipe_discovery_terms(recipe_id: &str) -> &'static [&'static str] {
    match recipe_id {
        "performance" => &[
            "performance optimization",
            "fps optimization",
            "memory performance",
            "server performance",
            "render optimization",
        ],
        "magic-machines" => &[
            "magic technology",
            "automation machines",
            "create addon",
            "magic adventure",
            "technology energy",
        ],
        "exploration" => &[
            "adventure exploration",
            "world generation biomes",
            "dungeons structures",
            "dimensions exploration",
            "mobs bosses",
        ],
        "quality" => &[
            "quality of life",
            "inventory utility",
            "interface hud",
            "map minimap",
            "building utility",
        ],
        "storage-tech" => &[
            "storage automation",
            "technology energy",
            "item transport pipes",
            "machines factory",
            "computer storage",
        ],
        _ => &[],
    }
}

fn is_user_facing_project(project: &ProjectSummary) -> bool {
    !project.categories.iter().any(|category| {
        matches!(
            normalize(category).as_str(),
            "library" | "apiandlibrary" | "libraryapi"
        )
    })
}

fn recipe_score(query: &str, project: &ProjectSummary) -> (u8, std::cmp::Reverse<u64>) {
    let query = normalize(query);
    let name = normalize(&project.name);
    let slug = normalize(&project.slug);
    let score = if name == query || slug == query {
        0
    } else if name.starts_with(&query) {
        1
    } else if name.contains(&query) {
        2
    } else {
        3
    };
    (score, std::cmp::Reverse(project.downloads))
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn project_key(project: &ProjectSummary) -> String {
    format!("{}:{}", project.provider.as_str(), project.project_id)
}

fn stored_candidate(candidate: &CachedCandidate) -> StoredRecommendationCandidate {
    let source = match &candidate.source {
        CandidateSource::Official {
            provider,
            project_id,
        } => StoredRecommendationSource::Official {
            provider: *provider,
            project_id: project_id.clone(),
        },
        CandidateSource::Mosaic { recipe_id } => StoredRecommendationSource::Mosaic {
            recipe_id: (*recipe_id).into(),
        },
    };
    StoredRecommendationCandidate {
        pack: candidate.pack.clone(),
        source,
    }
}

fn cached_candidate(record: StoredRecommendationCandidate) -> AppResult<CachedCandidate> {
    let source = match record.source {
        StoredRecommendationSource::Official {
            provider,
            project_id,
        } => CandidateSource::Official {
            provider,
            project_id,
        },
        StoredRecommendationSource::Mosaic { recipe_id } => CandidateSource::Mosaic {
            recipe_id: RECIPES
                .iter()
                .find(|recipe| recipe.id == recipe_id)
                .map(|recipe| recipe.id)
                .ok_or_else(|| AppError::Message("Esta coleção Mosaic não existe mais.".into()))?,
        },
    };
    Ok(CachedCandidate {
        pack: record.pack,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{secrets::SecretStore, store::JsonStore};

    #[test]
    fn recipes_adapt_their_core_mods_to_the_loader_family() {
        assert_eq!(
            recipe_queries("performance", ModLoader::Fabric)[0],
            "Sodium"
        );
        assert_eq!(
            recipe_queries("performance", ModLoader::Forge)[0],
            "Embeddium"
        );
        assert!(recipe_queries("magic-machines", ModLoader::Forge).contains(&"Create"));
        for recipe in RECIPES {
            assert!(recipe_queries(recipe.id, ModLoader::Forge).len() >= 19);
            assert!(recipe_discovery_terms(recipe.id).len() >= 5);
        }
    }

    #[test]
    fn exact_names_beat_merely_related_search_results() {
        let project = |name: &str, downloads| ProjectSummary {
            provider: ProviderId::Modrinth,
            project_id: name.into(),
            slug: name.into(),
            name: name.into(),
            summary: String::new(),
            author: String::new(),
            icon_url: None,
            website_url: String::new(),
            downloads,
            updated_at: String::new(),
            categories: Vec::new(),
            supported_versions: Vec::new(),
            supported_loaders: Vec::new(),
            side: ProjectSide::Unknown,
            featured: None,
        };
        assert!(
            recipe_score("Jade", &project("Jade", 1))
                < recipe_score("Jade", &project("Jade Addons", 10_000))
        );
    }

    #[tokio::test]
    #[ignore = "consulta e baixa um índice público da Modrinth"]
    async fn live_feed_opens_an_official_modrinth_pack() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            JsonStore::load(directory.path().join("state.json"))
                .await
                .unwrap(),
        );
        let providers = Arc::new(ProviderRegistry::new(Arc::new(SecretStore::new())).unwrap());
        let service = RecommendationService::new(providers, store);
        let feed = service
            .feed(RecommendationScope::AllVersions, None, 0)
            .await
            .unwrap();
        let pack = feed
            .packs
            .into_iter()
            .find(|pack| pack.provider == Some(ProviderId::Modrinth))
            .unwrap();

        let details = service.preview(&pack.id, None).await.unwrap();

        assert!(!details.projects.is_empty());
        assert!(details.source_file_count >= details.projects.len());
        assert!(!details.target.minecraft_version.is_empty());
    }

    #[tokio::test]
    #[ignore = "consulta e baixa um manifesto público da CurseForge"]
    async fn live_feed_opens_an_official_curseforge_pack_when_configured() {
        let secrets = Arc::new(SecretStore::new());
        let providers = Arc::new(ProviderRegistry::new(secrets).unwrap());
        if !providers.get(ProviderId::Curseforge).is_enabled() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            JsonStore::load(directory.path().join("state.json"))
                .await
                .unwrap(),
        );
        let service = RecommendationService::new(providers, store);
        let feed = service
            .feed(RecommendationScope::AllVersions, None, 0)
            .await
            .unwrap();
        let pack = feed
            .packs
            .into_iter()
            .find(|pack| pack.provider == Some(ProviderId::Curseforge))
            .unwrap();

        let details = service.preview(&pack.id, None).await.unwrap();

        assert!(!details.projects.is_empty());
        assert!(!details.target.minecraft_version.is_empty());
    }

    #[tokio::test]
    #[ignore = "consulta os catálogos públicos para montar uma coleção grande"]
    async fn live_mosaic_collection_reaches_the_requested_size() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            JsonStore::load(directory.path().join("state.json"))
                .await
                .unwrap(),
        );
        let providers = Arc::new(ProviderRegistry::new(Arc::new(SecretStore::new())).unwrap());
        let service = RecommendationService::new(providers, store);
        let target = ProfileTarget {
            minecraft_version: "1.20.1".into(),
            loader: ModLoader::Forge,
            release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
        };
        let feed = service
            .feed(RecommendationScope::CurrentProfile, Some(target), 0)
            .await
            .unwrap();
        let pack = feed
            .packs
            .into_iter()
            .find(|pack| pack.kind == RecommendedPackKind::Mosaic)
            .unwrap();

        let details = service.preview(&pack.id, Some(60)).await.unwrap();

        assert_eq!(details.projects.len(), 60);
        assert_eq!(
            details
                .projects
                .iter()
                .map(|project| normalize(&project.name))
                .collect::<HashSet<_>>()
                .len(),
            60
        );
    }
}
