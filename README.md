# IlhaBox

A Ilha do trabalho, num celular de teste pelo cabo USB. No PC fica só um ícone na bandeja; o painel roda deitado no celular.

## Telas (celular)
- **Início**: relógio, horas apontadas no dia × meta, apontamento em andamento, música e últimas mensagens.
- **Horas**: cronômetro por ticket; cada apontamento vira o texto do Movidesk (modelo nos ajustes) e vai pra área de transferência do PC; resumo do dia.
- **Música**: o que toca no PC (Spotify, navegador), controles e letra sincronizada (lrclib.net).
- **Whats**: notificações do WhatsApp do PC (Desktop ou Web) das últimas 24 h; tocar traz o WhatsApp pra frente no PC.
- **Atalhos**: Abrir EasyPac (no celular), Celular normal, WhatsApp no PC, Resumo do dia e os atalhos de suporte (texto colado no PC, site, programa, app do celular).

## Mouse do PC no celular
Empurre o mouse pra fora da borda do lado onde o celular está (Ajustes › Celular): o cursor some do PC e aparece no celular. Clique, role e digite; volte pela mesma borda. A tecla Windows sempre devolve o mouse pro PC.

## Como funciona
- `src-tauri/src/adb.rs`: a cada 2 s vê os celulares no adb; num celular novo faz `adb reverse tcp:8777`, instala/atualiza o app, entrega a chave e abre o painel. Usa o adb do Android SDK quando existe (dois adb de versões diferentes derrubam o servidor um do outro) e **nunca** reinicia o servidor do adb (derrubaria a depuração do app da EasyPac).
- `src-tauri/src/server.rs`: HTTP em 127.0.0.1:8777. Páginas públicas; `/api/*` exige a chave (`box.key`). `/api/events` é SSE com o estado inteiro no começo.
- `src-tauri/src/bus.rs`: último valor de cada tópico + feed ao vivo.
- `android/`: app mínimo (Java, sem dependências) que mostra `http://127.0.0.1:8777/` em tela cheia, deitado. Telas novas não exigem app novo: vêm do PC.
- `android/versao.txt`: versão do app do celular. Suba o número quando mudar algo em `android/`: o PC reinstala sozinho.

## Atualizações
Mesma esteira da Ilha: código no repositório privado `BySzx/ilhabox`, instaladores no público `BySzx/ilhabox-releases`. A Ilha de casa publica sozinha quando esta pasta muda (`lancar.ps1`); na primeira vez roda o `publicar.ps1`. O app do celular é compilado no GitHub Actions e vai dentro do instalador.
