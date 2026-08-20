import type { InstalledMod } from '../../shared/domain';

const reasonLabels: Record<InstalledMod['reason'], string> = {
  requested: 'escolhido solicitado manual você',
  required: 'dependência obrigatória automático',
  optional: 'dependência opcional',
};

export function filterInstalledMods(mods: InstalledMod[], query: string): InstalledMod[] {
  const normalized = query.trim().toLocaleLowerCase('pt-BR');
  if (!normalized) return mods;
  return mods.filter((mod) => [
    mod.name,
    mod.filename,
    mod.versionNumber,
    mod.provider,
    reasonLabels[mod.reason],
  ].some((value) => value.toLocaleLowerCase('pt-BR').includes(normalized)));
}
