# Arquitetura do Mosaic

## Visão geral

O Mosaic é um aplicativo Tauri 2. O frontend React cuida exclusivamente de apresentação e interação; todas as decisões de compatibilidade, rede, segredos e disco pertencem ao núcleo Rust.

```text
React
  │ invoke/eventos tipados
  ▼
Comandos Tauri
  ├── CatalogService ───────► ProviderRegistry ─► Modrinth / CurseForge
  ├── DependencyResolver ───► reconciliação entre fontes + grafo + plano imutável
  ├── PresetService ────────► coleções reutilizáveis de projetos
  ├── DownloadManager ──────► HTTPS + hash + mods/
  └── ProfileService ───────► JSON local + lockfile
```

## Fronteiras dos módulos

- `renderer`: estado da interface, filtros, revisão do plano e progresso. Não acessa Node.js, arquivos, URLs de download ou segredos diretamente.
- `commands.rs`: API pública do desktop. Valida entradas, traduz erros e expõe somente operações de alto nível.
- `application`: casos de uso. Coordena provedores, resolve dependências, instala e atualiza o estado dos perfis.
- `providers`: adaptadores que normalizam respostas externas para o domínio do Mosaic.
- `infrastructure`: detalhes substituíveis de persistência e credenciais.
- `domain.rs`: tipos compartilhados pelo núcleo, sem dependência de formato específico de provedor.

## Resolução de dependências

O resolvedor percorre o grafo recursivamente e mantém conjuntos de nós visitados e em visita para deduplicação e detecção de ciclos.

1. Seleciona a versão mais recente compatível com Minecraft, loader e canal do perfil.
2. Localiza com critérios conservadores a versão equivalente na outra plataforma e compara as dependências obrigatórias declaradas.
3. Para cada dependência ausente, prefere uma cópia compatível na fonte original e usa a fonte complementar como fallback.
4. Adiciona dependências `required` reconciliadas ao plano e continua a travessia, deduplicando identidades equivalentes.
5. Registra dependências `optional` em uma lista de escolhas separada.
6. Só percorre uma opcional quando o ID dela foi explicitamente selecionado.
7. Ignora `embedded` e transforma `incompatible` em bloqueio quando há conflito.
8. Mantém o plano no backend; a interface devolve apenas seu identificador para instalar.

Essa separação garante que dependências opcionais não sejam instaladas por acidente. A preferência global de pré-seleção é apenas uma conveniência opt-in.

### Remoção segura

Antes de remover um mod, o serviço recompõe o grafo combinando as relações registradas no lockfile com metadados atuais das duas plataformas. A coleta de dependências órfãs usa referências de **todos** os mods restantes, independentemente de terem sido originalmente solicitados, opcionais ou instalados como dependência. Se algum trecho do grafo não puder ser verificado, a coleta automática é desativada e somente o mod explicitamente solicitado pode ser removido; dependências são preservadas por segurança.

O renderer deriva um índice reverso dessas relações para explicar o uso de cada dependência sem duplicar estado persistido. A travessia reversa chega aos mods raiz, permitindo apresentar relações transitivas e pesquisar uma biblioteca pelo nome dos mods que a utilizam.

### Duplicação transacional

O `ProfileService` serializa duplicações e valida que origem e destino não sejam iguais, ancestrais ou descendentes. O destino precisa estar ausente ou vazio e não pode pertencer a outro perfil. Os arquivos são copiados primeiro para um diretório `.part` irmão ao destino; links simbólicos e itens especiais são recusados. Depois de conferir que o perfil original não mudou durante a operação, um `rename` no mesmo volume publica a instância e só então o novo perfil é persistido. Falhas removem o estágio e restauram uma pasta vazia escolhida pelo usuário.

O modo completo percorre toda a árvore. O modo limpo copia apenas os nomes de arquivos registrados no lockfile para `mods/`; metadados de projeto, versões, hashes, motivos e arestas de dependência são clonados para que as duas instâncias evoluam independentemente.

### Resolução de predefinições

Uma predefinição guarda referências estáveis de projetos e nomes para apresentação, nunca URLs ou arquivos de uma versão específica. Ao aplicá-la, todos os projetos entram como raízes do mesmo `ResolveContext`. Isso permite escolher versões adequadas ao perfil atual, deduplicar dependências compartilhadas e produzir um único plano atômico. Qualquer raiz incompatível gera um erro e impede a instalação parcial do lote.

## Download e integridade

O `DownloadManager` limita concorrência e aplica as seguintes proteções:

- somente HTTPS;
- hosts de download permitidos por provedor;
- nome de arquivo sanitizado e contenção dentro de `mods/`;
- escrita em arquivo temporário `.part`;
- hash SHA-512, SHA-1 ou MD5 validado durante a transferência;
- remoção do temporário em qualquer falha;
- substituição do arquivo final somente após download íntegro;
- progresso emitido por eventos Tauri.

## Persistência e segredos

Perfis e preferências são JSON versionado no diretório de dados do aplicativo. A escrita usa arquivo temporário antes da substituição. A chave da CurseForge é armazenada pelo gerenciador de credenciais do sistema através de `keyring` e nunca faz parte do JSON ou de uma resposta enviada ao frontend.

## Superfície de segurança

- CSP definida na configuração Tauri;
- capacidade padrão mínima, sem plugins de shell ou filesystem expostos à UI;
- URLs externas abertas somente após validação HTTPS;
- caminhos escolhidos por diálogo nativo;
- downloads autorizados por um plano criado pelo próprio backend.

## Verificação

O comando `npm run check` executa:

- verificação estática TypeScript;
- testes de comportamento do frontend;
- testes unitários Rust;
- build de produção do frontend.

Há ainda um teste de integração ignorado por padrão que consulta a Modrinth real e confirma que opcionais permanecem fora do plano quando não selecionadas.

## Próximas capacidades

- importação e exportação `.mrpack`, manifestos CurseForge e instâncias Prism/MultiMC;
- atualização em lote com snapshot e rollback;
- identificação de mods existentes por hash;
- regras de pins, overrides e presets de servidor;
- cache HTTP com ETag e retomada de downloads;
- auditoria de licenças, changelogs e vulnerabilidades;
- assinatura de lockfiles e colaboração em modpacks.
