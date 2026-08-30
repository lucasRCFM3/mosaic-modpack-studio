# Mosaic Modpack Studio

[![CI](https://github.com/lucasRCFM3/mosaic-modpack-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/lucasRCFM3/mosaic-modpack-studio/actions/workflows/ci.yml)

Aplicativo desktop local-first, construído com **Rust + Tauri 2 + React**, para pesquisar, resolver e instalar mods compatíveis da Modrinth e, mediante chave oficial, da CurseForge.

![Tela de descoberta do Mosaic](docs/mosaic-preview.png)

## O que funciona

- busca unificada e paralela nos dois catálogos;
- filtros por versão do Minecraft, loader, ambiente, origem e ordenação;
- resolução recursiva do grafo de dependências no backend Rust;
- dependências obrigatórias instaladas automaticamente;
- dependências opcionais exibidas para escolha e desmarcadas por padrão;
- seleção ou remoção de todas as dependências opcionais com uma única ação;
- fallback automático entre CurseForge e Modrinth quando uma fonte bloqueia o download;
- reconciliação cruzada de metadados para recuperar dependências obrigatórias omitidas por uma das fontes;
- detecção de ciclos, incompatibilidades, versões ausentes e distribuição bloqueada;
- revisão do plano antes de qualquer escrita em disco;
- fila de instalação persistente por perfil para selecionar vários mods durante a busca e resolver todo o lote de uma vez;
- pesquisa por nome, autor, origem ou categoria dentro da fila de instalação;
- downloads concorrentes por HTTPS, arquivo `.part`, verificação de hash e renomeação;
- perfis independentes com nome e descrição editáveis, registro dos mods e lockfile JSON exportável;
- duplicação transacional de modpacks, com cópia completa ou uma nova instância somente com os mods registrados;
- importação e reindexação de instâncias existentes, com prévia e detecção automática de Minecraft e loader;
- separação revisável dos mods em Cliente, Servidor, Cliente e Servidor e Não classificados;
- remoção conservadora que preserva qualquer dependência ainda referenciada por outro mod instalado;
- identifica na lista quais mods utilizam cada dependência, inclusive em cadeias transitivas;
- gera uma lista TXT que anota em cada dependência quais mods principais precisam dela;
- predefinições reutilizáveis que verificam versões compatíveis e resolvem todas as dependências em lote;
- descoberta de modpacks oficiais da Modrinth e CurseForge, com leitura dos manifests `.mrpack` e CurseForge;
- coleções Mosaic adaptadas ao Minecraft e loader atuais, pesquisa interna, seleção parcial e histórico de recomendações;
- chave da CurseForge guardada no cofre de credenciais do sistema operacional;
- frontend sem acesso direto ao sistema de arquivos ou aos segredos;
- instalador NSIS nativo para Windows.

## Dependências automáticas

| Tipo informado pelo catálogo | Comportamento |
| --- | --- |
| Obrigatória (`required`) | Incluída automaticamente no plano |
| Opcional (`optional`) | Exibida desmarcada; o usuário escolhe individualmente |
| Embutida (`embedded`) | Não é baixada separadamente |
| Incompatível (`incompatible`) | Bloqueia o plano quando o conflito está presente |

Existe uma preferência para pré-selecionar opcionais, mas ela é **opt-in** e vem desativada em instalações novas.

Na tela do modpack, cada dependência informa quais mods principais precisam dela. Relações transitivas são apresentadas pela origem mais útil para o usuário: se `Mod A` usa `Biblioteca B`, que usa `Núcleo C`, tanto B quanto C indicam que são usados por A. A pesquisa da lista também considera esses nomes relacionados.

Quando uma fonte não fornece URL para aplicativos de terceiros, o Mosaic procura uma cópia equivalente na outra fonte e só a aceita após conferir a identidade do projeto, versão do Minecraft e loader. Se nenhuma alternativa segura existir, o item passa a exigir instalação manual, mas não bloqueia os demais downloads do plano.

O mesmo cruzamento é aplicado aos metadados: se a versão equivalente na outra plataforma declarar uma dependência obrigatória ausente na fonte escolhida, o Mosaic a incorpora ao grafo. Primeiro tenta baixar essa dependência pela fonte original do mod; se ela não estiver disponível ali, usa a plataforma que forneceu o metadado. Projetos equivalentes são deduplicados antes da instalação.

## Instalação em lote

Em **Descobrir**, use **À lista** nos projetos desejados e continue pesquisando normalmente. A lista permanece vinculada ao modpack atual e permite remover itens ou limpar tudo. Ao clicar em **Instalar todos**, o Mosaic cria um único plano, elimina projetos e dependências duplicados, verifica a compatibilidade de todo o conjunto e baixa as dependências obrigatórias após a confirmação.

## Duplicar um modpack

Em **Meu modpack → Duplicar**, escolha o nome e, opcionalmente, uma pasta vazia. A **Cópia completa** preserva toda a instância, incluindo mods nas versões atuais, configurações, saves, resource packs e scripts. A **Cópia limpa** leva somente os arquivos de mods registrados pelo Mosaic, mantendo o mesmo Minecraft, loader e grafo de dependências sem copiar saves, configurações, logs ou JARs manuais.

A cópia é montada em uma pasta temporária e o perfil novo só aparece depois da conclusão. Pastas não vazias, destinos sobrepostos à origem e links simbólicos são recusados para impedir sobrescrita, recursão e escape da instância.

## Separar mods por ambiente

Em **Meu modpack → Separar mods**, o Mosaic consulta o ambiente informado pelo provedor original. Quando a CurseForge não possui essa informação, ele procura com verificação de identidade o projeto equivalente na Modrinth. A tela de revisão permite pesquisar e corrigir manualmente qualquer classificação antes de gerar quatro pastas: `Cliente`, `Servidor`, `Cliente e Servidor` e `Não classificados`.

Por segurança, a função **copia** os JARs para uma exportação organizada e preserva a pasta `mods/` ativa. Colocar segundas cópias dentro dela poderia fazer o loader carregar mods duplicados ou deixar de encontrá-los em subpastas. O destino inclui um `manifesto.txt`, nunca sobrescreve uma exportação existente e não pode ficar dentro da pasta ativa de mods.

## Importar ou substituir uma instância

Em **Meu modpack → Rescanear pasta**, escolha a raiz de uma instância que contenha `mods/` ou selecione a própria pasta `mods`. O Mosaic analisa tudo primeiro e exibe uma prévia com o caminho, Minecraft, loader, arquivos reconhecidos, arquivos locais e avisos. O perfil só é substituído depois da confirmação; as pastas antiga e nova não são apagadas nem movidas.

A detecção prioriza `minecraftinstance.json` do CurseForge, `mmc-pack.json` e `instance.cfg` do Prism/MultiMC, além de perfis JSON do Modrinth e outros launchers. Quando esses arquivos não bastam, o Mosaic combina os metadados internos `fabric.mod.json`, `quilt.mod.json`, `mods.toml` e `neoforge.mods.toml` com a identificação por hash na Modrinth e fingerprint na CurseForge.

Mods reconhecidos recuperam nome, versão, origem, hashes e dependências obrigatórias. JARs sem correspondência continuam registrados como **Local**, portanto aparecem na lista, no TXT e na separação por ambiente. Quando a evidência não é suficiente para detectar todo o alvo, a prévia informa quais campos foram mantidos do perfil anterior.

## Predefinições de mods

Abra **Predefinições → Nova predefinição**. Você pode pesquisar projetos diretamente na Modrinth e CurseForge, mesmo que não estejam instalados, ou aproveitar mods do perfil atual. Escolha os itens da lista e salve. Ao aplicar essa predefinição em outro perfil, o Mosaic:

1. consulta uma versão compatível de cada projeto para o Minecraft e loader do destino;
2. monta um único grafo, removendo dependências duplicadas;
3. mostra incompatibilidades e bloqueia instalações parciais inseguras;
4. mantém dependências opcionais desmarcadas, salvo se o usuário optar pelo contrário;
5. instala o lote somente após confirmação do plano.

## Modpacks recomendados

Abra **Modpacks** para alternar entre sugestões compatíveis com o perfil atual e inspirações de qualquer versão. Cada rodada mistura projetos oficiais publicados nos catálogos com coleções modulares criadas pelo Mosaic. O histórico das últimas rodadas fica disponível mesmo depois de reiniciar o aplicativo.

Ao abrir uma sugestão, o Mosaic lê o manifesto oficial, identifica os projetos, permite pesquisar e selecionar somente os mods desejados e marca os que já estão instalados. A seleção pode ser adicionada ao perfil atual quando Minecraft e loader coincidem ou usada para criar um perfil separado com o destino correto. Em ambos os casos, o resolvedor normal recalcula dependências obrigatórias, opcionais e conflitos antes de instalar.

Essa ação é deliberadamente uma **importação modular dos mods**. Configurações, scripts, resource packs e pastas `overrides` são detectados e informados, mas ainda não são copiados; portanto, criar um perfil “baseado no pack” não é apresentado como uma cópia exata do modpack oficial.

## Desenvolvimento

Requisitos no Windows:

- Node.js 22 ou mais recente;
- Rust stable com o alvo `x86_64-pc-windows-msvc`;
- Microsoft C++ Build Tools;
- Microsoft Edge WebView2 Runtime.

```powershell
npm install
npm run dev
```

## Validar e empacotar

```powershell
npm run check
npm run package
```

O instalador é criado em `src-tauri/target/release/bundle/nsis/`.

## Arquitetura

```text
src/
  renderer/                 interface React e ponte tipada para os comandos Tauri
  shared/                   contratos usados pelo frontend
src-tauri/src/
  application/             catálogo, perfis, resolução e downloads
  infrastructure/          persistência JSON e cofre de credenciais
  providers/               adaptadores Modrinth e CurseForge
  commands.rs              fronteira Tauri e validação de entrada
  domain.rs                modelo de domínio independente das APIs
  state.rs                 composição da aplicação
```

As decisões técnicas e os limites dos módulos estão em [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

Para contribuir, consulte [CONTRIBUTING.md](CONTRIBUTING.md). Questões de segurança devem seguir [SECURITY.md](SECURITY.md), e o tratamento de dados está descrito em [PRIVACY.md](PRIVACY.md).

## CurseForge: requisito importante

A integração fica desabilitada até o usuário fornecer sua própria chave. A CurseForge exige solicitação e aprovação de chave, e projetos cujo autor desativou a distribuição externa podem não oferecer URL direta. O Mosaic detecta esse caso e direciona o usuário à página oficial.

- [Como solicitar uma chave](https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key)
- [Termos da API de terceiros](https://support.curseforge.com/support/solutions/articles/9000207405-curseforge-3rd-party-api-terms-and-conditions)
- [Documentação da API CurseForge](https://docs.curseforge.com/rest-api/)
- [Documentação da API Modrinth](https://docs.modrinth.com/api/)

## Privacidade

Não há telemetria. Preferências e perfis ficam no diretório de dados do aplicativo, enquanto os mods ficam na pasta escolhida para cada instância. A chave da CurseForge nunca é devolvida ao frontend: a interface recebe somente o estado “configurada/não configurada”.

## Licença

Nenhuma licença de código aberto foi definida ainda. Até que uma licença seja adicionada, todos os direitos permanecem reservados ao autor.
