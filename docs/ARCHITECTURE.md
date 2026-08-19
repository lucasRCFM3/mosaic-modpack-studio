# Arquitetura do Mosaic

## Visão geral

O Mosaic é um aplicativo Tauri 2. O frontend React cuida exclusivamente de apresentação e interação; todas as decisões de compatibilidade, rede, segredos e disco pertencem ao núcleo Rust.

```text
React
  │ invoke/eventos tipados
  ▼
Comandos Tauri
  ├── CatalogService ───────► ProviderRegistry ─► Modrinth / CurseForge
  ├── DependencyResolver ───► grafo + plano imutável em memória
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
2. Adiciona dependências `required` ao plano e continua a travessia.
3. Registra dependências `optional` em uma lista de escolhas separada.
4. Só percorre uma opcional quando o ID dela foi explicitamente selecionado.
5. Ignora `embedded` e transforma `incompatible` em bloqueio quando há conflito.
6. Mantém o plano no backend; a interface devolve apenas seu identificador para instalar.

Essa separação garante que dependências opcionais não sejam instaladas por acidente. A preferência global de pré-seleção é apenas uma conveniência opt-in.

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
- deduplicação entre catálogos;
- regras de pins, overrides e presets de servidor;
- cache HTTP com ETag, fila persistente e retomada de downloads;
- auditoria de licenças, changelogs e vulnerabilidades;
- assinatura de lockfiles e colaboração em modpacks.
