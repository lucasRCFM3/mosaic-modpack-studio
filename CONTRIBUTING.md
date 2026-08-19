# Contribuindo

Obrigado por considerar uma contribuição ao Mosaic Modpack Studio.

## Preparação

No Windows, instale Node.js 22+, Rust stable com o alvo MSVC, Microsoft C++ Build Tools e WebView2 Runtime.

```powershell
npm ci
npm run check
npm run dev
```

## Pull requests

- mantenha regras de catálogo dentro dos adaptadores de provedor;
- mantenha rede, disco e segredos fora do frontend;
- adicione testes para mudanças no resolvedor ou no comportamento da interface;
- não inclua builds, chaves de API, dados de perfis ou arquivos de mods;
- execute `npm run check` antes de abrir o pull request.

Mudanças que afetem distribuição de arquivos da CurseForge devem preservar as preferências dos autores e os termos vigentes da API.
